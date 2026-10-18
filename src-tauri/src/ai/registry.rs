//! The model registry (`models.json` at the repo root, compiled in). Swapping models is
//! a data change: edit the file, no code changes.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Registry {
    pub engine: Download,
    pub default_profile: String,
    pub profiles: HashMap<String, Profile>,
    pub models: Vec<Model>,
}

/// Something to download and verify.
#[derive(Debug, Clone, Deserialize)]
pub struct Download {
    pub id: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// For the engine: the executable inside the zip.
    #[serde(default)]
    pub binary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub text: String,
    pub vision: String,
    pub min_ram_gb: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: String,
    pub role: String,
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub ram_peak_mb: u32,
    #[serde(default)]
    pub chat_template_kwargs: Option<serde_json::Value>,
}

impl Model {
    pub fn download(&self) -> Download {
        Download {
            id: self.id.clone(),
            url: self.url.clone(),
            size: self.size,
            sha256: self.sha256.clone(),
            binary: None,
        }
    }
}

impl Download {
    /// The file name at the end of the URL.
    pub fn file_name(&self) -> &str {
        self.url.rsplit('/').next().unwrap_or(&self.id)
    }
}

impl Registry {
    pub fn load() -> Self {
        serde_json::from_str(include_str!("../../../models.json")).expect("models.json is valid (checked by tests)")
    }

    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    /// The text and vision models for a profile, falling back to the default profile.
    pub fn profile(&self, name: &str) -> (&Model, &Model) {
        let p = self
            .profiles
            .get(name)
            .or_else(|| self.profiles.get(&self.default_profile))
            .expect("default profile exists");
        (
            self.model(&p.text).expect("profile text model exists"),
            self.model(&p.vision).expect("profile vision model exists"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_complete_and_consistent() {
        let r = Registry::load();
        assert!(r.engine.binary.is_some());
        assert!(r.profiles.contains_key(&r.default_profile));
        for (name, p) in &r.profiles {
            assert_eq!(r.model(&p.text).map(|m| m.role.as_str()), Some("text"), "profile {name}");
            assert_eq!(r.model(&p.vision).map(|m| m.role.as_str()), Some("vision"), "profile {name}");
        }
        for m in &r.models {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.url.starts_with("https://huggingface.co/"), "{}", m.id);
            assert!(m.size > 0);
        }
        assert!(r.engine.url.starts_with("https://github.com/ggml-org/llama.cpp/"));
        let (text, vision) = r.profile("standard");
        assert_eq!((text.id.as_str(), vision.id.as_str()), ("qwen3.5-2b-q4km", "qwen3.5-2b-mmproj-f16"));
        assert_eq!(r.profile("no-such-profile").0.id, text.id);
        assert_eq!(text.download().file_name(), "Qwen3.5-2B-Q4_K_M.gguf");
    }
}
