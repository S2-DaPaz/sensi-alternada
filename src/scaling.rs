//! Pure sensitivity-scaling logic: DPI ratio plus fractional remainder carry.

/// Slowest the aim may get while holding fire. Zero would freeze it entirely.
pub const MIN_FACTOR: f32 = 0.01;
/// Fastest the aim may get. Guards against a typo flinging the cursor across the desk.
pub const MAX_FACTOR: f32 = 10.0;

/// Scale factor to apply while the fire button is held.
pub fn factor_from_dpi(base_dpi: u32, shooting_dpi: u32) -> f32 {
    if base_dpi == 0 {
        return 1.0;
    }
    (shooting_dpi as f32 / base_dpi as f32).clamp(MIN_FACTOR, MAX_FACTOR)
}

/// Maps a screen coordinate to the 0..65535 space `MOUSEEVENTF_ABSOLUTE` expects.
///
/// The scale is over `size - 1`, not `size`: the last addressable pixel has to reach the
/// top of the range. Dividing by `size` shrinks every injection slightly, and because the
/// pointer is driven by repeated injections the shortfall accumulates into visible drift.
pub fn to_absolute(position: i32, origin: i32, size: i32) -> i32 {
    let span = (size - 1).max(1) as i64;
    ((position - origin) as i64 * 65535 / span) as i32
}

/// Where the pointer should be, in floating point. Only this program moves it while
/// scaling, so the fraction lives here and is never compared against the real cursor —
/// which is what keeps it from racing its own injections.
pub struct VirtualCursor {
    x: f64,
    y: f64,
}

impl VirtualCursor {
    pub fn seeded_at(x: i32, y: i32) -> Self {
        Self {
            x: x as f64,
            y: y as f64,
        }
    }

    /// Takes one raw device delta and returns where the pointer should now be.
    pub fn advance(&mut self, dx: i32, dy: i32, factor_x: f32, factor_y: f32) -> (i32, i32) {
        self.x += dx as f64 * factor_x as f64;
        self.y += dy as f64 * factor_y as f64;
        (self.x.round() as i32, self.y.round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_absolute_maps_the_last_pixel_to_the_top_of_the_range() {
        assert_eq!(to_absolute(0, 0, 1920), 0);
        assert_eq!(to_absolute(1919, 0, 1920), 65535);
    }

    #[test]
    fn slow_movement_moves_the_virtual_cursor_instead_of_vanishing() {
        let mut cursor = VirtualCursor::seeded_at(500, 500);
        let mut last = (0, 0);
        for _ in 0..10 {
            last = cursor.advance(1, 0, 0.5, 0.5);
        }
        assert_eq!(last, (505, 500));
    }

    #[test]
    fn factor_is_ratio_of_shooting_dpi_to_base() {
        assert_eq!(factor_from_dpi(800, 400), 0.5);
    }

    #[test]
    fn zero_base_dpi_falls_back_to_no_scaling() {
        assert_eq!(factor_from_dpi(0, 400), 1.0);
    }

    #[test]
    fn factor_is_clamped_to_a_usable_range() {
        // A typo in the panel must never freeze the aim nor fling the cursor.
        assert_eq!(factor_from_dpi(800, 0), MIN_FACTOR);
        assert_eq!(factor_from_dpi(1, 999_999), MAX_FACTOR);
    }
}
