//! Capture: the region-select flow and saving captures. OS-specific grabbing and the
//! selection overlay live behind `platform::{Capturer, RegionSelector}`; everything here
//! is platform-independent.

pub mod geometry;
pub mod session;
pub mod store;

pub use session::{start, CaptureManager};
