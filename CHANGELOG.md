# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
and [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

## [Unreleased]

### Fixed
- [docs] Fix Rustdoc lints and warnings

## [0.5.0] - 2024-12-09

### Added
- [player] Support JACK and ASIO audio backends
- [player] Queue reordering with position tracking
- [remote] Queue shuffle support with state synchronization
- [remote] Initial volume setting that remains active until client takes control below maximum

### Fixed
- [docs] Fix Rustdoc linking to error module in documentation

## [0.4.0] - 2024-12-02

### Added
- [docs] Comprehensive documentation for all public APIs and internals
- [docs] Recommendation to use 32-bit output formats for better audio precision
- [error] Add `downcast()` method to access underlying error types
- [player] Explicit audio device lifecycle with `start()`, `stop()` and `is_started()`
- [uuid] `uuid` module providing a fast UUID v4 generator

### Changed
- [docs] Clarify that Deezer Connect control only works from mobile devices
- [gateway] Use UNIX epoch instead of current time for expired token state
- [main] Use kernel instead of distribution version on Linux systems
- [player] Scale volume logarithmically with 60 dB dynamic range
- [player] Only show output devices that support stereo 44.1/48 kHz in I16/I32/F32 formats
- [remote] Start/stop audio device on controller connect/disconnect
- [remote] Improve connection handshake ordering and timeout handling

### Fixed
- [protocol] Use epsilon comparison for `Percentage` equality checks
- [player] Prevent from acquiring output device before playback starts
- [player] Default device was not enumerated on Alsa
- [remote] Improve queue refresh handling
- [remote] Fix race condition in controller connection setup
- [tokens] Fix token expiration check

### Removed
- [docs] Remove unnecessary Homebrew installation instructions

## [0.3.0] - 2024-11-28

### Added
- [chore] Add Debian package metadata
- [docs] Add ALSA device examples to the README
- [main] Add `--device-type` option to control device identification

### Changed
- [build] Document release profile optimization options
- [docs] Improve Rustdoc of `protos`, `channel` and `contents` modules
- [gateway] Improve error logging for response parsing failures
- [main] Clearer log messages for secrets and logins
- [protocol] Parse JSON as 64-bit and truncate internally

### Fixed
- [config] Hexademical base does not correlate to key length
- [gateway] Parse user data without all fields present
- [gateway] Incorrect user token expiry
- [player] Use pipe separator in device specs for ALSA compatibility
- [player] Clean up audio device enumeration output
- [player] Playback progress not updating correctly after third track
- [player] Delay reporting playback progress after a track change
- [repo] Fix pull request template format
- [remote] Trigger connected and disconnected events

## [0.2.0] - 2024-11-23

### Added
- [main] Support for configuring all command-line options via environment variables with `PLEEZER_` prefix
- [proxy] HTTPS proxy support via the `HTTPS_PROXY` environment variable
- [remote] Websocket monitoring mode for Deezer Connect protocol analysis
- [remote] Hook script support to execute commands on playback and connection events

### Changed
- [docs] Enhanced documentation clarity and consistency across all policy documents
- [main] Improved command-line argument descriptions and examples
- [main] Made command-line parsing dependency (`clap`) optional and binary-only
- [player] Optimized track skipping using `HashSet` for better performance
- [track] Get the media URL programmatically instead of hardcoding it

### Fixed
- [protocol] Correctly handle `connect` messages missing the `offer_id` field

### Security
- [arl] Prevent ARL token exposure in debug logs

## [0.1.0] - 2024-11-20

Initial release of pleezer, a headless streaming player for the Deezer Connect protocol.

### Added
- High-quality audio streaming (basic, HQ, or lossless) based on subscription
- Gapless playback for seamless transitions
- Flow and mixes support
- Playback reporting for artist monetization
- User MP3 playback support
- Deezer authentication via email/password or ARL token
- Volume normalization with clipping prevention
- Configurable audio output device selection
- Debug and trace logging capabilities
- Command-line interface with various configuration options

[Unreleased]: https://github.com/roderickvd/pleezer/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/roderickvd/pleezer/releases/tag/v0.5.0
[0.4.0]: https://github.com/roderickvd/pleezer/releases/tag/v0.4.0
[0.3.0]: https://github.com/roderickvd/pleezer/releases/tag/v0.3.0
[0.2.0]: https://github.com/roderickvd/pleezer/releases/tag/v0.2.0
[0.1.0]: https://github.com/roderickvd/pleezer/releases/tag/v0.1.0
