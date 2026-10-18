//! Local AI: model setup (download + verify), the `llama-server` engine lifecycle, and
//! turning a screenshot's note + text (+ image) into a validated filing.

pub mod commands;
pub mod download;
pub mod engine;
pub mod prompt;
pub mod registry;
pub mod urls;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use self::engine::{Engine, Launch};
use self::prompt::{Filing, Request};
use self::registry::{Download, Registry};
use crate::error::{Error, Result};

pub const STATUS_EVENT: &str = "ai-status";
/// Free the model's RAM after this long without work (spec 3.3).
const IDLE_SHUTDOWN: Duration = Duration::from_secs(120);
/// More engine crashes than this within the window means something is wrong (often
/// antivirus); stop restarting and tell the user.
const MAX_CRASHES: usize = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(600);
/// Images are scaled down to this before going to the vision model: more pixels mean many
/// more image tokens and much slower answers, with little gain for screenshots.
const VISION_MAX_SIDE: u32 = 1280;

pub struct Ai {
    dir: PathBuf,
    pub registry: Registry,
    slot: Mutex<Slot>,
    download: Mutex<DownloadState>,
    cancel: AtomicBool,
    running: AtomicBool,
}

#[derive(Default)]
struct Slot {
    engine: Option<Engine>,
    last_used: Option<Instant>,
    crashes: Vec<Instant>,
}

#[derive(Debug, Clone, Default)]
struct DownloadState {
    active: bool,
    /// Bytes present across all components of the current setup.
    downloaded: u64,
    total: u64,
    error: Option<String>,
    vision: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    /// "notInstalled" | "downloading" | "paused" | "error" | "ready"
    pub state: &'static str,
    pub model_name: String,
    /// Bytes the text setup needs in total (engine + text model) and how many are present.
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub error: Option<String>,
    pub engine_running: bool,
    pub vision_installed: bool,
    pub vision_bytes: u64,
    /// True while the vision projector itself is downloading.
    pub vision_downloading: bool,
}

impl Ai {
    pub fn new(data_dir: &Path) -> Self {
        let ai = Ai {
            dir: data_dir.join("ai"),
            registry: Registry::load(),
            slot: Mutex::new(Slot::default()),
            download: Mutex::new(DownloadState::default()),
            cancel: AtomicBool::new(false),
            running: AtomicBool::new(false),
        };
        ai.adopt_legacy_files();
        ai
    }

    /// Older builds kept every model directly in `models/`, so two models whose files
    /// share a name (the 2B and 4B vision projectors are both `mmproj-F16.gguf`) would
    /// overwrite each other. Files now live in `models/<model id>/`; this moves existing
    /// ones there once. A verified file goes to the model its checksum belongs to; an
    /// unverified or partial file only moves when its name belongs to a single model.
    fn adopt_legacy_files(&self) {
        let legacy_dir = self.dir.join("models");
        for m in &self.registry.models {
            let d = m.download();
            let legacy = legacy_dir.join(d.file_name());
            let target = self.model_path(&d);
            if target.exists() || download::part(&target).exists() {
                continue;
            }
            let verified_for_this = std::fs::read_to_string(download::marker(&legacy))
                .is_ok_and(|s| s.trim().eq_ignore_ascii_case(&d.sha256));
            let name_is_unique =
                self.registry.models.iter().filter(|o| o.download().file_name() == d.file_name()).count() == 1;
            if !(verified_for_this || name_is_unique) {
                continue;
            }
            let moves = [
                (legacy.clone(), target.clone()),
                (download::marker(&legacy), download::marker(&target)),
                (download::part(&legacy), download::part(&target)),
            ];
            if moves.iter().any(|(from, _)| from.exists()) {
                if let Some(dir) = target.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                for (from, to) in moves {
                    if from.exists() {
                        if let Err(e) = std::fs::rename(&from, &to) {
                            tracing::warn!(error = %e, from = %from.display(), "couldn't move model file");
                        }
                    }
                }
                tracing::info!(model = %m.id, "moved model files into their own folder");
            }
        }
    }

    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn dl(&self) -> MutexGuard<'_, DownloadState> {
        self.download.lock().unwrap_or_else(|p| p.into_inner())
    }

    // ---- files -------------------------------------------------------------------------

    fn engine_zip(&self) -> PathBuf {
        self.dir.join("engine").join(self.registry.engine.file_name())
    }

    fn engine_dir(&self) -> PathBuf {
        self.dir.join("engine").join(&self.registry.engine.id)
    }

    fn engine_exe(&self) -> PathBuf {
        self.engine_dir().join(self.registry.engine.binary.as_deref().unwrap_or("llama-server.exe"))
    }

    /// Each model has its own folder: different models can ship files with the same name.
    fn model_path(&self, d: &Download) -> PathBuf {
        self.dir.join("models").join(&d.id).join(d.file_name())
    }

    fn engine_ready(&self) -> bool {
        download::is_verified(&self.engine_zip(), &self.registry.engine) && self.engine_exe().exists()
    }

    pub fn text_ready(&self, profile: &str) -> bool {
        let (text, _) = self.registry.profile(profile);
        self.engine_ready() && download::is_verified(&self.model_path(&text.download()), &text.download())
    }

    pub fn vision_ready(&self, profile: &str) -> bool {
        let (_, vision) = self.registry.profile(profile);
        self.text_ready(profile) && download::is_verified(&self.model_path(&vision.download()), &vision.download())
    }

    /// The downloads a setup needs, in order.
    fn components(&self, profile: &str, vision: bool) -> Vec<(Download, PathBuf)> {
        let (text, vis) = self.registry.profile(profile);
        let mut c = vec![
            (self.registry.engine.clone(), self.engine_zip()),
            (text.download(), self.model_path(&text.download())),
        ];
        if vision {
            c.push((vis.download(), self.model_path(&vis.download())));
        }
        c
    }

    pub fn status(&self, profile: &str) -> AiStatus {
        let (text, vis) = self.registry.profile(profile);
        let text_parts = self.components(profile, false);
        let total: u64 = text_parts.iter().map(|(d, _)| d.size).sum();
        let present: u64 = text_parts.iter().map(|(d, p)| download::bytes_present(p, d)).sum();
        let dl = self.dl().clone();
        let ready = self.text_ready(profile);
        let state = match () {
            _ if dl.active && !dl.vision => "downloading",
            _ if ready => "ready",
            _ if dl.error.is_some() => "error",
            _ if present > 0 => "paused",
            _ => "notInstalled",
        };
        // While downloading, report the running job's own totals (which include the image
        // model when that's what's being fetched).
        let (total, present) = if dl.active { (dl.total, dl.downloaded.min(dl.total)) } else { (total, present) };
        AiStatus {
            state,
            model_name: text.name.clone(),
            total_bytes: total,
            downloaded_bytes: present,
            error: dl.error,
            engine_running: self.running.load(Ordering::Relaxed),
            vision_installed: self.vision_ready(profile),
            vision_bytes: vis.size,
            vision_downloading: dl.active && dl.vision,
        }
    }

    // ---- setup -------------------------------------------------------------------------

    /// Starts (or resumes) downloading what `profile` needs. Runs in the background; the
    /// app stays fully usable. With `vision`, the image projector is included.
    pub fn start_download<R: Runtime>(app: &AppHandle<R>, profile: String, vision: bool) {
        let ai = app.state::<Ai>();
        {
            let mut dl = ai.dl();
            if dl.active {
                return;
            }
            let parts = ai.components(&profile, vision);
            *dl = DownloadState {
                active: true,
                downloaded: parts.iter().map(|(d, p)| download::bytes_present(p, d)).sum(),
                total: parts.iter().map(|(d, _)| d.size).sum(),
                error: None,
                vision,
            };
        }
        ai.cancel.store(false, Ordering::Relaxed);
        publish(app, &profile);

        let app = app.clone();
        let spawned = std::thread::Builder::new().name("ai-download".into()).spawn(move || {
            crate::platform::lower_current_thread_priority();
            let ai = app.state::<Ai>();
            let result = ai.run_download(&app, &profile, vision);
            {
                let mut dl = ai.dl();
                dl.active = false;
                dl.error = match &result {
                    Err(e) if !download::is_paused(e) => Some(e.to_string()),
                    _ => None,
                };
            }
            match result {
                Ok(()) => {
                    tracing::info!(%profile, vision, "AI setup complete");
                    crate::queue::wake(&app);
                }
                Err(e) if download::is_paused(&e) => tracing::info!("AI download paused"),
                Err(e) => tracing::warn!(error = %e, "AI download failed"),
            }
            publish(&app, &profile);
        });
        if let Err(e) = spawned {
            tracing::error!(error = %e, "couldn't start the AI download");
        }
    }

    fn run_download<R: Runtime>(&self, app: &AppHandle<R>, profile: &str, vision: bool) -> Result<()> {
        let mut done_before = 0u64;
        let mut last_publish = Instant::now();
        for (d, dest) in self.components(profile, vision) {
            let mut report = |bytes: u64| {
                self.dl().downloaded = done_before + bytes;
                if last_publish.elapsed() >= Duration::from_millis(500) {
                    publish(app, profile);
                    last_publish = Instant::now();
                }
            };
            download::fetch(&d, &dest, &self.cancel, &mut report)?;
            if d.binary.is_some() && !self.engine_exe().exists() {
                download::extract_zip(&dest, &self.engine_dir())?;
            }
            done_before += d.size;
        }
        Ok(())
    }

    pub fn pause_download(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Removes the downloaded engine and models (e.g. to free space or re-download).
    pub fn delete_all(&self) -> Result<()> {
        self.pause_download();
        self.stop();
        if self.dl().active {
            return Err(Error::Invalid("Wait for the download to pause, then try again.".into()));
        }
        for profile in self.registry.profiles.keys() {
            for (_, dest) in self.components(profile, true) {
                download::remove(&dest);
            }
        }
        let _ = std::fs::remove_dir_all(self.engine_dir());
        self.dl().error = None;
        Ok(())
    }

    // ---- engine ------------------------------------------------------------------------

    pub fn stop(&self) {
        let mut slot = self.slot();
        if let Some(mut e) = slot.engine.take() {
            e.kill();
            tracing::info!("AI engine stopped");
        }
        self.running.store(false, Ordering::Relaxed);
    }

    /// Stops the engine if it has been idle long enough. Never blocks on a busy engine.
    pub fn stop_if_idle(&self) {
        let Ok(mut slot) = self.slot.try_lock() else { return }; // busy filing
        if slot.engine.is_some() && slot.last_used.is_some_and(|t| t.elapsed() >= IDLE_SHUTDOWN) {
            if let Some(mut e) = slot.engine.take() {
                e.kill();
            }
            self.running.store(false, Ordering::Relaxed);
            tracing::info!("AI engine stopped after being idle");
        }
    }

    fn ensure_engine<'a>(&self, slot: &'a mut Slot, profile: &str, vision: bool) -> Result<&'a mut Engine> {
        let alive = match slot.engine.as_mut() {
            Some(e) => e.is_alive(),
            None => true,
        };
        if !alive {
            slot.engine = None;
            slot.crashes.push(Instant::now());
            tracing::warn!("AI engine had stopped; restarting");
        }
        slot.crashes.retain(|t| t.elapsed() < CRASH_WINDOW);
        if slot.crashes.len() >= MAX_CRASHES {
            return Err(Error::Invalid(
                "The AI engine keeps stopping. Antivirus software may be interfering; try again later or re-download the AI in Settings."
                    .into(),
            ));
        }
        // Text-only engine but this item needs vision: restart with the projector.
        if vision && slot.engine.as_ref().is_some_and(|e| !e.vision) {
            slot.engine = None;
        }
        if slot.engine.is_none() {
            let (text, vis) = self.registry.profile(profile);
            let model = self.model_path(&text.download());
            let mmproj = vision.then(|| self.model_path(&vis.download()));
            let log = self.dir.join("llama-server.log");
            let engine = Engine::start(&Launch {
                exe: &self.engine_exe(),
                model: &model,
                mmproj: mmproj.as_deref(),
                log: &log,
            })?;
            slot.engine = Some(engine);
            self.running.store(true, Ordering::Relaxed);
        }
        Ok(slot.engine.as_mut().expect("just ensured"))
    }

    /// Files one screenshot. Retries once with a stricter prompt if the answer can't be
    /// used. `image` (Image mode) sends the picture to the vision model.
    pub fn file(&self, profile: &str, req: &Request, image: Option<&Path>, misc_name: &str) -> Result<Filing> {
        let image_url = image.map(data_url).transpose()?;
        let (text_model, _) = self.registry.profile(profile);
        let kwargs = text_model.chat_template_kwargs.clone().unwrap_or(json!({}));
        let schema = prompt::schema(req.categories);

        let mut slot = self.slot();
        let mut last_err = None;
        for strict in [false, true] {
            let body = json!({
                "messages": prompt::messages(req, strict, image_url.as_deref()),
                "temperature": if strict { 0.0 } else { 0.2 },
                "max_tokens": 400,
                "response_format": { "type": "json_schema", "json_schema": { "name": "screenshot", "schema": schema } },
                "chat_template_kwargs": kwargs,
            });
            let engine = self.ensure_engine(&mut slot, profile, image_url.is_some())?;
            let started = Instant::now();
            let reply = engine.chat(&body);
            slot.last_used = Some(Instant::now());
            let reply = reply?;
            let content = reply["choices"][0]["message"]["content"].as_str().unwrap_or_default();
            match prompt::parse(content, req.categories, misc_name) {
                Ok(f) => {
                    tracing::info!(ms = started.elapsed().as_millis() as u64, strict, vision = image_url.is_some(), purpose = %f.note_purpose, title = %f.title, "filed");
                    return Ok(f);
                }
                Err(e) => {
                    tracing::warn!(error = %e, strict, "unusable AI answer");
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Invalid("the AI's answer couldn't be used".into())))
    }
}

/// A PNG data URL of the image, scaled down for the vision model.
fn data_url(path: &Path) -> Result<String> {
    let img = image::open(path)?;
    let img = if img.width().max(img.height()) > VISION_MAX_SIDE {
        img.resize(VISION_MAX_SIDE, VISION_MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

/// Pushes the AI status to the UI.
pub fn publish<R: Runtime>(app: &AppHandle<R>, profile: &str) {
    if let Some(ai) = app.try_state::<Ai>() {
        let _ = app.emit(STATUS_EVENT, ai.status(profile));
    }
}

/// Background housekeeping: stops the engine after it's been idle (frees ~2-3 GB of RAM).
pub fn start_reaper<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("ai-reaper".into()).spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15));
        if let Some(ai) = app.try_state::<Ai>() {
            let was_running = ai.running.load(Ordering::Relaxed);
            ai.stop_if_idle();
            if was_running && !ai.running.load(Ordering::Relaxed) {
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    publish(&app, &state.settings().ai_profile);
                }
            }
        }
    });
}
