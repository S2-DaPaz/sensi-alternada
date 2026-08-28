//! State the panel writes and the hook reads. The hook runs on every mouse event, so
//! everything here is lock-free.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use crate::fire_button::FireButton;

/// Factors are carried as parts-per-million so they fit an atomic.
const PPM: f32 = 1_000_000.0;

pub struct Shared {
    pub enabled: AtomicBool,
    pub target_focused: AtomicBool,
    pub holding_fire: AtomicBool,
    factor_x_ppm: AtomicU32,
    factor_y_ppm: AtomicU32,
    fire_button: AtomicU8,
    /// Thread id of the message loop that owns the hook, published once it starts.
    pub hook_thread: AtomicU32,
    /// Contadores de diagnostico, dumpados quando SENSI_DEBUG=1.
    pub raw_seen: AtomicU32,
    pub raw_abs: AtomicU32,
    pub injected: AtomicU32,
    pub suppressed: AtomicU32,
    pub hook_installed: AtomicBool,
    pub sink_ok: AtomicBool,
    pub raw_registered: AtomicBool,
    /// HWND da janela so-de-mensagem, para reinscricao.
    pub sink_hwnd: std::sync::atomic::AtomicUsize,
    pub reregistros: AtomicU32,
}

pub static SHARED: Shared = Shared {
    enabled: AtomicBool::new(false),
    target_focused: AtomicBool::new(false),
    holding_fire: AtomicBool::new(false),
    factor_x_ppm: AtomicU32::new(1_000_000),
    factor_y_ppm: AtomicU32::new(1_000_000),
    fire_button: AtomicU8::new(0),
    hook_thread: AtomicU32::new(0),
    raw_seen: AtomicU32::new(0),
    raw_abs: AtomicU32::new(0),
    injected: AtomicU32::new(0),
    suppressed: AtomicU32::new(0),
    hook_installed: AtomicBool::new(false),
    sink_ok: AtomicBool::new(false),
    raw_registered: AtomicBool::new(false),
    sink_hwnd: std::sync::atomic::AtomicUsize::new(0),
    reregistros: AtomicU32::new(0),
};

impl Shared {
    pub fn set_factors(&self, x: f32, y: f32) {
        self.factor_x_ppm.store((x * PPM) as u32, Ordering::Relaxed);
        self.factor_y_ppm.store((y * PPM) as u32, Ordering::Relaxed);
    }

    pub fn factors(&self) -> (f32, f32) {
        (
            self.factor_x_ppm.load(Ordering::Relaxed) as f32 / PPM,
            self.factor_y_ppm.load(Ordering::Relaxed) as f32 / PPM,
        )
    }

    pub fn set_fire_button(&self, button: FireButton) {
        let index = FireButton::ALL
            .iter()
            .position(|b| *b == button)
            .unwrap_or(0) as u8;
        self.fire_button.store(index, Ordering::Relaxed);
    }

    pub fn fire_button(&self) -> FireButton {
        let index = self.fire_button.load(Ordering::Relaxed) as usize;
        FireButton::ALL[index.min(FireButton::ALL.len() - 1)]
    }
}
