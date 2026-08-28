//! Detecta o gatilho e manda o mouse trocar de DPI.
//!
//! O hook de baixo nível aqui **não mexe em movimento nenhum** — ele só vê o botão passar.
//! Escalar o cursor foi o desenho anterior e não funcionava: medido em 28/08/2026, o modo
//! de tiro do BlueStacks lê as contagens cruas do dispositivo, não o cursor, e ignorava
//! qualquer coisa feita em user-mode. Trocar a DPI no firmware chega ao jogo.
//!
//! A troca de DPI leva ~4 ms e **não** roda dentro do callback do hook: ela é postada para
//! o laço de mensagens. Se rodasse ali, cada clique chegaria ao jogo 4 ms atrasado.

use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, RegisterHotKey};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HHOOK, MSG, MSLLHOOKSTRUCT, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL, WM_APP, WM_HOTKEY,
};

use crate::fire_button::Transition;
use crate::mouse::Mouse;
use crate::shared::SHARED;

/// Instalar ou remover o hook. `wparam` carrega 1 ou 0.
const WM_APP_SET_ENABLED: u32 = WM_APP + 1;
/// Trocar a DPI. `wparam` carrega o valor.
const WM_APP_SET_DPI: u32 = WM_APP + 2;

const HOTKEY_TOGGLE: i32 = 1;
const VK_S: u32 = 0x53;

pub fn spawn() {
    std::thread::spawn(|| unsafe {
        SHARED
            .hook_thread
            .store(GetCurrentThreadId(), Ordering::SeqCst);

        let mouse = Mouse::find();
        match &mouse {
            Some(m) => {
                SHARED.mouse_found.store(true, Ordering::SeqCst);
                SHARED.per_axis.store(m.per_axis, Ordering::SeqCst);
                match m.current_dpi() {
                    Some(dpi) => SHARED.report(format!("mouse encontrado · DPI atual {dpi}")),
                    None => SHARED.report("mouse encontrado, mas não respondeu a leitura de DPI"),
                }
            }
            None => SHARED.report("nenhum mouse Logitech com DPI programável encontrado"),
        }

        let _ = RegisterHotKey(None, HOTKEY_TOGGLE, MOD_CONTROL | MOD_ALT, VK_S);

        let mut hook: Option<HHOOK> = None;
        let mut msg = MSG::default();

        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_APP_SET_DPI => {
                    if let Some(m) = &mouse {
                        let dpi = msg.wParam.0 as u16;
                        match m.set_dpi(dpi) {
                            Ok(()) => SHARED.report(format!("DPI {dpi}")),
                            Err(e) => SHARED.report(e),
                        }
                    }
                }
                WM_APP_SET_ENABLED => {
                    let want = msg.wParam.0 != 0;
                    apply(&mut hook, want);
                    if !want {
                        restore_base();
                    }
                }
                WM_HOTKEY if msg.wParam.0 as i32 == HOTKEY_TOGGLE => {
                    let want = !SHARED.enabled.load(Ordering::SeqCst);
                    SHARED.enabled.store(want, Ordering::SeqCst);
                    apply(&mut hook, want);
                    if !want {
                        restore_base();
                    }
                }
                _ => {}
            }
        }
    });
}

/// Instala ou remove o hook. Removido, não sobra nada residente no sistema.
unsafe fn apply(hook: &mut Option<HHOOK>, enabled: bool) {
    unsafe {
        match (enabled, *hook) {
            (true, None) => {
                let module = GetModuleHandleW(None).unwrap_or_default();
                if let Ok(handle) =
                    SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), Some(module.into()), 0)
                {
                    *hook = Some(handle);
                }
            }
            (false, Some(handle)) => {
                let _ = UnhookWindowsHookEx(handle);
                *hook = None;
                SHARED.holding_fire.store(false, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

/// Desligar com o gatilho pressionado não pode deixar a DPI de tiro valendo no desktop.
fn restore_base() {
    request_dpi(SHARED.dpi_base());
}

pub fn request_enabled(enabled: bool) {
    post(WM_APP_SET_ENABLED, enabled as usize);
}

fn request_dpi(dpi: u16) {
    post(WM_APP_SET_DPI, dpi as usize);
}

fn post(message: u32, wparam: usize) {
    // A thread pode ainda não ter publicado o id: o painel chama isto no arranque.
    // Desistir em silêncio aqui deixava o hook nunca instalado.
    let mut thread = SHARED.hook_thread.load(Ordering::SeqCst);
    for _ in 0..200 {
        if thread != 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        thread = SHARED.hook_thread.load(Ordering::SeqCst);
    }
    if thread == 0 {
        return;
    }
    unsafe {
        let _ = PostThreadMessageW(thread, message, WPARAM(wparam), LPARAM(0));
    }
}

/// Único papel do hook: ver o gatilho. Nunca toca em movimento, nunca suprime nada.
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code < 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        if let Some(transition) = SHARED
            .fire_button()
            .transition(wparam.0 as u32, info.mouseData)
        {
            let pressed = transition == Transition::Pressed;
            SHARED.holding_fire.store(pressed, Ordering::Relaxed);
            if SHARED.target_focused.load(Ordering::Relaxed) {
                // Postado, não executado aqui: a escrita HID leva ~4 ms e atrasaria o clique.
                request_dpi(if pressed {
                    SHARED.dpi_shooting()
                } else {
                    SHARED.dpi_base()
                });
            }
        }

        CallNextHookEx(None, code, wparam, lparam)
    }
}
