//! macOS implementations. Only OCR is OS-specific (capture, hotkeys, notifications and
//! autostart come from cross-platform crates/plugins). The Vision-framework OCR is
//! deferred to the Mac port; see ROADMAP.md.

use image::RgbaImage;

use crate::error::{Error, Result};
use crate::platform::{FrozenFrame, OcrEngine, OcrResult, Region, RegionSelector};

pub struct AppKitRegionSelector;

impl RegionSelector for AppKitRegionSelector {
    fn select(&self, _frames: &[FrozenFrame], _on_shown: &mut dyn FnMut()) -> Result<Option<Region>> {
        Err(Error::Platform("macOS region selection is not implemented yet".into()))
    }
}

pub struct VisionOcr;

impl OcrEngine for VisionOcr {
    fn languages(&self) -> Vec<String> {
        Vec::new()
    }

    fn recognize(&self, _image: &RgbaImage) -> Result<OcrResult> {
        Err(Error::OcrUnavailable("macOS text recognition is not implemented yet".into()))
    }
}
