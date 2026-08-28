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

/// Applies a per-axis factor to integer mouse deltas, carrying the fractional
/// remainder across events so slow movement is not truncated to zero.
pub struct Scaler {
    factor_x: f32,
    factor_y: f32,
    carry_x: f32,
    carry_y: f32,
}

impl Scaler {
    pub fn new(factor_x: f32, factor_y: f32) -> Self {
        Self {
            factor_x,
            factor_y,
            carry_x: 0.0,
            carry_y: 0.0,
        }
    }

    pub fn scale(&mut self, dx: i32, dy: i32) -> (i32, i32) {
        let wanted_x = dx as f32 * self.factor_x + self.carry_x;
        let wanted_y = dy as f32 * self.factor_y + self.carry_y;
        let out_x = wanted_x.trunc() as i32;
        let out_y = wanted_y.trunc() as i32;
        self.carry_x = wanted_x - out_x as f32;
        self.carry_y = wanted_y - out_y as f32;
        (out_x, out_y)
    }

    /// Applies new factors while the hook keeps running, so editing the panel takes
    /// effect without restarting the script.
    pub fn set_factors(&mut self, factor_x: f32, factor_y: f32) {
        self.factor_x = factor_x;
        self.factor_y = factor_y;
    }

    /// Drops the accumulated remainder. Called when the fire button is released so the
    /// first event of the next burst does not carry stale fraction.
    pub fn reset(&mut self) {
        self.carry_x = 0.0;
        self.carry_y = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn set_factors_takes_effect_on_the_next_event() {
        let mut scaler = Scaler::new(1.0, 1.0);
        scaler.set_factors(2.0, 3.0);
        assert_eq!(scaler.scale(10, 10), (20, 30));
    }

    #[test]
    fn reset_drops_carry_so_the_next_burst_does_not_jump() {
        let mut scaler = Scaler::new(0.5, 0.5);
        scaler.scale(1, 1); // leaves 0.5 of carry on each axis
        scaler.reset();
        assert_eq!(scaler.scale(1, 1), (0, 0));
    }

    #[test]
    fn slow_movement_is_not_swallowed_by_truncation() {
        let mut scaler = Scaler::new(0.5, 0.5);
        let total: i32 = (0..10).map(|_| scaler.scale(1, 0).0).sum();
        assert_eq!(total, 5);
    }
}
