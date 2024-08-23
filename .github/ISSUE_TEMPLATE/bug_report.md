---
name: Bug report
about: Create a report to help us improve
title: ''
labels: bug
assignees: roderickvd

---

## Describe the Bug

A clear and concise description of what the bug is. Mention how often the bug occurs (e.g., always, sometimes, rarely) if applicable.

## Steps to Reproduce

Steps to reproduce the behavior:

1. **Launch pleezer** with the command '...'
2. **Use the Deezer client** to '...' (e.g., play a song, skip to the next track, control volume).

## Logs

Please include a full verbose log from launch to the issue. Use the `-v` or `-vv` flag to enable verbose logging. If the log file is too large, please attach it as a file.

Example:
```
[2024-08-23T10:04:31Z DEBUG pleezer] Command Args {
        secrets_file: "secrets.toml",
        name: None,
        interruptions: true,
        quiet: false,
        verbose: 1,
    }
[2024-08-23T10:04:31Z INFO  pleezer] starting pleezer/0.1.0; debug; en
[2024-08-23T10:04:31Z DEBUG pleezer::remote] remote version: 1000
[2024-08-23T10:04:31Z DEBUG pleezer::remote] remote scheme: wss
[2024-08-23T10:04:31Z DEBUG pleezer::gateway] client id: 475417587
[2024-08-23T10:04:31Z DEBUG pleezer::remote] user id: 1234567890
[2024-08-23T10:04:31Z INFO  pleezer::remote] user casting quality: High Fidelity
[2024-08-23T10:04:31Z DEBUG pleezer::remote] user data time to live: 1295939s
[2024-08-23T10:04:32Z DEBUG pleezer::remote] subscribing to 1234567890_1234567890_STREAM
[2024-08-23T10:04:32Z DEBUG pleezer::remote] subscribing to 1234567890_1234567890_REMOTEDISCOVER
[2024-08-23T10:04:32Z INFO  pleezer::remote] ready for discovery
...
```

## Environment

Please complete the following information:

- **OS**: [e.g. Ubuntu 20.04, macOS 11.2, Windows 10]
- **pleezer version**: [e.g. 0.1.0]
- **Rust version** (if building from source): [e.g. 1.80.1]
- **Hardware specifications**: [e.g. Raspberry Pi 3B+, 1GB RAM]
- **Deezer client hardware and software**: [e.g. iPhone 12 with iOS 14.6, Deezer Web on Chrome, Google Pixel 5 with Android 11]

## Additional Context

Add any other context about the problem here, like your network or audio configuration, as applicable.

---

## Due Diligence

Please confirm that you have completed the following tasks by checking the boxes:

- [ ] I am using the latest version of **pleezer**.
- [ ] I have searched the issues for similar reports.
- [ ] I have included a full verbose log from launch to the issue, not just an excerpt.
- [ ] I confirm that this is an issue with **pleezer**, not with my system configuration or other software.
- [ ] I confirm that this is not a security issue, which should be reported privately.
- [ ] I have read and understood the [Contributing guidelines](CONTRIBUTING.md).
