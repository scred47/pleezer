# pleezer

**pleezer** is an open-source, headless streaming player built around the [Deezer Connect](https://support.deezer.com/hc/en-gb/articles/5449309457949-Deezer-Connect) protocol. "Headless" means it runs without a graphical interface, making it ideal for DIY setups, server-based systems, or custom integrations where flexibility is key.

## Project Status

**pleezer** is a new project and is actively being developed. We welcome contributions and feedback from the community. For more details, see the [Contributing](#contributing) section.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [Command-Line Arguments](#command-line-arguments)
  - [Environment Variables](#environment-variables)
  - [Proxy Configuration](#proxy-configuration)
  - [Hook Scripts](#hook-scripts)
  - [Stateless Configuration](#stateless-configuration)
  - [Configuring the Secrets File](#configuring-the-secrets-file)
- [Troubleshooting](#troubleshooting)
- [Setting Up Your Build Environment](#setting-up-your-build-environment)
- [Contributing](#contributing)
- [Support My Work](#support-my-work)
- [Related Projects](#related-projects)
- [License](#license)
- [Important Information](#important-information)
  - [Important Disclaimer](#important-disclaimer)
  - [Deezer Terms of Service](#deezer-terms-of-service)
  - [Security](#security)
- [Contacting the Author](#contacting-the-author)

## Features

### Supported Features

- **Playback Controls**:
  - Queue management with shuffle and repeat modes
  - Gapless playback for seamless transitions
- **High-Quality Audio**: Stream in basic, HQ, or lossless formats depending on your [Deezer subscription](http://www.deezer.com/offers).
- **Volume Controls**:
  - Logarithmic volume scaling for natural-feeling volume control
  - Volume normalization to maintain consistent levels across tracks
  - Configurable initial volume level with automatic fallback to client control
- **Content Support**:
  - **Flow and Mixes**: Access personalized playlists and [mixes](https://www.deezer.com/explore/mixes) tailored to your preferences
  - **User MP3s**: Play your [uploaded MP3 files](https://support.deezer.com/hc/en-gb/articles/115004221605-Upload-MP3s) alongside streamed content
  - **Playback Reporting**: Contribute to accurate artist monetization metrics
- **Audio Backends**:
  - [JACK](https://jackaudio.org/) backend for audio routing between applications (Linux only)
  - [ASIO](https://helpcenter.steinberg.de/hc/en-us/articles/17863730844946-Steinberg-built-in-ASIO-Driver-information-download) backend for low-latency audio with direct hardware access (Windows only)
- **Connection Options**:
  - Authentication via Deezer email/password or [ARL token](https://www.deezer.com/desktop/login/electron/callback)
  - HTTPS proxy support using system environment variables

### Planned Features

- [Live radio](https://www.deezer.com/explore/radio) and [podcast](https://www.deezer.com/explore/podcasts) integration
- Device registration

## Installation

Before installing **pleezer**, ensure your system has the necessary build environment set up. Refer to the [Setting Up Your Build Environment](#setting-up-your-build-environment) section for instructions.

**pleezer** can be installed in one of two ways:

1. **Install the Latest Stable Version**

   You can install the latest stable release of **pleezer** directly from [crates.io](https://crates.io/crates/pleezer) using Cargo:

   ```bash
   cargo install pleezer
   ```

   This command downloads, compiles, and installs the latest release version of **pleezer**. The binary will be placed in `~/.cargo/bin/` on Unix-like systems or `C:\Users\<YourUsername>\.cargo\bin\` on Windows.

2. **Build the Latest Development Version**

   If you want the latest development version, follow these steps:

   1. Clone the repository:
      ```bash
      git clone https://github.com/your-repo/pleezer.git
      cd pleezer
      ```

   2. Build the project:
      ```bash
      cargo build --release
      ```

      This command compiles the project and produces the binary in the `target/release/` directory.

   3. (Optional) Install the built version system-wide:
      ```bash
      cargo install --path .
      ```

      This installs the binary into `~/.cargo/bin/` on Unix-like systems or `C:\Users\<YourUsername>\.cargo\bin\` on Windows.

## Usage

**pleezer** acts as a remote playback device that can be controlled from the Deezer mobile app using [Deezer Connect](https://support.deezer.com/hc/en-gb/articles/5449309457949-Deezer-Connect). Note that Deezer Connect only works *from* mobile devices - you cannot control **pleezer** from desktop apps or the web player.

Following the official Deezer Connect instructions, here's how to control **pleezer** from your mobile device:

1. **Open** the Deezer app on your mobile device
2. **Tap** the loudspeaker icon in the bottom-left corner (Audio Options)
3. **Select** **Deezer Connect** to view available devices
4. **Choose** the device named with either your specified name (`-n` option) or the system hostname

Your music will then play through **pleezer** while being controlled from your mobile device.

**Note:** The device running **pleezer** must:
- Be connected to the internet
- Use the same Deezer account as your mobile device

### Command-Line Arguments

- `-n` or `--name`: Set the player's name as it appears to Deezer clients. By default, it uses the system hostname. Example:
    ```bash
    pleezer --name "My Deezer Player"
    ```

- `--device-type`: Set how the device identifies itself to Deezer clients. Affects how the device appears in Deezer apps. Options are: web (default), mobile, tablet, or desktop. Example:
    ```bash
    pleezer --device-type mobile
    ```

- `-d` or `--device`: Select the output device. Use `?` to list available stereo 44.1/48 kHz output devices. If omitted, the system default output device is used. Examples:
    ```bash
    # List available stereo 44.1/48 kHz output devices
    pleezer -d "?"
    ```

    Devices are specified in the format:
    `[<host>][|<device>][|<sample rate>][|<sample format>]` (case-insensitive)

    All fields are optional:
    - If you don't specify a host, it will use the system default host.
    - If you don't specify a device, it will use the host default device.
    - If you don't specify a sample rate, it will use the device default sample rate.
    - If you don't specify a sample format, it will use the device default sample format.

    Sample formats use Rust naming conventions:
    - `i16`: Signed 16-bit integer (S16 in ALSA)
    - `i32`: Signed 32-bit integer (S32)
    - `f32`: 32-bit float (FLOAT)

    Examples by platform:

    Linux ([ALSA](https://www.alsa-project.org/wiki/Main_Page)):
    ```bash
    pleezer -d "ALSA|default:CARD=Headphones"                  # Named device
    pleezer -d "ALSA|hw:CARD=sndrpihifiberry,DEV=0|44100|i16"  # Hardware device
    ```

    Linux ([JACK](https://jackaudio.org/)) (requires `--features jack`):
    ```bash
    pleezer -d "JACK|cpal_client_out"               # Connect as "cpal_client_out"
    ```

    macOS ([CoreAudio](https://developer.apple.com/documentation/coreaudio)):
    ```bash
    pleezer -d "CoreAudio"                          # System default
    pleezer -d "CoreAudio|Yggdrasil+"               # Specific device
    pleezer -d "CoreAudio|Yggdrasil+|44100"         # With sample rate
    pleezer -d "CoreAudio|Yggdrasil+|44100|f32"     # With format
    ```

    Windows ([ASIO](https://helpcenter.steinberg.de/hc/en-us/articles/17863730844946-Steinberg-built-in-ASIO-Driver-information-download)) (requires `--features asio`):
    ```bash
    pleezer -d "ASIO"                               # System default ASIO device
    pleezer -d "ASIO|Focusrite USB ASIO"            # Specific ASIO device
    ```

    Windows ([WASAPI](https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi)):
    ```bash
    pleezer -d "WASAPI"                            # System default
    pleezer -d "WASAPI|Speakers"                   # Specific device
    pleezer -d "WASAPI|Speakers|44100|f32"         # With format
    ```

    Shorthand syntax (any platform):
    ```bash
    pleezer -d "|yggdrasil+"    # Just device name (case-insensitive)
    pleezer -d "||44100"        # Just sample rate
    ```

    **Notes:**
    - Deezer streams audio at 44100 Hz exclusively. Sampling rates other than
      44100 Hz are not recommended and provided for compatibility only.
      Resampling is done using linear interpolation.
    - 32-bit sample formats (i32/f32) are recommended when using volume control
      or normalization, as they preserve more precision in the audio output.
    - Advanced: While device enumeration shows only common configurations
      (44.1/48 kHz, I16/I32/F32), other sample rates (e.g., 96 kHz) and
      formats (e.g., U16) are supported when explicitly specified in the
      device string.

- `--normalize-volume`: Enable volume normalization to maintain consistent volume levels across tracks. This operates independently from the "Normalize audio" setting in Deezer apps. Example:
    ```bash
    pleezer --normalize-volume
    ```

- `--initial-volume`: Set initial volume level between 0 and 100. Remains active until a Deezer client sets volume below maximum. Example:
    ```bash
    pleezer --initial-volume 50  # Start at 50% volume
    ```

- `--no-interruptions`: Prevent other clients from taking over the connection after **pleezer** has connected. By default, interruptions are allowed. Example:
    ```bash
    pleezer --no-interruptions
    ```

- `--hook`: Specify a script to execute when events occur (see [Hook Scripts](#hook-scripts) for details). Example:
    ```bash
    pleezer --hook /path/to/script.sh
    ```
    **Note:** The script must be executable and have a shebang line.

- `-q` or `--quiet`: Suppresses all output except warnings and errors. Example:
    ```bash
    pleezer -q
    ```

- `-v` or `--verbose`: Enables debug logging. Use `-vv` for trace logging. The `--quiet` and `--verbose` options are mutually exclusive. Examples:
    ```bash
    pleezer -v    # Debug logging
    pleezer -vv   # Trace logging
    ```

- `--eavesdrop`: Listen to the Deezer Connect websocket without participating. This is useful for development purposes and requires verbose or probably trace logging (`-v` or `-vv`). Example:
    ```bash
    pleezer --eavesdrop -vv
    ```

    **Note:** This option provides only partial insight into client communications. While some messages are echoed across all websockets belonging to a user, most messages are sent on separate websockets specific to each client. For complete traffic analysis, monitoring of all websockets would be required.

- `-s` or `--secrets`: Specify the secrets configuration file. Defaults to `secrets.toml`. Example:
    ```bash
    pleezer -s /path/to/secrets.toml
    ```

- `-h` or `--help`: Display help information about command-line options and exit. Example:
    ```bash
    pleezer -h
    ```

- `--version`: Show **pleezer** version and build information, then exit. Example:
    ```bash
    pleezer --version
    ```

### Environment Variables

All command-line options can be set using environment variables by prefixing `PLEEZER_` to the option name in SCREAMING_SNAKE_CASE. For example:

```bash
# Using environment variables
export PLEEZER_NAME="Living Room"
export PLEEZER_NO_INTERRUPTIONS=true
export PLEEZER_INITIAL_VOLUME=50  # Set initial volume to 50%

# Command-line arguments override environment variables
pleezer --name "Kitchen"  # Will use "Kitchen" instead
```

Command-line arguments take precedence over environment variables if both are set.

### Proxy Configuration

**pleezer** supports proxy connections through the `HTTPS_PROXY` environment variable. The value must include either the `http://` or `https://` schema prefix. HTTPS can be tunneled over either HTTP or HTTPS proxies.

Examples:

```bash
# Linux/macOS
export HTTPS_PROXY="http://proxy.example.com:8080"   # HTTPS over HTTP proxy
export HTTPS_PROXY="https://proxy.example.com:8080"  # HTTPS over HTTPS proxy

# Windows (Command Prompt)
set HTTPS_PROXY=https://proxy.example.com:8080

# Windows (PowerShell)
$env:HTTPS_PROXY="https://proxy.example.com:8080"
```

The proxy settings will be automatically detected and used for all Deezer Connect connections.

### Hook Scripts

You can use the `--hook` option to specify a script that will be executed when certain events occur. The script receives event information through environment variables.

For all events, the `EVENT` variable contains the event name. Additional variables depend on the specific event:

#### `playing`
Emitted when playback starts

Variables:
- `TRACK_ID`: The ID of the track being played

#### `paused`
Emitted when playback is paused

No additional variables

#### `track_changed`
Emitted when the track changes

Variables:
- `TRACK_ID`: The ID of the track
- `TITLE`: The track title
- `ARTIST`: The main artist name
- `ALBUM_TITLE`: The album title
- `ALBUM_COVER`: The album cover ID, which can be used to construct image URLs:
  ```
  https://e-cdns-images.dzcdn.net/images/cover/{album_cover}/{resolution}x{resolution}.{format}
  ```
  where `{resolution}` is the desired resolution in pixels (up to 1920) and
  `{format}` is either `jpg` (smaller file size) or `png` (higher quality).
  Deezer's default is 500x500.jpg
- `DURATION`: Track duration in seconds

#### `connected`
Emitted when a Deezer client connects to control playback

Variables:
- `USER_ID`: The Deezer user ID
- `USER_NAME`: The Deezer username

#### `disconnected`
Emitted when the controlling Deezer client disconnects

No additional variables

#### Example
All string values are properly escaped for shell safety.

```bash
#!/bin/bash
# example-hook.sh
echo "Event: $EVENT"
case "$EVENT" in
  "track_changed")
    echo "Track changed: $TITLE by $ARTIST"
    ;;
  "connected")
    echo "Connected as: $USER_NAME"
    ;;
esac
```

### Stateless Configuration

**pleezer** operates statelessly and loads user settings, such as normalization and audio quality, when it connects. To apply changes, disconnect and reconnect. This limitation is due to the Deezer Connect protocol.

Command-line options handle settings that cannot be managed through the Deezer Connect protocol.

### Configuring the Secrets File

For authentication, **pleezer** requires a `secrets.toml` file containing either:

- **email** and **password**: Your Deezer account email address and password, or
- **arl**: The Authentication Reference Link for your Deezer account. If present, this will override the email and password authentication. ARLs expire over time, so using email and password authentication is preferred for long-term access.

In addition to the authentication keys, the `secrets.toml` file can also include the following optional key:

- **bf_secret** (optional): The secret for computing the track decryption key. If not provided, **pleezer** will attempt to extract it from Deezer's public resources. Providing this secret is optional and **pleezer** does not include it to prevent piracy.

**Important:** Keep your `secrets.toml` file secure and private. Do not share it, as it contains sensitive information that can give unauthorized access to your Deezer account.

To obtain the ARL:

1. Visit [Deezer login callback](https://www.deezer.com/desktop/login/electron/callback) and log in.
2. Copy the Authentication Reference Link (ARL) from the button shown. The ARL link will look like `deezer://autolog/...`. You only need the part after `deezer://autolog/` (i.e., `...`).
3. Keep this link confidential as it grants full access to your account.

Here are examples of a `secrets.toml` file:

**Using email and password for authentication:**

```toml
email = "your-email@example.com"
password = "your-password"
```

**Using ARL for authentication (with optional bf_secret):**

```toml
arl = "your-arl"
bf_secret = "your-bf-secret"
```

You can start with the [`secrets.toml.example`](https://github.com/roderickvd/pleezer/blob/main/secrets.toml.example) file provided in the repository as a template.

## Troubleshooting

If you encounter any issues while using **pleezer**, visit our [GitHub Discussions](https://github.com/roderickvd/pleezer/discussions) for help and advice.

Common volume-related issues:
- Volume at maximum when connecting: Use `--initial-volume` to set a lower starting level
- Volume variations between tracks: Enable `--normalize-volume` for consistent playback levels

## Building pleezer

**pleezer** is supported on Linux and macOS with full compatibility. Windows support is tier two, meaning it is not fully tested and complete compatibility is not guaranteed. Contributions to enhance Windows support are welcome.

### Setting Up Your Build Environment

Before building **pleezer**, make sure your system is set up with a build environment.

#### Linux

1. Install the necessary build tools and dependencies:
  - On Debian/Ubuntu:
    ```bash
    sudo apt-get update
    sudo apt-get install build-essential libasound2-dev pkgconf
    ```
  - On Fedora:
    ```bash
    sudo dnf groupinstall 'Development Tools'
    sudo dnf install alsa-lib-devel
    ```

2. Install the latest version of Rust using [rustup](https://rustup.rs/). Follow the instructions on the rustup website for the most current setup commands.

3. Install Git (optional for the development version):
    ```bash
    sudo apt-get install git  # On Debian/Ubuntu
    sudo dnf install git      # On Fedora
    ```

#### macOS

1. Install the necessary build tools:
  - Install Xcode from the [App Store](https://apps.apple.com/us/app/xcode/id497799835). Then install the Xcode Command Line Tools by running:
    ```bash
    xcode-select --install
    ```

2. Install Rust using [rustup](https://rustup.rs/). Follow the instructions on the rustup website for the most current setup commands.

Note: Git comes pre-installed on macOS, so no additional Git installation is needed.

#### Windows

1. Set up a build environment by installing Visual Studio with the required components, following the instructions on the [Visual Studio official site](https://visualstudio.microsoft.com/).

2. Install the latest version of Rust using [rustup](https://rustup.rs/). Follow the instructions on the rustup website for the most current setup commands.

3. Install Git (optional for the development version):
  - Download and install Git from the [official site](https://git-scm.com/).

### Advanced Audio Backends

**pleezer** supports additional audio backends that can be enabled at compile time:

#### JACK Backend (Linux only)

[JACK](https://jackaudio.org/) (JACK Audio Connection Kit) is a professional audio server that enables routing audio between applications. It provides flexible routing capabilities.

1. Install JACK development files:
  - On Debian/Ubuntu:
    ```bash
    sudo apt-get install libjack-dev
    ```
  - On Fedora:
    ```bash
    sudo dnf install jack-audio-connection-kit-devel
    ```

Then build with JACK support:
```bash
cargo build --features jack
```

#### ASIO Backend (Windows only)

[ASIO](https://helpcenter.steinberg.de/hc/en-us/articles/17863730844946-Steinberg-built-in-ASIO-Driver-information-download) (Audio Stream Input/Output) is a low-latency audio driver protocol developed by Steinberg. It bypasses the Windows audio mixer to provide direct hardware access and sub-millisecond latency.

1. Install the Steinberg ASIO SDK
2. Configure build environment following the [CPAL documentation](https://docs.rs/crate/cpal/latest)

Then build with ASIO support:
```bash
cargo build --features asio
```

## Contributing

We appreciate and encourage contributions to **pleezer**! Whether you're fixing bugs, adding features, or improving documentation, your involvement is valuable.

### How to Contribute

1. **Submit Issues**: Submit issues for bugs or feature requests by [reporting an issue](https://github.com/roderickvd/pleezer/issues). Include detailed debug logs and be responsive to follow-up questions and testing. Inactive issues may be closed. This is not a general help forum; issues should focus on **pleezer** itself, not your system configuration.

2. **Create Pull Requests**: To contribute code changes or improvements, submit a [pull request](https://github.com/roderickvd/pleezer/pulls). Follow the coding standards in the [Contributing Guidelines](https://github.com/roderickvd/pleezer/blob/main/CONTRIBUTING.md).

3. **Participate in Discussions**: Engage in [GitHub discussions](https://github.com/roderickvd/pleezer/discussions) to offer feedback, stay informed, and collaborate with the community.

For more details on contributing, refer to the [Contributing Guidelines](https://github.com/roderickvd/pleezer/blob/main/CONTRIBUTING.md).

## Support My Work

If you appreciate the effort and dedication put into **pleezer** and other open-source projects, consider supporting my work through GitHub Sponsorships. Your contributions help me continue developing and improving software, and they make a meaningful difference in the sustainability of these projects.

Become a sponsor today at [github.com/sponsors/roderickvd](https://github.com/sponsors/roderickvd). Thank you for your support!

## Related Projects

There are several projects that have influenced **pleezer**. Here are a few:

- [deezer-linux](https://github.com/aunetx/deezer-linux): An unofficial Linux port of the native Deezer Windows application, providing offline listening capabilities.
- [librespot](https://github.com/librespot-org/librespot): An open-source client library for Spotify with support for Spotify Connect.
- [lms-deezer](https://github.com/philippe44/lms-deezer): A plugin for Logitech Media Server to stream music from Deezer.

## License

**pleezer** is licensed under the [Sustainable Use License](https://github.com/roderickvd/pleezer/blob/main/LICENSE.md). This license promotes [fair use](https://faircode.io) and sustainable development of open-source software while preventing unregulated commercial exploitation.

### Non-Commercial Use

You may use, modify, and distribute **pleezer** freely for non-commercial purposes. This includes integrating it into other software or hardware as long as these offerings are available at no cost to users.

### Commercial Use

If you intend to use **pleezer** in a commercial context—such as incorporating it into paid software or hardware products, or any offering that requires payment—you must obtain a separate commercial license. Commercial use includes:

- Bundling **pleezer** with software or hardware that requires payment to unlock features or access.
- Distributing **pleezer** as part of a paid product or service.

For example, **pleezer** can be included in free software or hardware, but if it is part of a product or service that charges for access or additional features, a commercial license is required.

This approach addresses challenges seen with projects like [librespot](https://github.com/librespot-org/librespot), which, despite widespread use, has seen limited contributions. By requiring commercial users to obtain a separate license, we aim to promote fair contributions and support the ongoing development of **pleezer**.

## Important Information

### Important Disclaimer

**pleezer** is an independent project and is not affiliated with, endorsed by, or created by Deezer. It is developed to provide a streaming player that is fully compatible with the Deezer Connect protocol.

**pleezer** **does not and will not support** saving or extracting music files for offline use. This project is committed to respecting artists' rights and strongly opposes piracy. Users must not use **pleezer** to infringe on intellectual property rights.

### Deezer Terms of Service

When using **pleezer**, you must comply with [Deezer's Terms of Service](https://www.deezer.com/legal/cgu). This includes, but is not limited to:

- Using the software only for permitted purposes, such as personal or family use.
- Avoiding any activities that violate Deezer's policies or terms.

It is your responsibility to thoroughly understand and adhere to Deezer's Terms of Service while using **pleezer**.

### Security

For information on how security is handled, including how to report vulnerabilities, please refer to the [Security Policy](https://github.com/roderickvd/pleezer/blob/main/SECURITY.md).

## Contacting the Author

For general inquiries, please use [GitHub Issues](https://github.com/roderickvd/pleezer/issues) or [GitHub Discussions](https://github.com/roderickvd/pleezer/discussions). For commercial licensing or to report security vulnerabilities, you may contact me directly via email. Please avoid using email for general support or feature requests.
