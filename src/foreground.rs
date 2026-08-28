//! Tracks whether the emulator is the foreground window.
//!
//! Resolving the foreground process costs three syscalls, far too much to run on every
//! mouse event. A background thread samples it and publishes a flag the hook can read
//! for free.

use std::sync::atomic::Ordering;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::shared::SHARED;

/// BlueStacks 5 / MSI player.
pub const TARGET_EXE: &str = "hd-player.exe";

const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);

pub fn spawn_watcher() {
    std::thread::spawn(|| {
        loop {
            let focused = foreground_exe()
                .map(|name| name.eq_ignore_ascii_case(TARGET_EXE))
                .unwrap_or(false);
            SHARED.target_focused.store(focused, Ordering::Relaxed);
            std::thread::sleep(SAMPLE_INTERVAL);
        }
    });
}

fn foreground_exe() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; MAX_PATH as usize];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        ok.ok()?;

        let full = String::from_utf16_lossy(&buffer[..len as usize]);
        full.rsplit(['\\', '/']).next().map(|s| s.to_string())
    }
}
