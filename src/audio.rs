use clap::ValueEnum;

/// Audio quality levels as per Deezer on desktop.
///
/// Note that the remote device has no control over the audio quality of the
/// player.
#[derive(ValueEnum, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Quality {
    /// 128 kbps MP3 (default)
    #[default]
    Standard,
    /// 320 kbps MP3 (requires Premium subscription)
    High,
    /// 1411 kbps FLAC (requires HiFi subscription)
    Lossless,
}
