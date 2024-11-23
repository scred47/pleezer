pub trait ToF32 {
    /// Truncates the value to an `f32` preventing infinities and NaNs.
    fn to_f32_lossy(self) -> f32;
}

impl ToF32 for f64 {
    /// Truncates the `f64` to an `f32` preventing infinities and NaNs.
    #[expect(clippy::cast_possible_truncation)]
    fn to_f32_lossy(self) -> f32 {
        self.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
    }
}
