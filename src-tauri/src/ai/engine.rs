//! The `llama-server` sidecar: started on demand, health-checked, talked to over
//! localhost HTTP, and stopped when idle. It runs at below-normal priority on about half
//! the CPU's threads, with no console window, and is tied to the app's lifetime so it can
//! never be left running after the app exits.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::error::{Error, Result};

/// Model loading on a slow disk can take a while; the first request also warms caches.
const START_TIMEOUT: Duration = Duration::from_secs(180);
/// A single filing request (image mode on a slow CPU can approach the spec's 3 minutes).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const CONTEXT: u32 = 4096;

pub struct Engine {
    child: Child,
    port: u16,
    /// Whether this instance was started with the vision projector loaded.
    pub vision: bool,
    log: PathBuf,
    agent: ureq::Agent,
}

pub struct Launch<'a> {
    pub exe: &'a Path,
    pub model: &'a Path,
    pub mmproj: Option<&'a Path>,
    pub log: &'a Path,
}

/// About half the logical cores (spec 3.3), at least 2 and at most 8.
pub fn thread_count() -> usize {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (n / 2).clamp(2, 8)
}

impl Engine {
    pub fn start(l: &Launch) -> Result<Engine> {
        if !l.exe.exists() {
            return Err(Error::Invalid(
                "The AI engine file is missing. Antivirus software may have removed it. Re-download it in Settings › AI."
                    .into(),
            ));
        }
        let port = free_port()?;
        let log = File::create(l.log)?;
        let mut cmd = crate::platform::background_command(l.exe);
        cmd.args(["--model"])
            .arg(l.model)
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            .args(["--threads", &thread_count().to_string()])
            .args(["--ctx-size", &CONTEXT.to_string(), "--n-gpu-layers", "0", "--parallel", "1", "--no-webui"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        if let Some(mmproj) = l.mmproj {
            cmd.arg("--mmproj").arg(mmproj);
        }
        let child = cmd.spawn().map_err(|e| {
            Error::Invalid(format!("Couldn't start the AI engine ({e}). Antivirus software may be blocking it."))
        })?;
        crate::platform::bind_child_to_app(&child);

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        let mut engine = Engine { child, port, vision: l.mmproj.is_some(), log: l.log.to_path_buf(), agent };
        let started = Instant::now();
        engine.wait_healthy()?;
        tracing::info!(port, vision = engine.vision, ms = started.elapsed().as_millis() as u64, "AI engine ready");
        Ok(engine)
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn wait_healthy(&mut self) -> Result<()> {
        let deadline = Instant::now() + START_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait()? {
                return Err(Error::Invalid(format!(
                    "The AI engine stopped while starting ({status}). {}",
                    self.log_tail()
                )));
            }
            if let Ok(resp) = self.agent.get(self.url("/health")).call() {
                if resp.status().as_u16() == 200 {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        self.kill();
        Err(Error::Invalid("The AI engine took too long to start.".into()))
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// POSTs an OpenAI-style chat completion and returns the parsed response.
    pub fn chat(&mut self, body: &Value) -> Result<Value> {
        let resp = self.agent.post(self.url("/v1/chat/completions")).send_json(body);
        let mut resp = match resp {
            Ok(r) => r,
            Err(e) => {
                let crashed = !self.is_alive();
                return Err(Error::Invalid(if crashed {
                    format!("The AI engine stopped unexpectedly. {}", self.log_tail())
                } else {
                    format!("The AI engine didn't answer ({e}).")
                }));
            }
        };
        let status = resp.status().as_u16();
        let json: Value = resp
            .body_mut()
            .with_config()
            .limit(8 * 1024 * 1024)
            .read_json()
            .map_err(|e| Error::Invalid(format!("The AI engine sent an unreadable reply ({e}).")))?;
        if status != 200 {
            let msg = json["error"]["message"].as_str().unwrap_or("unknown error");
            return Err(Error::Invalid(format!("The AI engine reported an error: {msg}")));
        }
        Ok(json)
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// The last line or two of the engine's log, to explain a failure.
    fn log_tail(&self) -> String {
        let Ok(mut f) = File::open(&self.log) else { return String::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let _ = f.seek(SeekFrom::Start(len.saturating_sub(600)));
        let mut tail = String::new();
        let _ = f.read_to_string(&mut tail);
        let line = tail.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        if line.is_empty() {
            String::new()
        } else {
            format!("Last message: {line}")
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.kill();
    }
}

fn free_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_count_is_bounded() {
        let n = thread_count();
        assert!((2..=8).contains(&n));
    }

    #[test]
    fn missing_engine_explains_antivirus() {
        let err = Engine::start(&Launch {
            exe: Path::new("C:/definitely/missing/llama-server.exe"),
            model: Path::new("m.gguf"),
            mmproj: None,
            log: &std::env::temp_dir().join("rs-engine-test.log"),
        });
        assert!(err.err().unwrap().to_string().contains("Antivirus"));
    }
}
