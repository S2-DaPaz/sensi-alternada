//! Colours derived from the two the user picks, so no choice can make the panel
//! unreadable.

pub const INK_LIGHT: [u8; 3] = [236, 239, 244];
pub const INK_DARK: [u8; 3] = [18, 22, 28];

/// Perceived brightness, sRGB weighted. Green carries most of it, blue almost none —
/// a plain average would call a saturated blue "light" and pick unreadable dark ink.
pub fn luminance(colour: [u8; 3]) -> f32 {
    0.2126 * colour[0] as f32 + 0.7152 * colour[1] as f32 + 0.0722 * colour[2] as f32
}

/// Text colour that stays legible on `background`.
pub fn ink_for(background: [u8; 3]) -> [u8; 3] {
    if luminance(background) > 140.0 {
        INK_DARK
    } else {
        INK_LIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A plain (r+g+b)/3 average calls bright green "dark" and puts light text on it.
    // Green carries most of the perceived brightness; the weights are the whole point.
    #[test]
    fn a_saturated_green_background_is_treated_as_bright() {
        assert_eq!(ink_for([0, 255, 0]), INK_DARK);
    }

    #[test]
    fn ink_flips_with_the_brightness_of_the_background() {
        assert_eq!(ink_for([250, 249, 246]), INK_DARK, "fundo claro");
        assert_eq!(ink_for([20, 22, 26]), INK_LIGHT, "fundo escuro");
    }
}
