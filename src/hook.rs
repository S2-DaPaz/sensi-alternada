//! Duas APIs com papéis separados, que é a única combinação que funciona no Windows:
//!
//! * `WH_MOUSE_LL` **engole** o movimento físico. Só o hook consegue suprimir.
//! * **Raw Input** entrega o delta cru do dispositivo. Só ele diz quanto o mouse andou.
//!
//! A tentação é derivar o delta da posição de cursor que o hook entrega. Medido em
//! 27/08/2026, com 400 px injetados: o raw input reportou 400, e o hook reportou 700,
//! depois 100, depois **−150** para o mesmo movimento. A posição de cursor chega
//! acelerada pelo Windows e disputada pelas nossas próprias reinjeções assíncronas.
//!
//! É o mesmo princípio do RawAccel, que transforma `LastX`/`LastY` no pacote do driver e
//! nunca toca na posição do cursor. Daí vem também o filtro `MOUSE_MOVE_ABSOLUTE`: é como
//! se ignoram as injeções deste programa, que voltam pelo raw input marcadas como
//! absolutas.

use std::cell::RefCell;
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOD_ALT, MOD_CONTROL, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_MOVE_NOCOALESCE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, RegisterHotKey, SendInput,
};
use windows::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT,
    RIDEV_INPUTSINK, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, GetCursorPos, GetMessageW, GetSystemMetrics, HHOOK,
    HWND_MESSAGE, MSG, MSLLHOOKSTRUCT, PostThreadMessageW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE_LL,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_HOTKEY, WM_INPUT, WM_MOUSEMOVE,
};
use windows::core::w;

use crate::fire_button::Transition;
use crate::scaling::{VirtualCursor, to_absolute};
use crate::shared::SHARED;

/// Stamped into `dwExtraInfo` so the hook can tell our injection from a real movement.
const TAG: usize = 0x5E_1A_17_00;

/// `MOUSE_MOVE_ABSOLUTE` in `RAWMOUSE::usFlags`.
const MOUSE_MOVE_ABSOLUTE: u16 = 0x0001;

/// `RIM_TYPEMOUSE`.
const RIM_TYPEMOUSE: u32 = 0;

/// Ask the hook thread to install or remove the hook. `wparam` carries 1 or 0.
const WM_APP_SET_ENABLED: u32 = WM_APP + 1;

const HOTKEY_TOGGLE: i32 = 1;
const VK_S: u32 = 0x53;

thread_local! {
    /// Owned by the hook thread alone — the hook callback and the raw-input handling both
    /// run there, so no locking is needed.
    static STATE: RefCell<HookState> = RefCell::new(HookState::new());
}

struct HookState {
    cursor: VirtualCursor,
    holding: bool,
}

impl HookState {
    fn new() -> Self {
        Self {
            cursor: VirtualCursor::seeded_at(0, 0),
            holding: false,
        }
    }
}

/// True while the physical movement must be swallowed and replaced by a scaled one.
fn scaling() -> bool {
    SHARED.target_focused.load(Ordering::Relaxed) && STATE.with(|s| s.borrow().holding)
}

/// Starts the thread that owns the hook, the raw-input sink and the global hotkey.
pub fn spawn() {
    std::thread::spawn(|| unsafe {
        SHARED
            .hook_thread
            .store(GetCurrentThreadId(), Ordering::SeqCst);

        let module = GetModuleHandleW(None).unwrap_or_default();

        // A message-only window on the system "STATIC" class, so no class registration is
        // needed. Raw input needs a window to deliver WM_INPUT to; the messages are read
        // straight from the loop below instead of through a window procedure.
        if let Ok(sink) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(module.into()),
            None,
        ) {
            SHARED.sink_ok.store(true, Ordering::SeqCst);
            SHARED.sink_hwnd.store(sink.0 as usize, Ordering::SeqCst);
            let device = RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x02, // mouse
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: sink,
            };
            let ok =
                RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32);
            SHARED.raw_registered.store(ok.is_ok(), Ordering::SeqCst);
        }

        let _ = RegisterHotKey(None, HOTKEY_TOGGLE, MOD_CONTROL | MOD_ALT, VK_S);

        let mut hook: Option<HHOOK> = None;
        let mut msg = MSG::default();

        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_INPUT => on_raw_input(msg.lParam),
                WM_APP_SET_ENABLED => apply(&mut hook, msg.wParam.0 != 0),
                WM_HOTKEY if msg.wParam.0 as i32 == HOTKEY_TOGGLE => {
                    let want = !SHARED.enabled.load(Ordering::SeqCst);
                    SHARED.enabled.store(want, Ordering::SeqCst);
                    apply(&mut hook, want);
                }
                _ => {}
            }
        }
    });
}

/// One raw packet from the mouse. This is the only place movement is measured.
unsafe fn on_raw_input(lparam: LPARAM) {
    unsafe {
        // Conta ANTES da guarda: e o que separa "WM_INPUT nao chega" de "chega e e
        // descartado". Contar depois da guarda confunde as duas causas.
        SHARED.raw_seen.fetch_add(1, Ordering::Relaxed);
        if !scaling() {
            return;
        }
        let mut raw = RAWINPUT::default();
        let mut size = std::mem::size_of::<RAWINPUT>() as u32;
        let read = GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_INPUT,
            Some(&mut raw as *mut _ as *mut _),
            &mut size,
            std::mem::size_of::<RAWINPUTHEADER>() as u32,
        );
        if read == u32::MAX || raw.header.dwType != RIM_TYPEMOUSE {
            return;
        }

        let mouse = raw.data.mouse;
        // Absolute packets are our own injections coming back, or a tablet. Either way
        // there is no relative count to scale. Same filter the RawAccel driver applies.
        if mouse.usFlags.0 & MOUSE_MOVE_ABSOLUTE != 0 {
            SHARED.raw_abs.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if mouse.lLastX == 0 && mouse.lLastY == 0 {
            return;
        }

        let (fx, fy) = SHARED.factors();
        let (x, y) = STATE.with(|s| {
            s.borrow_mut()
                .cursor
                .advance(mouse.lLastX, mouse.lLastY, fx, fy)
        });
        SHARED.injected.fetch_add(1, Ordering::Relaxed);
        inject_absolute(x, y);
    }
}

/// Installs or removes the hook. Removing it leaves zero footprint on the system, which
/// is what makes "back to normal" literal rather than a promise.
unsafe fn apply(hook: &mut Option<HHOOK>, enabled: bool) {
    unsafe {
        match (enabled, *hook) {
            (true, None) => {
                let module = GetModuleHandleW(None).unwrap_or_default();
                if let Ok(handle) =
                    SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), Some(module.into()), 0)
                {
                    *hook = Some(handle);
                    SHARED.hook_installed.store(true, Ordering::SeqCst);
                }
            }
            (false, Some(handle)) => {
                let _ = UnhookWindowsHookEx(handle);
                *hook = None;
                SHARED.hook_installed.store(false, Ordering::SeqCst);
                STATE.with(|s| s.borrow_mut().holding = false);
                SHARED.holding_fire.store(false, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

/// Reinscreve o mouse no raw input apontando para a nossa janela.
///
/// A inscricao de raw input e **por processo e por uso**: quem chamar por ultimo fica com
/// as mensagens. O winit, por baixo do eframe, se inscreve para a janela dele e rouba as
/// nossas — por isso a inscricao unica no arranque nao basta.
pub fn reregister_raw_input() -> bool {
    let raw = SHARED.sink_hwnd.load(Ordering::SeqCst);
    if raw == 0 {
        return false;
    }
    let device = RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x02,
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: windows::Win32::Foundation::HWND(raw as *mut _),
    };
    let ok = unsafe {
        RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32).is_ok()
    };
    if ok {
        SHARED.reregistros.fetch_add(1, Ordering::Relaxed);
    }
    ok
}

/// Tells the hook thread to install or remove the hook.
pub fn request_enabled(enabled: bool) {
    // A thread do hook pode ainda nao ter publicado o id: o painel chama isto no arranque.
    // Desistir em silencio aqui deixava o hook nunca instalado quando a configuracao ja
    // vinha ligada do disco.
    let mut thread = SHARED.hook_thread.load(Ordering::SeqCst);
    for _ in 0..100 {
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
        let _ = PostThreadMessageW(
            thread,
            WM_APP_SET_ENABLED,
            WPARAM(enabled as usize),
            LPARAM(0),
        );
    }
}

/// The hook has exactly two jobs: track the trigger, and swallow the physical movement.
/// It deliberately reads nothing from `MSLLHOOKSTRUCT::pt`.
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code < 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let message = wparam.0 as u32;

        if info.dwExtraInfo == TAG {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        if let Some(transition) = SHARED.fire_button().transition(message, info.mouseData) {
            let pressed = transition == Transition::Pressed;
            STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.holding = pressed;
                if pressed {
                    // The burst starts wherever the pointer actually is; nothing carries
                    // over from the previous one.
                    reregister_raw_input();
                    let mut at = POINT { x: 0, y: 0 };
                    let _ = GetCursorPos(&mut at);
                    state.cursor = VirtualCursor::seeded_at(at.x, at.y);
                }
            });
            SHARED.holding_fire.store(pressed, Ordering::Relaxed);
            return CallNextHookEx(None, code, wparam, lparam);
        }

        if message == WM_MOUSEMOVE && scaling() {
            // Non-zero swallows it. Measured on 2026-08-26: this does not blind Raw Input,
            // which is exactly why the two APIs can be combined.
            SHARED.suppressed.fetch_add(1, Ordering::Relaxed);
            return LRESULT(1);
        }

        CallNextHookEx(None, code, wparam, lparam)
    }
}

unsafe fn inject_absolute(x: i32, y: i32) {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: to_absolute(x, left, width),
                    dy: to_absolute(y, top, height),
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE
                        | MOUSEEVENTF_ABSOLUTE
                        | MOUSEEVENTF_VIRTUALDESK
                        | MOUSEEVENTF_MOVE_NOCOALESCE,
                    time: 0,
                    dwExtraInfo: TAG,
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}
