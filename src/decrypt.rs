use std::{
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom},
    ops::Deref,
    str::FromStr,
};

use blowfish::{cipher::BlockDecryptMut, cipher::KeyIvInit, Blowfish};
use cbc::cipher::block_padding::NoPadding;
use md5::{Digest, Md5};

use crate::{
    error::{Error, Result},
    protocol::media::Cipher,
    track::{Track, TrackId},
};

/// Provides a stream of decrypted data from an encrypted track by implementing
/// the `Read` and `Seek` traits. Decryption is done on the fly, meaning that
/// data is decrypted as it is read from the source.
///
/// # On the Fly Decryption
///
/// On the fly decryption means that data is decrypted as it is read from the
/// source, without saving the decrypted data to disk. This way, no DRM is
/// bypassed, and we abide by the Deezer Terms of Service.
///
/// # Decryption Key
///
/// Again to abide by the Deezer Terms of Service, the Deezer decryption key
/// (actually: salt) is not contained within this module. Its value must be
/// provided by the user.
///
/// # Supported Encryption
///
/// Currently, this module only supports CBC decryption with Blowfish. This is
/// the only encryption algorithm used by Deezer at the time of writing.
///
/// Tracks without encryption are not supported by this module simply have their
/// `Read` and `Seek` implementations passed through.
pub struct Decrypt {
    /// The file to decrypt.
    file: fs::File,

    /// The size of the file.
    file_size: Option<u64>,

    /// The encryption cipher.
    cipher: Cipher,

    /// The decryption key.
    key: Key,

    /// The buffer to store the current block of decrypted data. Each block is
    /// at most 2 kB, and can be smaller if it is the last block until the end
    /// of the track.
    buffer: Cursor<Vec<u8>>,

    /// The current block number.
    block: Option<u64>,
}

/// The fixed length of a decryption key.
pub const KEY_LENGTH: usize = 16;

pub type RawKey = [u8; KEY_LENGTH];

/// A decryption key with fixed length.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Key(RawKey);

impl FromStr for Key {
    type Err = Error;

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

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Decrypt {
    /// The initialization vector to use for CBC decryption. Deezer uses a fixed
    /// initialization vector of 8 bytes.
    const CBC_BF_IV: &[u8; 8] = b"\x00\x01\x02\x03\x04\x05\x06\x07";

    /// The size of a block. Deezer uses blocks of 2 kB.
    const CBC_BLOCK_SIZE: usize = 2 * 1024;

    /// For each set of blocks in a stripe, which block is encrypted. Deezer
    /// encrypts every third block.
    const CBC_STRIPE_COUNT: usize = 3;

    /// The supported encryption ciphers.
    const SUPPORTED_CIPHERS: [Cipher; 2] = [Cipher::NONE, Cipher::BF_CBC_STRIPE];

    /// Create a new decryptor for the given track and salt. The salt is the
    /// fixed Deezer decryption key, from which the track-specific key is
    /// calculated.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the track uses an unsupported encryption algorithm.
    pub fn new(track: &Track, salt: &Key) -> Result<Self> {
        if !Self::SUPPORTED_CIPHERS.contains(&track.cipher()) {
            return Err(Error::unimplemented("unsupported encryption algorithm"));
        }

        let mut file = track.try_file()?;
        file.rewind()?;

        // Calculate decryption key.
        let key = Self::key_for_track_id(track.id(), salt);

        Ok(Self {
            file,
            file_size: track.file_size(),
            cipher: track.cipher(),
            key,
            buffer: Cursor::new(Vec::new()),
            block: None,
        })
    }

    /// Calculate the decryption key for a track ID and salt. The salt is the
    /// Deezer decryption key, from which the track-specific key is calculated.
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
    /// Seeks to the given position in the decrypted stream. If the track is not
    /// encrypted, this is a simple pass-through to the underlying track.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // If the track is not encrypted, we can seek directly.
        if self.cipher == Cipher::NONE {
            return self.file.seek(pos);
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
            self.file
                .seek(SeekFrom::Start(block * Self::CBC_BLOCK_SIZE as u64))?;

            // TODO : when this is the first block of two unencrypted blocks,
            // read ahead 2 * CBC_BLOCK_SIZE.
            let mut buffer = [0; Self::CBC_BLOCK_SIZE];
            let length = self.file.read(&mut buffer)?;

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
    /// Reads data from the decrypted stream. If the track is not encrypted,
    /// this is a simple pass-through to the underlying track.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If the track is not encrypted, we can read directly.
        if self.cipher == Cipher::NONE {
            return self.file.read(buf);
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
