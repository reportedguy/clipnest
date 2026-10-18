//! OCR through the built-in `Windows.Media.Ocr` WinRT API: offline, free, and uses the
//! language packs the user already has installed.

use image::{imageops::FilterType, RgbaImage};
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine as WinOcrEngine;
use windows::Security::Cryptography::CryptographicBuffer;

use crate::error::{Error, Result};
use crate::platform::{OcrEngine, OcrResult};

pub struct WindowsOcr;

fn ocr_err(e: windows::core::Error) -> Error {
    Error::Ocr(e.message().to_string())
}

impl OcrEngine for WindowsOcr {
    fn languages(&self) -> Vec<String> {
        let Ok(langs) = WinOcrEngine::AvailableRecognizerLanguages() else { return Vec::new() };
        langs.into_iter().filter_map(|l| l.LanguageTag().ok()).map(|t| t.to_string()).collect()
    }

    fn recognize(&self, image: &RgbaImage) -> Result<OcrResult> {
        // Uses the user's profile languages (falls back through the list Windows provides).
        // Errors here almost always mean no OCR-capable language pack is installed.
        let engine = WinOcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| Error::OcrUnavailable("no Windows OCR language pack is installed".into()))?;

        // The API rejects images larger than MaxImageDimension on either side.
        let max = WinOcrEngine::MaxImageDimension().map_err(ocr_err)?;
        let scaled;
        let img = if image.width() > max || image.height() > max {
            let ratio = f64::from(max) / f64::from(image.width().max(image.height()));
            let w = ((f64::from(image.width()) * ratio).floor() as u32).max(1);
            let h = ((f64::from(image.height()) * ratio).floor() as u32).max(1);
            scaled = image::imageops::resize(image, w, h, FilterType::Triangle);
            &scaled
        } else {
            image
        };

        // WinRT wants BGRA8.
        let mut bgra = img.as_raw().clone();
        for px in bgra.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let buffer = CryptographicBuffer::CreateFromByteArray(&bgra).map_err(ocr_err)?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            img.width() as i32,
            img.height() as i32,
            BitmapAlphaMode::Premultiplied,
        )
        .map_err(ocr_err)?;

        // Blocks this thread until recognition finishes; callers run OCR off the UI thread.
        let result = engine.RecognizeAsync(&bitmap).map_err(ocr_err)?.join().map_err(ocr_err)?;
        let lines: Vec<String> = result
            .Lines()
            .map_err(ocr_err)?
            .into_iter()
            .filter_map(|line| line.Text().ok())
            .map(|t| t.to_string())
            .collect();
        let language = engine.RecognizerLanguage().and_then(|l| l.LanguageTag()).ok().map(|t| t.to_string());

        Ok(OcrResult { text: lines.join("\n"), language })
    }
}
