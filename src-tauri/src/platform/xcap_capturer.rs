//! Screen capture via the cross-platform `xcap` crate (DXGI/GDI on Windows,
//! ScreenCaptureKit/CoreGraphics on macOS).

use super::{Capturer, FrozenFrame, MonitorInfo};
use crate::error::{Error, Result};

pub struct XcapCapturer;

fn info(m: &xcap::Monitor) -> Result<MonitorInfo> {
    let err = |e: xcap::XCapError| Error::Capture(e.to_string());
    Ok(MonitorInfo {
        id: m.id().map_err(err)?,
        name: m.name().map_err(err)?,
        x: m.x().map_err(err)?,
        y: m.y().map_err(err)?,
        width: m.width().map_err(err)?,
        height: m.height().map_err(err)?,
        scale_factor: f64::from(m.scale_factor().map_err(err)?),
        is_primary: m.is_primary().map_err(err)?,
    })
}

impl Capturer for XcapCapturer {
    fn monitors(&self) -> Result<Vec<MonitorInfo>> {
        xcap::Monitor::all().map_err(|e| Error::Capture(e.to_string()))?.iter().map(info).collect()
    }

    fn freeze_all(&self) -> Result<Vec<FrozenFrame>> {
        let monitors = xcap::Monitor::all().map_err(|e| Error::Capture(e.to_string()))?;
        monitors
            .iter()
            .map(|m| {
                let image = m.capture_image().map_err(|e| Error::Capture(e.to_string()))?;
                Ok(FrozenFrame { monitor: info(m)?, image })
            })
            .collect()
    }
}
