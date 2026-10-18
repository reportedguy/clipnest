//! Spike 2: frozen-frame capture on every monitor, plus crop-coordinate checks.
//!
//!   cargo run --release --example spike_capture
//!
//! Prints each monitor's geometry and scale, confirms captured frames are at physical
//! resolution, times the grab, and writes a centred crop per monitor to `.spikes/`.

use std::time::Instant;

use clipnest_lib::capture::geometry::from_drag;
use clipnest_lib::platform::native_capturer;

fn main() {
    let capturer = native_capturer();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.spikes");
    std::fs::create_dir_all(&out).unwrap();

    let t = Instant::now();
    let monitors = capturer.monitors().expect("enumerate monitors");
    println!("enumerate: {:?}", t.elapsed());
    for m in &monitors {
        println!(
            "monitor {} {:?}: origin ({}, {}) size {}x{} scale {} primary={}",
            m.id, m.name, m.x, m.y, m.width, m.height, m.scale_factor, m.is_primary
        );
    }

    // Warm-up, then time a few grabs: this is the latency before the overlay can appear.
    let _ = capturer.freeze_all();
    let mut times = Vec::new();
    let mut frames = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        frames = capturer.freeze_all().expect("capture");
        times.push(t.elapsed());
    }
    times.sort();
    println!(
        "freeze_all over {} monitor(s): min {:?} median {:?} max {:?}",
        frames.len(),
        times[0],
        times[2],
        times[4]
    );

    for f in &frames {
        let m = &f.monitor;
        let physical = f.image.width() == m.width && f.image.height() == m.height;
        println!(
            "monitor {}: frame {}x{} -> {}",
            m.id,
            f.image.width(),
            f.image.height(),
            if physical { "matches physical size (OK)" } else { "MISMATCH with reported size" }
        );

        // Simulate what the overlay will report: a 500x375 physical drag centred on the monitor.
        let (cx, cy) = (f.image.width() as i32 / 2, f.image.height() as i32 / 2);
        let r = from_drag((cx - 250, cy - 187), (cx + 250, cy + 188), f.image.width(), f.image.height())
            .expect("valid selection");
        let crop = image::imageops::crop_imm(&f.image, r.x, r.y, r.width, r.height).to_image();
        let path = out.join(format!("crop-monitor-{}.png", m.id));
        crop.save(&path).unwrap();
        println!("  centred crop {}x{} at ({}, {}) saved {}", r.width, r.height, r.x, r.y, path.display());
    }
}
