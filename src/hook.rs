//! Low-level mouse hook: swallows the physical move and re-injects the scaled one.
//!
//! Two details are load-bearing:
//!
//! * Our own injection is tagged in `dwExtraInfo`. Without the tag the hook reprocesses
//!   what it just injected and feeds back on itself.
//! * The re-injection is **absolute**, which bypasses the Windows pointer speed and
//!   "enhance pointer precision". A relative injection would be scaled a second time by
//!   the OS and the factor would stop meaning what the panel says.

use std::cell::RefCell;
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOD_ALT, MOD_CONTROL, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_MOVE_NOCOALESCE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, RegisterHotKey, SendInput,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, GetSystemMetrics, HHOOK, MSG, MSLLHOOKSTRUCT, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL, WM_APP, WM_HOTKEY, WM_MOUSEMOVE,
};

use crate::fire_button::Transition;
use crate::scaling::Scaler;
use crate::shared::SHARED;

/// Stamped into `dwExtraInfo` so the hook can tell our injection from a real movement.
const TAG: usize = 0x5E_1A_17_00;

/// Ask the hook thread to install or remove the hook. `wparam` carries 1 or 0.
const WM_APP_SET_ENABLED: u32 = WM_APP + 1;

const HOTKEY_TOGGLE: i32 = 1;
const VK_S: u32 = 0x53;

thread_local! {
    /// Only the hook thread touches these, so no locking is needed.
    static STATE: RefCell<HookState> = RefCell::new(HookState::new());
}

struct HookState {
    scaler: Scaler,
    /// Where the cursor actually sits. We own it while suppressing.
    last_pos: POINT,
    holding: bool,
}

impl HookState {
    fn new() -> Self {
        Self {
            scaler: Scaler::new(1.0, 1.0),
            last_pos: POINT { x: 0, y: 0 },
            holding: false,
        }
    }
}

/// Starts the thread that owns the hook and the global hotkey. Runs for the life of the
/// process.
pub fn spawn() {
    std::thread::spawn(|| unsafe {
        SHARED
            .hook_thread
            .store(GetCurrentThreadId(), Ordering::SeqCst);

        // Realises the thread message queue before anyone posts to it.
        let mut msg = MSG::default();
        let _ = RegisterHotKey(None, HOTKEY_TOGGLE, MOD_CONTROL | MOD_ALT, VK_S);

        let mut hook: Option<HHOOK> = None;

        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_APP_SET_ENABLED => {
                    let want = msg.wParam.0 != 0;
                    apply(&mut hook, want);
                }
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

/// Tells the hook thread to install or remove the hook.
pub fn request_enabled(enabled: bool) {
    let thread = SHARED.hook_thread.load(Ordering::SeqCst);
    if thread == 0 {
        return;
    }
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            thread,
            WM_APP_SET_ENABLED,
            WPARAM(enabled as usize),
            LPARAM(0),
        );
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code < 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let message = wparam.0 as u32;

        // Our own injection: let it through, and remember where it put the cursor.
        if info.dwExtraInfo == TAG {
            STATE.with(|s| s.borrow_mut().last_pos = info.pt);
            return CallNextHookEx(None, code, wparam, lparam);
        }

        if let Some(transition) = SHARED.fire_button().transition(message, info.mouseData) {
            STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.holding = transition == Transition::Pressed;
                if !state.holding {
                    // Stale fraction must not leak into the next burst.
                    state.scaler.reset();
                }
            });
            SHARED
                .holding_fire
                .store(transition == Transition::Pressed, Ordering::Relaxed);
            return CallNextHookEx(None, code, wparam, lparam);
        }

        if message != WM_MOUSEMOVE {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        let scaling =
            SHARED.target_focused.load(Ordering::Relaxed) && STATE.with(|s| s.borrow().holding);

        if !scaling {
            STATE.with(|s| s.borrow_mut().last_pos = info.pt);
            return CallNextHookEx(None, code, wparam, lparam);
        }

        let target = STATE.with(|s| {
            let mut state = s.borrow_mut();
            let (fx, fy) = SHARED.factors();
            state.scaler.set_factors(fx, fy);

            let dx = info.pt.x - state.last_pos.x;
            let dy = info.pt.y - state.last_pos.y;
            let (sx, sy) = state.scaler.scale(dx, dy);

            state.last_pos = POINT {
                x: state.last_pos.x + sx,
                y: state.last_pos.y + sy,
            };
            state.last_pos
        });

        inject_absolute(target);

        // Non-zero swallows the physical movement. Measured on 2026-08-26: this does not
        // blind Raw Input, only the cursor path.
        LRESULT(1)
    }
}

unsafe fn inject_absolute(target: POINT) {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1);

        let nx = ((target.x - left) as i64 * 65535 / width as i64) as i32;
        let ny = ((target.y - top) as i64 * 65535 / height as i64) as i32;

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: nx,
                    dy: ny,
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
