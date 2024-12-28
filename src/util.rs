//! Utility traits and functions.
//!
//! This module provides general-purpose utilities including:
//! * Type conversion traits
//! * Numeric value handling

/// Trait for converting numeric values to `f32` with controlled truncation.
///
/// Provides safe conversion to `f32` by:
/// * Clamping values to `f32` range
/// * Preventing infinity values
/// * Preventing NaN values
///
/// # Example
///
/// ```rust
/// use pleezer::util::ToF32;
///
/// let large_value: f64 = 1e308;
/// let clamped: f32 = large_value.to_f32_lossy();
/// assert!(clamped == f32::MAX);
/// ```
pub trait ToF32 {
    /// Converts a value to `f32`, clamping to prevent invalid results.
    ///
    /// Values outside the `f32` range are clamped to the nearest valid value:
    /// * Values > `f32::MAX` become `f32::MAX`
    /// * Values < `f32::MIN` become `f32::MIN`
    ///
    /// # Returns
    ///
    /// A valid `f32` value within the supported range.
    fn to_f32_lossy(self) -> f32;
}

/// Implements conversion from `f64` to `f32` with range clamping.
///
/// Clamps the value to the valid `f32` range before truncating:
/// * `f64` values beyond `f32::MAX` become `f32::MAX`
/// * `f64` values beyond `f32::MIN` become `f32::MIN`
///
/// # Example
///
/// ```rust
/// use pleezer::util::ToF32;
///
/// let too_large = f64::MAX;
/// let clamped = too_large.to_f32_lossy();
/// assert!(clamped == f32::MAX);
/// ```
impl ToF32 for f64 {
    #[expect(clippy::cast_possible_truncation)]
    fn to_f32_lossy(self) -> f32 {
        self.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
    }
}

/// Implements conversion from `u64` to `f32` with range clamping.
///
/// Clamps the value to the valid `f32` range before truncating:
/// * `u64` values beyond `f32::MAX` become `f32::MAX`
/// * `u64` values below `f32::MIN` (0) are impossible due to unsigned type
///
/// # Example
///
/// ```rust
/// use pleezer::util::ToF32;
///
/// let too_large = u64::MAX;
/// let clamped = too_large.to_f32_lossy();
/// assert!(clamped == f32::MAX);
/// ```
impl ToF32 for u64 {
    #[expect(clippy::cast_possible_truncation)]
    #[expect(clippy::cast_precision_loss)]
    #[expect(clippy::cast_sign_loss)]
    fn to_f32_lossy(self) -> f32 {
        if self > f32::MAX as u64 {
            f32::MAX
        } else {
            self as f32
        }
    }
}

/// Implements conversion from `u128` to `f32` with range clamping.
///
/// Clamps the value to the valid `f32` range before truncating:
/// * `u128` values beyond `f32::MAX` become `f32::MAX`
/// * `u128` values below `f32::MIN` (0) are impossible due to unsigned type
///
/// # Example
///
/// ```rust
/// use pleezer::util::ToF32;
///
/// let too_large = u128::MAX;
/// let clamped = too_large.to_f32_lossy();
/// assert!(clamped == f32::MAX);
/// ```
impl ToF32 for u128 {
    #[expect(clippy::cast_possible_truncation)]
    #[expect(clippy::cast_precision_loss)]
    #[expect(clippy::cast_sign_loss)]
    fn to_f32_lossy(self) -> f32 {
        if self > f32::MAX as u128 {
            f32::MAX
        } else {
            self as f32
        }
    }
}
