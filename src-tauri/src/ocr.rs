//! Platform-independent OCR pipeline around `platform::OcrEngine`: image preparation and
//! the "is there enough text?" check behind the smart fallback to Image mode.
//!
//! Spike 3 found that 2× upscaling fixes small/light UI text (CER 3.9% → 0.4%) but hurts
//! light-on-dark text (0% → 12.5%). So dark images are inverted to dark-on-light first,
//! and only then upscaled when the text is likely small.

use image::imageops::FilterType;
use image::RgbaImage;

use crate::error::Result;
use crate::platform::{OcrEngine, OcrResult};

/// Below this many letters/digits, the screenshot is treated as visual (Image mode).
pub const MIN_MEANINGFUL_CHARS: usize = 15;

/// Upscale when the image is at most this tall; taller captures already hold readable text sizes.
const UPSCALE_MAX_HEIGHT: u32 = 1200;
/// Don't upscale past this on either side (Windows OCR's own limit is around 10 000 px).
const UPSCALE_MAX_SIDE: u32 = 4000;

/// Runs OCR with preparation and cleanup. Engines downscale oversize images themselves.
pub fn recognize(engine: &dyn OcrEngine, image: &RgbaImage) -> Result<OcrResult> {
    let mut result = engine.recognize(&prepare(image))?;
    result.text = fix_split_numbers(&result.text);
    Ok(result)
}

static SPLIT_NUMBER: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\b(\d) (\d{1,3}[/.\-]\d)").expect("valid regex"));

/// Windows OCR sometimes splits a number at a narrow gap ("12/11/2026" read as
/// "1 2/11/2026"), which turns a deadline into the wrong date. Rejoins a lone digit
/// with a directly following date-like number.
pub fn fix_split_numbers(text: &str) -> String {
    SPLIT_NUMBER.replace_all(text, "$1$2").into_owned()
}

/// Inverts mostly-dark images to dark-on-light, then upscales 2× if the image is small.
pub fn prepare(image: &RgbaImage) -> RgbaImage {
    let mut img = image.clone();
    if is_dark(&img) {
        for px in img.pixels_mut() {
            px.0 = [255 - px[0], 255 - px[1], 255 - px[2], px[3]];
        }
    }
    let (w, h) = img.dimensions();
    if h <= UPSCALE_MAX_HEIGHT && w * 2 <= UPSCALE_MAX_SIDE && h * 2 <= UPSCALE_MAX_SIDE {
        img = image::imageops::resize(&img, w * 2, h * 2, FilterType::CatmullRom);
    }
    img
}

/// True when the average luminance is below middle grey (dark-mode UI, video players…).
fn is_dark(img: &RgbaImage) -> bool {
    // Sampling every 7th pixel is plenty for a mean and keeps this ~free on 4K captures.
    let (sum, n) = img.pixels().step_by(7).fold((0u64, 0u64), |(s, n), p| {
        let y = (299 * u64::from(p[0]) + 587 * u64::from(p[1]) + 114 * u64::from(p[2])) / 1000;
        (s + y, n + 1)
    });
    n > 0 && sum / n < 128
}

/// Letters and digits in the text (any script). Punctuation and stray symbols from icons
/// don't count, so a toolbar screenshot reading "| • ≡ ›" isn't mistaken for text.
pub fn meaningful_chars(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_images_are_inverted_light_ones_are_not() {
        let dark = RgbaImage::from_pixel(10, 10, image::Rgba([20, 20, 30, 255]));
        let light = RgbaImage::from_pixel(10, 10, image::Rgba([240, 240, 240, 255]));
        assert_eq!(prepare(&dark).get_pixel(0, 0).0, [235, 235, 225, 255]);
        assert_eq!(prepare(&light).get_pixel(0, 0).0, [240, 240, 240, 255]);
    }

    #[test]
    fn small_images_are_upscaled_large_ones_are_not() {
        assert_eq!(prepare(&RgbaImage::new(600, 200)).dimensions(), (1200, 400));
        assert_eq!(prepare(&RgbaImage::new(1920, 1300)).dimensions(), (1920, 1300));
        assert_eq!(prepare(&RgbaImage::new(2500, 300)).dimensions(), (2500, 300));
        // would exceed max side
    }

    #[test]
    fn split_numbers_are_rejoined() {
        // Seen in testing: the hackathon deadline read as 2 Nov instead of 12 Nov.
        assert_eq!(fix_split_numbers("close on 1 2/11/2026. Apply"), "close on 12/11/2026. Apply");
        assert_eq!(fix_split_numbers("due 3 1.12.26"), "due 31.12.26");
        // Ordinary text with numbers is left alone.
        assert_eq!(fix_split_numbers("Teams of up to 4 people. Top 3 teams"), "Teams of up to 4 people. Top 3 teams");
        assert_eq!(fix_split_numbers("page 1 of 2"), "page 1 of 2");
    }

    #[test]
    fn meaningful_chars_ignores_symbols() {
        assert_eq!(meaningful_chars("| • ≡ › ← →"), 0);
        assert_eq!(meaningful_chars("Apply by 12/11"), 11);
        // Non-Latin scripts count too (Unicode treats vowel signs as alphabetic, the virama not).
        assert_eq!(meaningful_chars("नमस्ते"), 5);
    }
}
