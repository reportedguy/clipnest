//! Resumable, verified downloads for the AI engine and models: HTTP Range resume from a
//! `.part` file, retries with backoff, a free-space check, and SHA-256 verification before
//! anything is used. These are the app's only network requests.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::registry::Download;
use crate::error::{Error, Result};

const RETRIES: u32 = 5;
const CHUNK: usize = 1 << 20;
/// Kept free beyond the download itself so the user's disk never fills up completely.
const DISK_MARGIN: u64 = 500 * 1024 * 1024;

/// Marker written next to a verified file, so it isn't re-hashed on every start.
pub(crate) fn marker(dest: &Path) -> PathBuf {
    let mut m = dest.as_os_str().to_owned();
    m.push(".verified");
    m.into()
}

pub(crate) fn part(dest: &Path) -> PathBuf {
    let mut p = dest.as_os_str().to_owned();
    p.push(".part");
    p.into()
}

/// True if `dest` is present, the right size, and was verified against `sha256`.
pub fn is_verified(dest: &Path, d: &Download) -> bool {
    let size_ok = fs::metadata(dest).map(|m| m.len() == d.size).unwrap_or(false);
    size_ok && fs::read_to_string(marker(dest)).map(|s| s.trim().eq_ignore_ascii_case(&d.sha256)).unwrap_or(false)
}

/// Bytes already on disk for this download (finished, or partial).
pub fn bytes_present(dest: &Path, d: &Download) -> u64 {
    if is_verified(dest, d) {
        return d.size;
    }
    fs::metadata(part(dest)).map(|m| m.len().min(d.size)).unwrap_or(0)
}

pub fn remove(dest: &Path) {
    for p in [dest.to_path_buf(), part(dest), marker(dest)] {
        let _ = fs::remove_file(p);
    }
}

/// Downloads `d` to `dest` (resuming a previous partial download), verifies it, and marks
/// it verified. `progress` receives total bytes present so far. Stops early (keeping the
/// partial file, see `is_paused`) when `cancel` is set.
pub fn fetch(d: &Download, dest: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(u64)) -> Result<()> {
    if is_verified(dest, d) {
        progress(d.size);
        return Ok(());
    }
    let dir = dest.parent().expect("download destination has a parent");
    fs::create_dir_all(dir)?;
    let partial = part(dest);
    let have = fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
    if have > d.size {
        fs::remove_file(&partial)?;
    }
    let needed = d.size.saturating_sub(have.min(d.size)) + DISK_MARGIN;
    if let Some(free) = crate::platform::free_disk_space(dir) {
        if free < needed {
            return Err(Error::Invalid(format!(
                "Not enough disk space: the AI download needs {} MB more free space.",
                (needed - free).div_ceil(1024 * 1024)
            )));
        }
    }

    let mut attempt = 0;
    loop {
        match stream(d, &partial, cancel, progress) {
            Ok(()) => break,
            Err(e) if is_paused(&e) => return Err(e),
            Err(e) if attempt < RETRIES => {
                attempt += 1;
                let wait = Duration::from_secs(2u64.pow(attempt).min(30));
                tracing::warn!(id = %d.id, attempt, error = %e, ?wait, "download interrupted; retrying");
                let until = Instant::now() + wait;
                while Instant::now() < until {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(paused());
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
            Err(e) => {
                return Err(Error::Invalid(format!(
                    "The download keeps failing ({e}). Check your connection and try again."
                )))
            }
        }
    }

    let actual = sha256_file(&partial)?;
    if !actual.eq_ignore_ascii_case(&d.sha256) {
        let _ = fs::remove_file(&partial);
        return Err(Error::Invalid("The downloaded file was damaged (checksum mismatch). Please try again.".into()));
    }
    fs::rename(&partial, dest)?;
    fs::write(marker(dest), &d.sha256)?;
    tracing::info!(id = %d.id, bytes = d.size, "download verified");
    Ok(())
}

fn paused() -> Error {
    Error::Invalid("paused".into())
}

pub fn is_paused(e: &Error) -> bool {
    matches!(e, Error::Invalid(m) if m == "paused")
}

/// One HTTP attempt, appending to the partial file from where it left off.
fn stream(d: &Download, partial: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(u64)) -> Result<()> {
    let have = fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    if have == d.size {
        return Ok(());
    }
    // ureq's body timeout is a budget for the *whole* body, not per read, so it's scaled
    // to the remaining size at a pessimistic 50 KB/s. If a stalled connection hits it,
    // the retry simply resumes from the partial file.
    let remaining = d.size.saturating_sub(have);
    let body_budget = Duration::from_secs((remaining / (50 * 1024)).max(600));
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(body_budget))
        .build()
        .into();
    let mut req = agent.get(&d.url).header("User-Agent", concat!("ClipNest/", env!("CARGO_PKG_VERSION")));
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let resp = req.call().map_err(|e| Error::Platform(format!("network error: {e}")))?;
    // 206 = resumed; 200 = server ignored the range, so start over.
    let resumed = resp.status().as_u16() == 206;
    let mut file = if resumed { OpenOptions::new().append(true).open(partial)? } else { File::create(partial)? };
    let mut done = if resumed { have } else { 0 };
    let mut reader = resp.into_body().into_with_config().limit(d.size + 1).reader();
    let mut buf = vec![0u8; CHUNK];
    let mut last_report = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            file.flush()?;
            return Err(paused());
        }
        let n = reader.read(&mut buf).map_err(|e| Error::Platform(format!("network error: {e}")))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if last_report.elapsed() >= Duration::from_millis(250) {
            progress(done);
            last_report = Instant::now();
        }
    }
    file.flush()?;
    progress(done);
    if done != d.size {
        return Err(Error::Platform(format!("connection closed early ({done} of {} bytes)", d.size)));
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        match file.read(&mut buf)? {
            0 => break,
            n => hasher.update(&buf[..n]),
        }
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Extracts a zip into `dir` (flattening nothing; refusing paths that escape `dir`).
pub fn extract_zip(zip_path: &Path, dir: &Path) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(File::open(zip_path)?).map_err(|e| Error::Invalid(format!("bad engine archive: {e}")))?;
    fs::create_dir_all(dir)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| Error::Invalid(format!("bad engine archive: {e}")))?;
        let Some(rel) = entry.enclosed_name() else { continue };
        let out = dir.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            io::copy(&mut entry, &mut File::create(&out)?)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rs-dl-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn verification_needs_marker_size_and_hash() {
        let dir = tmp("verify");
        let dest = dir.join("model.gguf");
        fs::write(&dest, b"hello").unwrap();
        let sha = sha256_file(&dest).unwrap();
        assert_eq!(sha, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        let d =
            Download { id: "m".into(), url: "https://x/model.gguf".into(), size: 5, sha256: sha.clone(), binary: None };
        assert!(!is_verified(&dest, &d)); // no marker yet
        fs::write(marker(&dest), &sha).unwrap();
        assert!(is_verified(&dest, &d));
        assert_eq!(bytes_present(&dest, &d), 5);
        fs::write(&dest, b"hello!").unwrap(); // size changed
        assert!(!is_verified(&dest, &d));
        remove(&dest);
        assert!(!dest.exists() && !marker(&dest).exists());
    }

    #[test]
    fn already_verified_files_skip_the_network() {
        let dir = tmp("skip");
        let dest = dir.join("m.bin");
        fs::write(&dest, b"abc").unwrap();
        let sha = sha256_file(&dest).unwrap();
        fs::write(marker(&dest), &sha).unwrap();
        // An unreachable URL proves no request is made.
        let d = Download { id: "m".into(), url: "http://127.0.0.1:9/m.bin".into(), size: 3, sha256: sha, binary: None };
        let mut seen = 0;
        fetch(&d, &dest, &AtomicBool::new(false), &mut |b| seen = b).unwrap();
        assert_eq!(seen, 3);
    }

    #[test]
    fn partial_downloads_count_as_progress() {
        let dir = tmp("partial");
        let dest = dir.join("m.bin");
        fs::write(part(&dest), vec![0u8; 40]).unwrap();
        let d =
            Download { id: "m".into(), url: "https://x/m.bin".into(), size: 100, sha256: "0".repeat(64), binary: None };
        assert_eq!(bytes_present(&dest, &d), 40);
    }
}
