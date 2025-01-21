//! Audio decoder implementation using Symphonia.
//!
//! This module provides a decoder that directly uses Symphonia's capabilities to:
//! * Support multiple formats (AAC/ADTS, FLAC, MP3, MP4, WAV)
//! * Enable format-specific seeking with proper error recovery
//! * Handle both constant and variable bitrate streams
//! * Process audio in floating point format
//!
//! # Audio Parameters
//!
//! The decoder detects and provides:
//! * Sample rate (defaults to 44.1 kHz if unspecified)
//! * Bits per sample (codec-dependent)
//! * Channel count (mono/stereo/multi-channel)
//! # Error Handling
//!
//! The decoder implements robust error recovery:
//! * Skips corrupted packets (up to 3 consecutive)
//! * Handles codec reset requests
//! * Recovers from seekable I/O errors
//! * Gracefully handles end of stream
//! * Ensures clean state by clearing buffers after any decoder error
//!
//! # Performance
//!
//! The decoder is optimized for:
//! * Memory efficient buffering (64 KiB minimum, matching Symphonia's requirements)
//! * Coordinated with `AudioFile` buffer sizes (32 KiB for unencrypted, 2 KiB for encrypted)
//! * Low allocation overhead (reuses sample buffers)
//! * Fast initialization through codec-specific handlers
//! * Optimized CBR MP3 seeking

use std::{io, time::Duration};

use rodio::source::SeekError;
use symphonia::{
    core::{
        audio::SampleBuffer,
        codecs::{CodecParameters, CodecRegistry, DecoderOptions},
        errors::Error as SymphoniaError,
        formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
        io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
        meta::{MetadataOptions, StandardTagKey, Value},
        probe::{Hint, Probe},
    },
    default::{
        codecs::{AacDecoder, FlacDecoder, MpaDecoder, PcmDecoder},
        formats::{AdtsReader, FlacReader, IsoMp4Reader, MpaReader, WavReader},
    },
};

use crate::{
    audio_file::{AudioFile, BUFFER_LEN},
    error::{Error, Result},
    normalize::{self, Normalize},
    player::SampleFormat,
    protocol::Codec,
    track::{Track, DEFAULT_SAMPLE_RATE},
    util::ToF32,
};

/// Audio decoder supporting multiple formats through Symphonia.
///
/// Works in conjunction with [`AudioFile`] and [`Track`] to provide:
/// * Format-specific decoding based on track codec
/// * Audio parameters (sample rate, bits per sample, channels)
/// * Duration and seeking information
/// * Normalization settings
/// * Efficient buffering coordinated with `AudioFile`:
///   - Uses 64+ KiB internal buffer (Symphonia requirement)
///   - Works with both 32 KiB unencrypted and 2 KiB encrypted input buffers
///
/// Features:
/// * Multi-format support
/// * Optimized MP3 CBR seeking
/// * Buffer reuse for minimal allocations
/// * Error recovery
/// * Transparent handling of encrypted and unencrypted streams
/// * Automatic detection of audio parameters:
///   - Sample rate (defaults to 44.1 kHz)
///   - Bits per sample (codec-dependent)
///   - Channel count (format/content specific)
///
/// # Example
/// ```no_run
/// use pleezer::decoder::Decoder;
/// use pleezer::audio_file::AudioFile;
///
/// let track = /* ... */;
/// let file = /* AudioFile instance ... */;
/// let mut decoder = Decoder::new(&track, file)?;
///
/// // Seek to 1 minute
/// decoder.try_seek(std::time::Duration::from_secs(60))?;
///
/// // Process audio samples
/// for sample in decoder {
///     // Process f32 sample...
/// }
/// ```
pub struct Decoder {
    /// Format reader (demuxer) for extracting encoded audio packets
    demuxer: Box<dyn FormatReader>,

    /// Codec decoder for converting encoded packets to PCM samples
    decoder: Box<dyn symphonia::core::codecs::Decoder>,

    /// Seeking strategy (Coarse for CBR, Accurate for VBR)
    seek_mode: SeekMode,

    /// Reusable sample buffer to minimize allocations
    buffer: Option<SampleBuffer<SampleFormat>>,

    /// Current position in the sample buffer
    position: usize,

    /// Number of audio channels in the stream
    channels: u16,

    /// Sample rate of the audio stream in Hz
    sample_rate: u32,

    /// Total duration of the audio stream
    total_duration: Option<Duration>,

    /// Total number of samples in the stream
    total_samples: Option<usize>,

    /// Maximum number of samples per frame for the current codec
    max_frame_length: Option<usize>,
}

/// Maximum number of consecutive corrupted packets to skip before giving up.
const MAX_RETRIES: usize = 3;

impl Decoder {
    /// Creates a new decoder for the given track and audio file.
    ///
    /// Optimizes decoder initialization by:
    /// * Using format-specific decoders when codec is known
    /// * Enabling coarse seeking for CBR MP3 content
    /// * Pre-allocating buffers based on format parameters
    /// * Using direct pass-through for unencrypted content
    ///
    /// Audio parameters are determined in this order:
    /// * Sample rate: From codec, falling back to 44.1 kHz
    /// * Bits per sample: From codec if available
    /// * Channels: From codec, falling back to content type default
    ///
    /// # Arguments
    /// * `track` - Track metadata including codec information
    /// * `file` - Unified audio file interface handling encryption transparently
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Format detection fails
    /// * Codec initialization fails
    /// * Required track is not found
    /// * Stream parameters are invalid
    pub fn new(track: &Track, file: AudioFile) -> Result<Self> {
        // Twice the buffer length to allow for Symphonia's read-ahead behavior,
        // and 64 kB minimum that Symphonia asserts for its ring buffer.
        let buffer_len = usize::max(64 * 1024, BUFFER_LEN * 2);
        let stream =
            MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions { buffer_len });

        // We know the codec for all tracks except podcasts, so be as specific as possible.
        let mut hint = Hint::new();
        let mut codecs = CodecRegistry::default();
        let mut probes = Probe::default();
        let (codecs, probe) = if let Some(codec) = track.codec() {
            match codec {
                Codec::ADTS => {
                    codecs.register_all::<AacDecoder>();
                    probes.register_all::<AdtsReader>();
                }
                Codec::FLAC => {
                    codecs.register_all::<FlacDecoder>();
                    probes.register_all::<FlacReader>();
                }
                Codec::MP3 => {
                    codecs.register_all::<MpaDecoder>();
                    probes.register_all::<MpaReader>();
                }
                Codec::MP4 => {
                    // MP4 files can contain many audio codecs, but most likely AAC.
                    codecs.register_all::<AacDecoder>();
                    probes.register_all::<IsoMp4Reader>();
                }
                Codec::WAV => {
                    codecs.register_all::<PcmDecoder>();
                    probes.register_all::<WavReader>();
                }
            }

            hint.with_extension(codec.extension());
            hint.mime_type(codec.mime_type());

            (&codecs, &probes)
        } else {
            // Probe all formats when the codec is unknown.
            (
                symphonia::default::get_codecs(),
                symphonia::default::get_probe(),
            )
        };

        // Coarse seeking without a known byte length causes a panic.
        // Further, it's not reliable for VBR streams.
        let seek_mode = if track.is_cbr() && stream.byte_len().is_some() {
            SeekMode::Coarse
        } else {
            SeekMode::Accurate
        };

        let demuxer = probe
            .format(
                &hint,
                stream,
                &FormatOptions {
                    enable_gapless: true,
                    ..Default::default()
                },
                &MetadataOptions::default(),
            )?
            .format;
        let default_track = demuxer
            .default_track()
            .ok_or_else(|| Error::not_found("default track not found"))?;

        let codec_params = &default_track.codec_params;
        let decoder = codecs.make(codec_params, &DecoderOptions::default())?;

        // Update the codec parameters with the actual decoder parameters.
        // This may yield information not available before decoder initialization.
        let codec_params = decoder.codec_params();
        let total_duration = Self::calc_total_duration(codec_params);
        let channels = Self::calc_channels(codec_params).unwrap_or(track.typ().default_channels());
        let sample_rate = Self::calc_sample_rate(codec_params);
        let max_frame_length = track
            .codec()
            .map(|codec| codec.max_frame_length(sample_rate, channels));
        let total_samples = Self::calc_total_samples(codec_params, max_frame_length);

        Ok(Self {
            demuxer,
            decoder,
            seek_mode,

            buffer: None,
            position: 0,

            channels,
            sample_rate,
            total_duration,
            total_samples,
            max_frame_length,
        })
    }

    /// Creates a normalized version of this decoder's output.
    ///
    /// Applies a feedforward limiter in the log domain to prevent clipping
    /// while maintaining perceived loudness. Works uniformly across all
    /// sample rates and channel configurations.
    ///
    /// Note: The limiter processes audio in floating point, so the original
    /// bits per sample value does not affect normalization quality.
    ///
    /// # Arguments
    ///
    /// * `ratio` - Basic gain ratio to apply before limiting
    /// * `threshold` - Level in dB above which limiting begins
    /// * `knee_width` - Softening range around threshold in dB
    /// * `attack` - Time for limiter to respond to level increases
    /// * `release` - Time for limiter to recover after level decreases
    ///
    /// # Returns
    ///
    /// A [`Normalize`] wrapper that processes the decoder's output through
    /// the limiter.
    #[must_use]
    pub fn normalize(
        self,
        ratio: f32,
        threshold: f32,
        knee_width: f32,
        attack: Duration,
        release: Duration,
    ) -> Normalize<Self>
    where
        Self: Sized,
    {
        normalize::normalize(self, ratio, threshold, knee_width, attack, release)
    }

    /// Returns the track's `ReplayGain` value in dB, if available.
    ///
    /// While Deezer normally provides gain information through its API for proper
    /// normalization to its -15 LUFS target, this method serves as a fallback when
    /// that information is missing. It extracts `ReplayGain` metadata from the audio
    /// file itself.
    ///
    /// Note that audio files served by Deezer do not contain `ReplayGain` metadata.
    /// This method is primarily useful for external content like podcasts that may
    /// include their own `ReplayGain` tags.
    ///
    /// `ReplayGain` is a standard for measuring and adjusting perceived audio loudness.
    /// The reference level for `ReplayGain` is -14 LUFS. When normalizing to Deezer's
    /// -15 LUFS target:
    ///
    /// 1. Calculate actual LUFS: -14 - `replay_gain`
    /// 2. Calculate difference: -15 - `actual_LUFS`
    /// 3. Convert to gain factor: 10^(difference/20)
    ///
    /// Returns `None` if no `ReplayGain` metadata is present in the audio file.
    pub fn replay_gain(&mut self) -> Option<f32> {
        self.demuxer
            .metadata()
            .skip_to_latest()
            .and_then(|metadata| {
                for tag in metadata.tags() {
                    if tag
                        .std_key
                        .is_some_and(|key| key == StandardTagKey::ReplayGainTrackGain)
                    {
                        if let Value::Float(gain) = tag.value {
                            return Some(gain.to_f32_lossy());
                        }
                    }
                }
                None
            })
    }

    /// Returns the number of bits per sample used by the audio codec, if known.
    ///
    /// This represents the precision of the audio data as decoded, before
    /// conversion to floating point samples for playback.
    #[must_use]
    pub fn bits_per_sample(&self) -> Option<u32> {
        // Not cached because it is called infrequently.
        self.decoder.codec_params().bits_per_sample
    }

    /// Extracts channel count from codec parameters, converting to `u16`.
    /// Returns `None` if channel information is unavailable.
    ///
    /// # Panics
    ///
    /// Panics if the channel count exceeds the maximum value for `u16`.
    #[must_use]
    fn calc_channels(codec_params: &CodecParameters) -> Option<u16> {
        codec_params
            .channels
            .map(|channels| channels.count().try_into().expect("channel count overflow"))
    }

    /// Gets sample rate from codec parameters, defaulting to 44.1 kHz if unspecified.
    #[must_use]
    fn calc_sample_rate(codec_params: &CodecParameters) -> u32 {
        codec_params.sample_rate.unwrap_or(DEFAULT_SAMPLE_RATE)
    }

    /// Calculates total samples in the stream from frame count and maximum frame length.
    /// Returns `None` if either value is unavailable or multiplication would overflow.
    #[must_use]
    fn calc_total_samples(
        codec_params: &CodecParameters,
        max_frame_length: Option<usize>,
    ) -> Option<usize> {
        if let (Some(n_frames), Some(max_frame_length)) = (codec_params.n_frames, max_frame_length)
        {
            usize::try_from(n_frames)
                .ok()
                .and_then(|frames| frames.checked_mul(max_frame_length))
        } else {
            None
        }
    }

    /// Extracts total duration from codec parameters if both time base and frame count are
    /// available.
    #[must_use]
    fn calc_total_duration(codec_params: &CodecParameters) -> Option<Duration> {
        if let (Some(time_base), Some(frames)) = (codec_params.time_base, codec_params.n_frames) {
            Some(time_base.calc_time(frames).into())
        } else {
            None
        }
    }

    /// Updates decoder specifications after a codec reset.
    ///
    /// Recalculates:
    /// * Sample rate
    /// * Total number of samples
    /// * Total duration
    /// * Channel count (only if explicitly provided by codec)
    fn reload_spec(&mut self) {
        let codec_params = self.decoder.codec_params();

        self.sample_rate = Self::calc_sample_rate(codec_params);
        self.total_samples = Self::calc_total_samples(codec_params, self.max_frame_length);
        self.total_duration = Self::calc_total_duration(codec_params);

        // The channel count is initialized to the default for the track type.
        // Only update it if the codec provides a specific count.
        if let Some(channels) = Self::calc_channels(codec_params) {
            self.channels = channels;
        }
    }

    /// Gets the next decodable packet from the stream.
    ///
    /// Handles error recovery by:
    /// * Skipping corrupted packets (up to `MAX_RETRIES`)
    /// * Resetting decoder state when required
    /// * Clearing internal buffer on unrecoverable errors
    ///
    /// # Returns
    ///
    /// The duration of the decoded packet in codec timebase units.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Too many consecutive packets are corrupted
    /// * An unrecoverable decoder error occurs
    /// * End of stream is reached
    fn get_next_packet(&mut self) -> Result<u64> {
        let mut skipped = 0;
        loop {
            if skipped > MAX_RETRIES {
                break Err(Error::cancelled("skipped too many packets, giving up"));
            }

            // Assume failure until a packet is successfully decoded.
            skipped = skipped.saturating_add(1);
            match self.demuxer.next_packet() {
                Ok(packet) => {
                    let decoded = match self.decoder.decode(&packet) {
                        Ok(decoded) => decoded,

                        // If a `DecodeError` or `IoError` is returned, the packet is
                        // undecodeable and should be discarded. Decoding may be continued
                        // with the next packet.
                        Err(SymphoniaError::DecodeError(e)) => {
                            error!("discarding malformed packet: {e}");
                            continue;
                        }
                        Err(SymphoniaError::IoError(e)) => {
                            error!("discarding unreadable packet: {e}");
                            continue;
                        }

                        // If `ResetRequired` is returned, consumers of the decoded audio data
                        // should expect the duration and `SignalSpec` of the decoded audio
                        // buffer to change.
                        Err(SymphoniaError::ResetRequired) => {
                            trace!("resetting decoder");
                            self.decoder.reset();
                            self.reload_spec();
                            continue;
                        }

                        // All other errors are unrecoverable.
                        Err(e) => {
                            break Err(e.into());
                        }
                    };

                    let buffer = match self.buffer.as_mut() {
                        Some(buffer) => buffer,
                        None => {
                            // The first packet is always the largest, so
                            // allocate the buffer once and reuse it.
                            self.buffer.insert(SampleBuffer::new(
                                decoded.capacity() as u64,
                                *decoded.spec(),
                            ))
                        }
                    };
                    buffer.copy_interleaved_ref(decoded);
                    self.position = 0;
                    break Ok(packet.dur());
                }

                // If `ResetRequired` is returned, then the track list must be re-examined and
                // all `Decoder`s re-created.
                Err(SymphoniaError::ResetRequired) => {
                    trace!("re-creating decoder");
                    let track = self
                        .demuxer
                        .default_track()
                        .ok_or_else(|| Error::not_found("default track not found"))?;
                    let codecs = symphonia::default::get_codecs();
                    self.decoder = codecs.make(&track.codec_params, &DecoderOptions::default())?;
                    self.reload_spec();
                    continue;
                }

                // All other errors are unrecoverable.
                Err(e) => {
                    break Err(e.into());
                }
            }
        }
    }
}

impl rodio::Source for Decoder {
    /// Returns the number of samples left in the current decoded frame.
    ///
    /// Returns `None` if no frame is currently buffered.
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.buffer.as_ref().map(SampleBuffer::len)
    }

    /// Returns the number of channels in the audio stream.
    #[inline]
    fn channels(&self) -> u16 {
        self.channels
    }

    /// Returns the sample rate of the audio stream in Hz.
    #[inline]
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the total duration of the audio stream.
    ///
    /// Returns `None` if duration cannot be determined (e.g., for streams).
    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    /// Attempts to seek to the specified position in the audio stream.
    ///
    /// Uses Symphonia's seeking capabilities with format-specific optimizations:
    /// * Coarse seeking for CBR content (faster)
    /// * Accurate seeking for VBR content (more precise)
    ///
    /// Also resets the decoder state to prevent audio glitches that could occur
    /// from seeking to a position that requires different decoding parameters.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Seeking operation fails
    /// * Position is beyond stream end
    /// * Stream format doesn't support seeking
    fn try_seek(&mut self, pos: Duration) -> std::result::Result<(), SeekError> {
        self.demuxer
            .seek(
                self.seek_mode,
                SeekTo::Time {
                    // `track_id: None` implies the default track
                    track_id: None,
                    time: pos.into(),
                },
            )
            .map_err(|e| SeekError::Other(Box::new(e)))?;

        // Seeking is a demuxer operation, so the decoder cannot reliably
        // know when a seek took place. Reset it to avoid audio glitches.
        self.decoder.reset();

        Ok(())
    }
}

impl Iterator for Decoder {
    /// A single audio sample as 32-bit floating point.
    ///
    /// Values are normalized to the range [-1.0, 1.0] regardless of the
    /// source audio's bits per sample or format.
    type Item = SampleFormat;

    /// Provides the next audio sample.
    ///
    /// Handles:
    /// * Automatic buffer refilling
    /// * Packet decoding
    /// * Error recovery (skips corrupted packets)
    /// * End of stream detection
    ///
    /// Returns `None` when:
    /// * Stream ends
    /// * Unrecoverable error occurs
    /// * Too many corrupt packets encountered
    fn next(&mut self) -> Option<Self::Item> {
        // Fill the buffer if it's empty or we've reached its end.
        if self
            .buffer
            .as_ref()
            .is_none_or(|buffer| self.position >= buffer.len())
        {
            if let Err(e) = self.get_next_packet() {
                // Internal buffer *must* be cleared if an error occurs.
                self.buffer = None;

                // `UnexpectedEof` is not an error, just the end of the stream.
                if e.downcast::<io::Error>()
                    .is_none_or(|e| e.kind() != std::io::ErrorKind::UnexpectedEof)
                {
                    error!("{e}");
                }

                return None;
            }
        }

        let sample = *self
            .buffer
            .as_ref()
            .and_then(|buf| buf.samples().get(self.position))?;
        self.position = self.position.checked_add(1)?;

        Some(sample)
    }

    /// Provides size hints for the number of samples.
    ///
    /// The lower bound is always 0 because the decoder cannot reliably predict how many
    /// samples will be successfully decoded, due to potential corruption or errors in the
    /// stream.
    ///
    /// The upper bound is:
    /// * `Some(n)` when the total number of samples can be calculated from frame count
    /// * `None` for streams where the total length is unknown or larger than `usize::MAX`
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.total_samples)
    }
}
