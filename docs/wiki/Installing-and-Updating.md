# Installing and updating

The current version is **{{version}}**, a free beta. Every release, with its notes, is on the
[Releases page](https://github.com/PIRA-Painting-Illustration/PIRA/releases).

| System | Download | How to install |
|---|---|---|
| Android tablets (64-bit ARM, Android 8 or newer) | [pira-{{version}}-arm64-v8a.apk](https://github.com/PIRA-Painting-Illustration/PIRA/releases/download/v{{version}}/pira-{{version}}-arm64-v8a.apk) | Open the APK on the tablet. Android asks to allow installs from that source the first time. |
| Windows 10 and 11 (64-bit) | [pira-{{version}}-windows-x64.zip](https://github.com/PIRA-Painting-Illustration/PIRA/releases/download/v{{version}}/pira-{{version}}-windows-x64.zip) | Unzip it anywhere and run `PIRA.exe`. |
| Linux (x86_64) | [pira-{{version}}-linux-x86_64.tar.gz](https://github.com/PIRA-Painting-Illustration/PIRA/releases/download/v{{version}}/pira-{{version}}-linux-x86_64.tar.gz) | Extract it and run `./pira`. |

There's no Mac build.

PIRA is a beta: keep copies of work that matters, and report problems on
[GitHub](https://github.com/PIRA-Painting-Illustration/PIRA/issues).

## Android

- PIRA is made for pen tablets with a pressure-sensitive stylus. A finger works too, but pressure
  and tilt need a pen.
- Installing a newer APK over an older one keeps your settings, brushes and files.

## Windows and Linux

- The desktop builds are portable: no installer, and nothing goes into the system folders.
- Windows may warn that the publisher is unknown, because PIRA isn't code-signed yet. Choose
  **More info › Run anyway**.
- Pen tablets work through the system's pen support, and on Windows also through WinTab
  (Wacom and similar drivers).
- Settings are kept in `%APPDATA%\PIRA` on Windows and `~/.config/pira` on Linux.
- **Portable mode:** put an empty file named `portable.txt` next to `PIRA.exe` (or `pira`) and
  PIRA keeps its settings in a `PIRAData` folder beside it instead, for a copy on a USB stick.

## Updates

- **Help › Check for Updates…** looks for a newer release and offers to download it.
- Preferences can check automatically at startup.
- On Android the update downloads the new APK and hands it to Android's installer.
- A beta is offered the next beta and then the full release. A full release is only offered
  other full releases.
- The check needs no account: it reads PIRA's public releases on GitHub.
- **Copies from before 0.2.0-beta** (the 0.1.0 test builds) can't update themselves. Install
  0.2.0-beta by hand once, over the top: your settings and files are kept.
