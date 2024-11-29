//! Track decryption for Deezer's protected media content.
//!
//! This module provides decryption of Deezer tracks while streaming:
//! * Decrypts data in 2KB blocks as it's read
//! * Uses temporary file storage for encrypted data
//! * Supports Blowfish CBC encryption with striping
//!
//! # Encryption Format
//!
//! Deezer uses a striped encryption pattern:
//! * Content is divided into 2KB blocks
//! * Every third block is encrypted
//! * Encryption uses Blowfish in CBC mode
//! * A fixed IV is used
//!
//! # Security
//!
//! To comply with Deezer's Terms of Service:
//! * No decryption keys are included in this code
//! * Keys must be provided externally
//!
//! # Memory Management
//!
//! The implementation uses:
//! * Temporary file storage for the encrypted stream
//! * 2KB buffer for decrypted blocks
//! * No additional buffering needed
//!
//! # Examples
//!
//! ```rust
//! use pleezer::decrypt::{Decrypt, Key};
//!
//! // Create decryptor with track and key
//! let decryptor = Decrypt::new(&track, download, &key)?;
//!
//! // Read and decrypt content
//! let mut buffer = Vec::new();
//! decryptor.read_to_end(&mut buffer)?;
//! ```

use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    ops::Deref,
    str::FromStr,
};

use blowfish::{cipher::BlockDecryptMut, cipher::KeyIvInit, Blowfish};
use cbc::cipher::block_padding::NoPadding;
use md5::{Digest, Md5};
use stream_download::{storage::temp::TempStorageProvider, StreamDownload};

use crate::{
    error::{Error, Result},
    protocol::media::Cipher,
    track::{Track, TrackId},
};

/// Streaming decryptor for protected tracks.
///
/// Provides decryption of Deezer tracks by implementing `Read` and `Seek`.
/// Uses temporary file storage for the encrypted stream and decrypts
/// data in 2KB blocks as it's read.
///
/// # Buffering
///
/// Uses 2KB blocks for decryption. No additional buffering is needed
/// as the `Read` implementation handles blocks efficiently.
///
/// # Supported Encryption
///
/// Currently supports:
/// * No encryption (passthrough)
/// * Blowfish CBC with striping (every third 2KB block)
pub struct Decrypt {
    /// Source of encrypted data using temporary file storage.
    download: StreamDownload<TempStorageProvider>,

    /// Total size of the track in bytes, if known.
    ///
    /// Used for seek operations, particularly for seeking from
    /// the end of the track.
    file_size: Option<u64>,

    /// Encryption method used for this track.
    ///
    /// Either `NONE` for unencrypted tracks or `BF_CBC_STRIPE`
    /// for Blowfish CBC encryption with striping.
    cipher: Cipher,

    /// Track-specific decryption key.
    ///
    /// Derived from the track ID and Deezer master key using
    /// `key_for_track_id()`.
    key: Key,

    /// Decrypted data buffer.
    ///
    /// Contains the current 2KB block (or smaller for the last block)
    /// of decrypted data. Position tracks how much has been read.
    buffer: Cursor<Vec<u8>>,

    /// Current block number being processed.
    ///
    /// Used to track position in the stream and determine which
    /// blocks need decryption (every third block when using
    /// `BF_CBC_STRIPE`).
    block: Option<u64>,
}

/// Length of decryption keys in bytes.
pub const KEY_LENGTH: usize = 16;

/// Raw key bytes.
pub type RawKey = [u8; KEY_LENGTH];

/// Validated decryption key.
///
/// Ensures keys are the correct length and format for use
/// with Blowfish decryption.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Key(RawKey);

impl FromStr for Key {
    type Err = Error;

    /// Parses a string into a decryption key.
    ///
    /// The string must be exactly 16 bytes long, as required by
    /// Blowfish and Deezer's encryption format.
    ///
    /// # Errors
    ///
    /// Returns `Error::OutOfRange` if the string length isn't
    /// exactly 16 bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pleezer::decrypt::Key;
    ///
    /// // Valid 16-byte key
    /// let key: Key = "1234567890123456".parse()?;
    ///
    /// // Too short
    /// assert!("12345".parse::<Key>().is_err());
    ///
    /// // Too long
    /// assert!("12345678901234567".parse::<Key>().is_err());
    /// ```
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let len = s.len();
        if len != KEY_LENGTH {
            return Err(Error::out_of_range(format!(
                "key length is {len} but should be {KEY_LENGTH}",
            )));
        }

        let bytes = s.as_bytes();
        let mut key = [0; KEY_LENGTH];
        key.copy_from_slice(bytes);

        Ok(Self(key))
    }
}

impl Deref for Key {
    type Target = RawKey;

    /// Provides read-only access to the raw key bytes.
    ///
    /// This allows using the key with cryptographic functions
    /// that expect byte arrays while maintaining key encapsulation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pleezer::decrypt::Key;
    ///
    /// let key: Key = "1234567890123456".parse()?;
    ///
    /// // Access raw bytes
    /// assert_eq!(&*key, b"1234567890123456");
    ///
    /// // Use with crypto functions
    /// let cipher = Blowfish::new_from_slice(&*key)?;
    /// ```
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Decrypt {
    /// Fixed IV for CBC decryption.
    const CBC_BF_IV: &[u8; 8] = b"\x00\x01\x02\x03\x04\x05\x06\x07";

    /// Size of each block in bytes (2KB).
    const CBC_BLOCK_SIZE: usize = 2 * 1024;

    /// Number of blocks in a stripe (3).
    ///
    /// Every third block is encrypted.
    const CBC_STRIPE_COUNT: usize = 3;

    /// Supported encryption methods.
    const SUPPORTED_CIPHERS: [Cipher; 2] = [Cipher::NONE, Cipher::BF_CBC_STRIPE];

    /// Creates a new decryptor for a track.
    ///
    /// # Arguments
    ///
    /// * `track` - Track to decrypt
    /// * `download` - Download stream
    /// * `salt` - Deezer decryption key (used to derive track-specific key)
    ///
    /// # Errors
    ///
    /// Returns `Error::Unimplemented` if track uses unsupported encryption.
    pub fn new(
        track: &Track,
        download: StreamDownload<TempStorageProvider>,
        salt: &Key,
    ) -> Result<Self> {
        if !Self::SUPPORTED_CIPHERS.contains(&track.cipher()) {
            return Err(Error::unimplemented("unsupported encryption algorithm"));
        }

        // Calculate decryption key.
        let key = Self::key_for_track_id(track.id(), salt);

        Ok(Self {
            download,
            file_size: track.file_size(),
            cipher: track.cipher(),
            key,
            buffer: Cursor::new(Vec::new()),
            block: None,
        })
    }

    /// Calculates track-specific decryption key.
    ///
    /// The key is derived using:
    /// 1. MD5 hash of track ID
    /// 2. XOR with Deezer master key
    ///
    /// # Arguments
    ///
    /// * `track_id` - Track to generate key for
    /// * `salt` - Deezer master key
    #[must_use]
    pub fn key_for_track_id(track_id: TrackId, salt: &Key) -> Key {
        let track_hash = format!("{:x}", Md5::digest(track_id.to_string()));
        let track_hash = track_hash.as_bytes();

        let mut key = RawKey::default();
        for i in 0..KEY_LENGTH {
            key[i] = track_hash[i] ^ track_hash[i + KEY_LENGTH] ^ salt[i];
        }
        Key(key)
    }

    /// Calculates number of bytes in the buffer that have not been read yet.
    #[must_use]
    fn bytes_on_buffer(&self) -> u64 {
        let len = self.buffer.get_ref().len() as u64;

        // The buffer position can be beyond the buffer length if a position
        // beyond the buffer length is seeked to.
        len.saturating_sub(self.buffer.position())
    }
}

impl Seek for Decrypt {
    /// Seeks within the decrypted stream.
    ///
    /// Handles:
    /// * Block boundary calculation
    /// * Buffer management
    /// * Decryption of new blocks
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // If the track is not encrypted, we can seek directly.
        if self.cipher == Cipher::NONE {
            return self.download.seek(pos);
        }

        // Calculate the target position in the encrypted file.
        let target = match pos {
            SeekFrom::Start(pos) => pos,

            SeekFrom::End(pos) => {
                let file_size = self.file_size.ok_or(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "cannot seek from the end of a stream with unknown size",
                ))?;

                file_size
                    .checked_add_signed(pos)
                    .and_then(|pos| pos.checked_sub(1))
                    .ok_or(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid seek to a negative or overflowing position",
                    ))?
            }

            SeekFrom::Current(pos) => {
                let current = self.block.map_or(0, |block| {
                    block * Self::CBC_BLOCK_SIZE as u64 + self.buffer.position()
                });

                current.checked_add_signed(pos).ok_or(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid seek to a negative or overflowing position",
                ))?
            }
        };

        if self.file_size.is_some_and(|file_size| target >= file_size) {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "seek to a position beyond the end of the file",
            ));
        }

        // The encrypted file is striped into blocks of STRIPE_SIZE bytes,
        // alternating between encrypted and non-encrypted blocks. Calculate
        // the block number within the encrypted file and the offset within the
        // block.
        let block = target
            .checked_div(Self::CBC_BLOCK_SIZE as u64)
            .ok_or(io::Error::new(
                io::ErrorKind::InvalidInput,
                "block calculation would be divide by zero",
            ))?;
        let offset = target
            .checked_rem(Self::CBC_BLOCK_SIZE as u64)
            .ok_or(io::Error::new(
                io::ErrorKind::InvalidInput,
                "offset calculation would be divide by zero",
            ))?;

        // If the buffer is empty, or the target block is different from the
        // current block, read the block from the encrypted file.
        if self.block.is_none_or(|current| current != block) {
            self.block = Some(block);

            // Seek to the start of the block in the encrypted file.
            self.download
                .seek(SeekFrom::Start(block * Self::CBC_BLOCK_SIZE as u64))?;

            // TODO : when this is the first block of two unencrypted blocks,
            // read ahead 2 * CBC_BLOCK_SIZE.
            let mut buffer = [0; Self::CBC_BLOCK_SIZE];
            let length = self.download.read(&mut buffer)?;

            // Decrypt the block if it is encrypted. Every third block is
            // encrypted, and only if the block is of a full stripe size.
            let is_encrypted = block % Self::CBC_STRIPE_COUNT as u64 == 0;
            let is_full_block = length == Self::CBC_BLOCK_SIZE;

            if is_encrypted && is_full_block {
                // The state of the cipher is reset on each block.
                let cipher =
                    cbc::Decryptor::<Blowfish>::new_from_slices(&*self.key, Self::CBC_BF_IV)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

                // Decrypt the block in-place. The buffer is guaranteed to be
                // a multiple of the block size, so no padding is necessary.
                cipher
                    .decrypt_padded_mut::<NoPadding>(&mut buffer)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            }

            // Truncate the buffer to the actual length of the block.
            let mut buffer = buffer.to_vec();
            buffer.truncate(length);

            self.buffer = Cursor::new(buffer);
        }

        // Set the offset position within the current block, and return the
        // target position in the decrypted stream.
        self.buffer.set_position(offset);
        Ok(target)
    }
}

impl Read for Decrypt {
    /// Reads decrypted data from the stream.
    ///
    /// For unencrypted tracks, passes through directly to the
    /// underlying stream. For encrypted tracks:
    /// 1. Fills internal buffer if empty
    /// 2. Decrypts blocks as needed
    /// 3. Returns requested number of bytes
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If the track is not encrypted, we can read directly.
        if self.cipher == Cipher::NONE {
            return self.download.read(buf);
        }

        let mut bytes_on_buffer = self.bytes_on_buffer();
        let bytes_wanted = buf.len();
        let mut bytes_read = 0;

        while bytes_read < bytes_wanted {
            // If the buffer is empty, trigger a seek to read the next block.
            if bytes_on_buffer == 0 {
                // equivalent to self.seek(SeekFrom::Current(0))
                let _ = self.stream_position()?;
                bytes_on_buffer = self.bytes_on_buffer();
            }

            // If the buffer is still empty, we have reached the end.
            if bytes_on_buffer == 0 {
                break;
            }

            // Read as many bytes as possible from the buffer. If
            // `bytes_on_buffer` is larger than `usize`, set it to `usize::MAX`
            // which should be equal to or larger than `bytes_wanted`.
            let bytes_to_read = usize::min(
                bytes_on_buffer.try_into().unwrap_or(usize::MAX),
                bytes_wanted.saturating_sub(bytes_read),
            );
            let bytes_read_from_buffer = self
                .buffer
                .read(&mut buf[bytes_read..bytes_read + bytes_to_read])?;

            bytes_on_buffer -= bytes_read_from_buffer as u64;
            bytes_read += bytes_read_from_buffer;
        }

        Ok(bytes_read)
    }
}
