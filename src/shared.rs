//! Estado que o painel escreve e a thread do hook lê.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, Ordering};

use crate::brand::Brand;
use crate::fire_button::FireButton;

pub struct Shared {
    pub enabled: AtomicBool,
    pub target_focused: AtomicBool,
    pub holding_fire: AtomicBool,
    /// O motor da marca escolhida consegue mesmo trocar a DPI.
    pub engine_usable: AtomicBool,
    /// O mouse tem a feature 0x2202 e aceita DPI diferente em X e Y.
    pub per_axis: AtomicBool,
    dpi_base: AtomicU16,
    dpi_shooting: AtomicU16,
    fire_button: AtomicU8,
    brand: AtomicU8,
    /// Thread do laço de mensagens que fala com o mouse, publicada quando ela sobe.
    pub hook_thread: AtomicU32,
    /// Última mensagem do motor, para o painel dizer o que houve em vez de ficar mudo.
    status: Mutex<String>,
}

pub static SHARED: Shared = Shared {
    enabled: AtomicBool::new(false),
    target_focused: AtomicBool::new(false),
    holding_fire: AtomicBool::new(false),
    engine_usable: AtomicBool::new(false),
    per_axis: AtomicBool::new(false),
    dpi_base: AtomicU16::new(800),
    dpi_shooting: AtomicU16::new(400),
    fire_button: AtomicU8::new(0),
    brand: AtomicU8::new(0),
    hook_thread: AtomicU32::new(0),
    status: Mutex::new(String::new()),
};

impl Shared {
    pub fn set_dpi(&self, base: u16, shooting: u16) {
        self.dpi_base.store(base, Ordering::Relaxed);
        self.dpi_shooting.store(shooting, Ordering::Relaxed);
    }

    pub fn dpi_base(&self) -> u16 {
        self.dpi_base.load(Ordering::Relaxed)
    }

    pub fn dpi_shooting(&self) -> u16 {
        self.dpi_shooting.load(Ordering::Relaxed)
    }

    pub fn set_brand(&self, brand: Brand) {
        let index = Brand::ALL.iter().position(|b| *b == brand).unwrap_or(0) as u8;
        self.brand.store(index, Ordering::Relaxed);
    }

    pub fn brand(&self) -> Brand {
        let index = self.brand.load(Ordering::Relaxed) as usize;
        Brand::ALL[index.min(Brand::ALL.len() - 1)]
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

    pub fn report(&self, message: impl Into<String>) {
        if let Ok(mut slot) = self.status.lock() {
            *slot = message.into();
        }
    }

    pub fn last_message(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}
