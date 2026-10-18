/// Current time as Unix epoch milliseconds (the unit used for every DB timestamp).
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Installed memory in whole GB (what the sticker says, e.g. 16), or 0 if unknown.
pub fn total_ram_gb() -> u32 {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{
            GetPhysicallyInstalledSystemMemory, GlobalMemoryStatusEx, MEMORYSTATUSEX,
        };
        let mut kb = 0u64;
        // SAFETY: plain out-parameter calls.
        if unsafe { GetPhysicallyInstalledSystemMemory(&mut kb) }.is_ok() && kb > 0 {
            return (kb as f64 / (1024.0 * 1024.0)).round() as u32;
        }
        let mut m = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        if unsafe { GlobalMemoryStatusEx(&mut m) }.is_ok() {
            return (m.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0)).round() as u32;
        }
    }
    0
}
