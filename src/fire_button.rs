//! Which physical mouse button acts as the trigger, and how to recognise its
//! press/release inside the low-level hook.

use windows::Win32::UI::WindowsAndMessaging::{
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_XBUTTONDOWN, WM_XBUTTONUP,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FireButton {
    Left,
    Right,
    Middle,
    Side1,
    Side2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Pressed,
    Released,
}

impl FireButton {
    /// Reads one hook event. `None` means the event is not this button.
    pub fn transition(self, message: u32, mouse_data: u32) -> Option<Transition> {
        // The X button id is in the high word; the low word carries wheel data.
        let x_id = mouse_data >> 16;
        match (self, message) {
            (FireButton::Left, WM_LBUTTONDOWN) => Some(Transition::Pressed),
            (FireButton::Left, WM_LBUTTONUP) => Some(Transition::Released),
            (FireButton::Right, WM_RBUTTONDOWN) => Some(Transition::Pressed),
            (FireButton::Right, WM_RBUTTONUP) => Some(Transition::Released),
            (FireButton::Middle, WM_MBUTTONDOWN) => Some(Transition::Pressed),
            (FireButton::Middle, WM_MBUTTONUP) => Some(Transition::Released),
            (FireButton::Side1, WM_XBUTTONDOWN) if x_id == 1 => Some(Transition::Pressed),
            (FireButton::Side1, WM_XBUTTONUP) if x_id == 1 => Some(Transition::Released),
            (FireButton::Side2, WM_XBUTTONDOWN) if x_id == 2 => Some(Transition::Pressed),
            (FireButton::Side2, WM_XBUTTONUP) if x_id == 2 => Some(Transition::Released),
            _ => None,
        }
    }

    pub const ALL: [FireButton; 5] = [
        FireButton::Left,
        FireButton::Right,
        FireButton::Middle,
        FireButton::Side1,
        FireButton::Side2,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FireButton::Left => "Esquerdo",
            FireButton::Right => "Direito",
            FireButton::Middle => "Meio",
            FireButton::Side1 => "Lateral 1",
            FireButton::Side2 => "Lateral 2",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The X button id lives in the HIGH word of mouseData. Reading the low word
    // instead makes side buttons silently never fire.
    #[test]
    fn side_buttons_are_told_apart_by_the_high_word_of_mouse_data() {
        const XBUTTON1_DATA: u32 = 0x0001_0000;
        const XBUTTON2_DATA: u32 = 0x0002_0000;

        assert_eq!(
            FireButton::Side1.transition(WM_XBUTTONDOWN, XBUTTON1_DATA),
            Some(Transition::Pressed)
        );
        assert_eq!(
            FireButton::Side1.transition(WM_XBUTTONDOWN, XBUTTON2_DATA),
            None
        );
    }

    #[test]
    fn left_button_down_is_a_press() {
        assert_eq!(
            FireButton::Left.transition(WM_LBUTTONDOWN, 0),
            Some(Transition::Pressed)
        );
    }
}
