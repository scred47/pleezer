//! Audio decoder implementation using Symphonia.
//!
//! This module provides a decoder that:
//! * Supports multiple formats (AAC/ADTS, FLAC, MP3, MP4, WAV)
//! * Enables efficient seeking
//! * Handles both constant and variable bitrate streams
//! * Processes audio in floating point
//!
//! # Format Support
//!
//! Format-specific optimizations:
//! * MP3: Fast seeking for CBR streams using coarse mode
//! * FLAC: Native seeking with frame boundaries
//! * AAC: Proper ADTS frame synchronization
//! * WAV: Direct PCM access
//!
//! # Performance
//!
//! The decoder is optimized for:
//! * Low memory usage (reuses sample buffers)
//! * Fast initialization
//! * Efficient seeking
//! * Robust error recovery

use std::time::Duration;

use rodio::source::SeekError;
use symphonia::{
    core::{
        audio::SampleBuffer,
        codecs::{CodecRegistry, DecoderOptions},
        errors::Error as SymphoniaError,
        formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
        io::{MediaSource, MediaSourceStream},
        meta::MetadataOptions,
        probe::{Hint, Probe},
    },
    default::{
        codecs::{AacDecoder, FlacDecoder, MpaDecoder, PcmDecoder},
        formats::{AdtsReader, FlacReader, IsoMp4Reader, MpaReader, WavReader},
    },
};

use crate::{
    error::{Error, Result},
    player::{SampleFormat, DEFAULT_SAMPLE_RATE},
    protocol::Codec,
    track::Track,
    util::ToF32,
};

/// Audio decoder supporting multiple formats through Symphonia.
///
/// Features:
/// * Format-specific optimizations
/// * Efficient seeking modes
/// * Buffer reuse
/// * Error recovery
///
/// # Example
/// ```no_run
/// use pleezer::decoder::Decoder;
/// use symphonia::core::io::MediaSourceStream;
///
/// let track = /* ... */;
/// let stream = MediaSourceStream::new(/* ... */);
/// let mut decoder = Decoder::new(&track, stream)?;
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
}

/// Maximum number of consecutive corrupted packets to skip before giving up.
const MAX_RETRIES: usize = 3;

impl Decoder {
    /// Creates a new decoder for the given track and media stream.
    ///
    /// Optimizes decoder initialization by:
    /// * Using format-specific decoders when codec is known
    /// * Selecting appropriate seek mode (coarse for CBR, accurate for VBR)
    /// * Pre-allocating buffers based on format parameters
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Format detection fails
    /// * Codec initialization fails
    /// * Required track is not found
    /// * Stream parameters are invalid
    pub fn new(track: &Track, stream: MediaSourceStream) -> Result<Self> {
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
                    // MP4 files can contain any type of audio codec, but most likely AAC.
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
            .ok_or(Error::not_found("default track not found"))?;

        let codec_params = &default_track.codec_params;
        trace!(
            "sampling rate: {} kHz",
            codec_params
                .sample_rate
                .map_or("unknown".to_string(), |rate| (rate.to_f32_lossy() / 1000.)
                    .to_string())
        );
        let decoder = codecs.make(codec_params, &DecoderOptions::default())?;

        let sample_rate = codec_params.sample_rate.unwrap_or(DEFAULT_SAMPLE_RATE);
        let channels = codec_params.channels.map_or_else(
            || track.typ().default_channels(),
            |channels| u16::try_from(channels.count()).unwrap_or(u16::MAX),
        );

        let mut total_duration = None;
        if let Some(time_base) = codec_params.time_base {
            if let Some(frames) = codec_params.n_frames {
                total_duration = Some(time_base.calc_time(frames).into());
            }
        }
        let total_samples = codec_params.n_frames.and_then(|frames| {
            frames
                .checked_mul(channels.into())
                .and_then(|samples| usize::try_from(samples).ok())
        });

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
        })
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
    /// Uses format-specific optimizations:
    /// * Coarse seeking for CBR content (faster)
    /// * Accurate seeking for VBR content (more precise)
    ///
    /// Also resets decoder state to prevent audio glitches after seeking.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Seeking operation fails
    /// * Position is beyond stream end
    /// * Stream doesn't support seeking
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
            .map_err(|e| {
                rodio::source::SeekError::SymphoniaDecoder(
                    rodio::decoder::symphonia::SeekError::BaseSeek(e),
                )
            })?;

        // Seeking is a demuxer operation, so the decoder cannot reliably
        // know when a seek took place. Reset it to avoid audio glitches.
        self.decoder.reset();

        Ok(())
    }
}

impl Iterator for Decoder {
    /// A single audio sample as 32-bit floating point.
    ///
    /// Values are normalized to the range [-1.0, 1.0].
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
            let mut skipped = 0;
            loop {
                if skipped > MAX_RETRIES {
                    error!("skipped too many packets, giving up");
                    return None;
                }

                match self.demuxer.next_packet() {
                    Ok(packet) => {
                        let decoded = self.decoder.decode(&packet).ok()?;
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
                        break;
                    }

                    Err(SymphoniaError::IoError(e)) => {
                        if e.kind() == std::io::ErrorKind::UnexpectedEof {
                            // Not an error, just the end of the stream.
                            return None;
                        }
                        error!("{e}");
                        return None;
                    }
                    Err(SymphoniaError::DecodeError(e)) => {
                        error!("skipping malformed packet: {e}");
                        skipped = skipped.saturating_add(1);
                        continue;
                    }
                    Err(SymphoniaError::ResetRequired) => {
                        self.decoder.reset();
                        continue;
                    }
                    Err(e) => {
                        error!("{e}");
                        return None;
                    }
                }
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
    /// Returns exact count when total frames are known.
    /// Otherwise returns (0, None) for streams.
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.total_samples.unwrap_or(0), self.total_samples)
    }
}

impl ExactSizeIterator for Decoder {}
