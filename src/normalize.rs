//! Audio normalization through feedforward limiting.
//!
//! This module implements a feedforward limiter in the log domain, based on:
//! Giannoulis, D., Massberg, M., & Reiss, J.D. (2012). Digital Dynamic
//! Range Compressor Design—A Tutorial and Analysis. Journal of The Audio
//! Engineering Society, 60, 399-408.
//!
//! Features:
//! * Soft-knee limiting for natural sound
//! * Decoupled peak detection
//! * Configurable attack/release times
//! * CPU-efficient processing
//!
//! # Architecture
//!
//! The limiter processes audio in these steps:
//! 1. Initial gain stage
//! 2. Half-wave rectification and dB conversion
//! 3. Soft-knee gain computation
//! 4. Smoothed peak detection
//! 5. Gain reduction application
//!
//! # Example
//!
//! ```no_run
//! use std::time::Duration;
//! use pleezer::normalize::normalize;
//!
//! // Configure limiter
//! let normalized = normalize(
//!     source,
//!     1.0,             // Unity gain
//!     -6.0,            // Threshold (dB)
//!     12.0,            // Knee width (dB)
//!     Duration::from_millis(5),    // Attack time
//!     Duration::from_millis(100),  // Release time
//! );
//! ```

use std::time::Duration;

use rodio::{source::SeekError, Sample, Source};

use crate::util::{self, ToF32, ZERO_DB};

/// Creates a normalized audio filter with configurable limiting.
///
/// # Arguments
///
/// * `input` - Audio source to process
/// * `ratio` - Initial gain scaling (1.0 = unity)
/// * `threshold` - Level where limiting begins (dB)
/// * `knee_width` - Range over which limiting gradually increases (dB)
/// * `attack` - Time to respond to level increases
/// * `release` - Time to recover after level decreases
///
/// # Returns
///
/// A `Normalize` filter that processes the input audio through the limiter.
pub fn normalize<I>(
    input: I,
    ratio: f32,
    threshold: f32,
    knee_width: f32,
    attack: Duration,
    release: Duration,
) -> Normalize<I>
where
    I: Source,
    I::Item: Sample,
{
    let sample_rate = input.sample_rate();
    let channels = input.channels() as usize;

    let attack = duration_to_coefficient(attack, sample_rate);
    let release = duration_to_coefficient(release, sample_rate);

    Normalize {
        input,

        ratio,
        threshold,
        knee_width,
        attack,
        release,

        normalisation_integrators: vec![ZERO_DB; channels],
        normalisation_peaks: vec![ZERO_DB; channels],
        position: 0,
    }
}

/// Converts a time duration to a smoothing coefficient.
///
/// Used for attack/release filtering:
/// * Longer times = higher coefficients = slower response
/// * Shorter times = lower coefficients = faster response
///
/// # Arguments
///
/// * `duration` - Desired response time
/// * `sample_rate` - Audio sample rate in Hz
///
/// # Returns
///
/// Smoothing coefficient in the range [0.0, 1.0]
#[must_use]
fn duration_to_coefficient(duration: Duration, sample_rate: u32) -> f32 {
    f32::exp(-1.0 / (duration.as_secs_f32() * sample_rate.to_f32_lossy()))
}

/// Audio filter that applies normalization through feedforward limiting.
///
/// Processing stages:
/// 1. Initial gain scaling by `ratio`
/// 2. Peak detection above `threshold`
/// 3. Soft-knee limiting over `knee_width`
/// 4. Smoothing with `attack`/`release` filtering
///
/// # Type Parameters
///
/// * `I` - Input audio source type
#[derive(Clone, Debug)]
pub struct Normalize<I>
where
    I: Source,
    I::Item: Sample,
{
    /// Input audio source
    input: I,

    /// Initial gain scaling factor (1.0 = unity)
    ratio: f32,

    /// Level where limiting begins (dB)
    threshold: f32,

    /// Range for gradual limiting transition (dB)
    knee_width: f32,

    /// Attack smoothing coefficient
    attack: f32,

    /// Release smoothing coefficient
    release: f32,

    /// Per-channel peak detector integrator states (dB)
    normalisation_integrators: Vec<f32>,

    /// Per-channel smoothed peak levels (dB)
    normalisation_peaks: Vec<f32>,

    /// Current sample position for channel tracking
    position: usize,
}

impl<I> Normalize<I>
where
    I: Source,
    I::Item: Sample,
{
    /// Returns a reference to the inner audio source.
    ///
    /// Useful for inspecting source properties without consuming the filter.
    #[inline]
    pub fn inner(&self) -> &I {
        &self.input
    }

    /// Returns a mutable reference to the inner audio source.
    ///
    /// Enables modifying source properties while maintaining the filter.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.input
    }

    /// Consumes the filter and returns the inner audio source.
    ///
    /// Useful when normalization is no longer needed but source should continue.
    #[inline]
    pub fn into_inner(self) -> I {
        self.input
    }
}

impl<I> Iterator for Normalize<I>
where
    I: Source,
    I::Item: Sample,
{
    type Item = I::Item;

    /// Processes the next audio sample through the limiter.
    ///
    /// Processing steps:
    /// 1. Apply initial gain scaling
    /// 2. Convert to dB and detect peaks
    /// 3. Apply soft-knee limiting curve
    /// 4. Smooth response with attack/release
    /// 5. Apply gain reduction
    ///
    /// Returns None when input source is exhausted.
    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        let sample = self.input.next()?;

        let channel = self.position % self.input.channels() as usize;
        self.position = self.position.wrapping_add(1);

        // step 0: apply gain stage
        sample.amplify(self.ratio);

        // zero-cost shorthands
        let threshold_db = self.threshold;
        let knee_db = self.knee_width;
        let attack_cf = self.attack;
        let release_cf = self.release;

        // Some tracks have samples that are precisely 0.0. That's silence
        // and we know we don't need to limit that, in which we can spare
        // the CPU cycles.
        //
        // Also, calling `ratio_to_db(0.0)` returns `inf` and would get the
        // peak detector stuck. Also catch the unlikely case where a sample
        // is decoded as `NaN` or some other non-normal value.
        let sample_f32 = sample.to_f32();

        let mut limiter_db = ZERO_DB;
        if sample_f32.is_normal() {
            // step 1-4: half-wave rectification and conversion into dB
            // and gain computer with soft knee and subtractor
            let bias_db = util::ratio_to_db(sample_f32.abs()) - threshold_db;
            let knee_boundary_db = bias_db * 2.0;

            if knee_boundary_db < -knee_db {
                limiter_db = ZERO_DB;
            } else if knee_boundary_db.abs() <= knee_db {
                // Textbook:
                // ```
                // ratio_to_db(sample.abs()) - (ratio_to_db(sample.abs()) -
                // bias_db + knee_db / 2.0).powi(2) / (2.0 * knee_db))
                // ```
                limiter_db = (knee_boundary_db + knee_db).powi(2) / (8.0 * knee_db);
            } else {
                // Textbook:
                // ```
                // ratio_to_db(sample.abs()) - threshold_db
                // ```
                // ...which is already our `bias_db`.
                limiter_db = bias_db;
            }
        }

        // Spare the CPU unless:
        // 1. the limiter is engaged, or
        // 2. we were in attack, or
        // 3. we were in release,
        // ...and that attack/release were not finished yet.
        if limiter_db > ZERO_DB
            || self.normalisation_integrators[channel] > ZERO_DB
            || self.normalisation_peaks[channel] > ZERO_DB
        {
            // step 5: smooth, decoupled peak detector
            //
            // Textbook:
            // ```
            // release_cf * self.normalisation_integrator + (1.0 - release_cf) * limiter_db
            // ```
            self.normalisation_integrators[channel] = f32::max(
                limiter_db,
                release_cf * self.normalisation_integrators[channel] - release_cf * limiter_db
                    + limiter_db,
            );

            // Textbook:
            // ```
            // attack_cf * self.normalisation_peak + (1.0 - attack_cf)
            // * self.normalisation_integrator
            // ```
            self.normalisation_peaks[channel] = attack_cf * self.normalisation_peaks[channel]
                - attack_cf * self.normalisation_integrators[channel]
                + self.normalisation_integrators[channel];

            // Find maximum peak across all channels
            let max_peak = self
                .normalisation_peaks
                .iter()
                .copied()
                .fold(ZERO_DB, f32::max);

            // steps 6-8: conversion into level and multiplication into gain stage
            sample.amplify(util::db_to_ratio(-max_peak));
        }

        Some(sample)
    }

    /// Provides size hints from the inner source.
    ///
    /// Used by collection operations for optimization.
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

/// Exact size iterator when inner source provides exact size.
impl<I> ExactSizeIterator for Normalize<I>
where
    I: Source + ExactSizeIterator,
    I::Item: Sample,
{
}

impl<I> Source for Normalize<I>
where
    I: Source,
    I::Item: Sample,
{
    /// Returns the number of samples in the current audio frame.
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    /// Returns the number of audio channels.
    #[inline]
    fn channels(&self) -> u16 {
        self.input.channels()
    }

    /// Returns the audio sample rate in Hz.
    #[inline]
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    /// Returns the total duration of the audio.
    ///
    /// Returns None for streams without known duration.
    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    /// Attempts to seek to the specified position.
    ///
    /// Also resets limiter state to prevent artifacts.
    #[inline]
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)?;

        self.normalisation_integrators = vec![ZERO_DB; self.channels() as usize];
        self.normalisation_peaks = vec![ZERO_DB; self.channels() as usize];
        self.position = 0;

        Ok(())
    }
}
