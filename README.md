<div align="center">

# Swift Download Manager

**A fast, IDM-inspired download manager for Windows, Linux and macOS, written in Rust.**

[![Latest release](https://img.shields.io/github/v/release/Namitjain07/Swift-Download-Manager?label=release)](https://github.com/Namitjain07/Swift-Download-Manager/releases/latest)
[![License](https://img.shields.io/github/license/Namitjain07/Swift-Download-Manager)](LICENSE)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-blue)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/screenshot-dark.png">
  <img src="docs/images/screenshot.png" alt="Swift Download Manager main window in light mode: a category tree and queues on the left, a table of downloads with status dots and thin progress bars in the middle, and a toolbar of line icons on top" width="900">
</picture>

<sub>The main window in light and dark mode (it follows your system), shown with the app's built-in sample data (<code>--demo</code> mode).</sub>

<img src="docs/images/screenshot-dark.png" alt="The same window in dark mode" width="440"> <img src="docs/images/scheduler-dark.png" alt="The scheduler window in dark mode with a wheel picker for the start time" width="300">

</div>

## Why Swift Download Manager?

Most downloads use a single connection and leave your bandwidth idle. Swift Download Manager splits every file into
several parts, downloads them at the same time, and keeps going when a connection drops, so big files finish
sooner and a broken connection never means starting over.

## Features

**Fast and reliable**
- Splits each download into up to **32 connections**; a free connection takes half of the biggest remaining part
- **Pause and resume**, even after closing the app; broken connections are retried and busy servers get fewer connections
- **SHA-256 checksum** check, plus refreshing a link that has expired

**A calm, native-feeling window**
- A flat, iOS-style look with **light and dark mode** that follows your system (or choose under **View → Appearance**)
- Smooth animations: buttons fade and press, the sidebar highlight slides, progress bars glide, and windows fade in
- Switches, wheel pickers for times, and spinner wheels while something is loading. The window stays idle when nothing moves

**Organised**
- **Queues** with their own schedules (start and stop times, daily or once, close the app when done)
- **Categories** with your own file types and folders, search, and sorting by any column
- **Batch downloads**: a list of links, or numbered links such as `img*.jpg`; import and export link lists, or drop link files onto the window
- **Link grabber** that lists the files a web page links to

**In control**
- Overall and per-download **speed limits**, a **proxy**, saved site logins, and a referrer and login per download
- **Clipboard watching**: copy a download link and the app offers to download it
- A download-complete window when a file is finished

**Browser add-on** (Chrome, Edge, Brave, Opera and Firefox)
- Catches downloads you start in the browser and hands them to the app
- Passes your browser login (cookies) along, so downloads behind a login work
- Adds **Download with Swift Download Manager** to the right-click menu
- Spots videos and sounds that a page plays

## Download

Get the files from the [**latest release**](https://github.com/Namitjain07/Swift-Download-Manager/releases/latest).

| Platform | File | How to install |
| --- | --- | --- |
| **Windows 10/11** (64-bit) | `SwiftDownloadManager.exe` | Double-click it. No installer needed. Windows may warn about an unknown app: choose **More info**, then **Run anyway**. |
| **Windows**, with browser add-on | `SwiftDownloadManager-<version>-windows-x64.zip` | The same program together with the browser add-on and a short read-me. |
| **Ubuntu 24.04+, Debian 13+, Mint 22+** (64-bit) | `swift-download-manager_<version>_amd64.deb` | Double-click it, or run `sudo apt install ./swift-download-manager_<version>_amd64.deb`. |
| **macOS** | build it, see below | A Mac app can only be built on a Mac. See [`mac/HOW TO GET THE MAC VERSION.txt`](mac/HOW%20TO%20GET%20THE%20MAC%20VERSION.txt). On first start, right-click the app and choose **Open**, because it is not signed with a paid Apple developer account. |
| **Older Linux** (e.g. Ubuntu 22.04) | build it, see below | `bash packaging/linux/build-deb.sh` in [`source/`](source/). |

Every release also lists a `SHA256SUMS` file so you can check what you downloaded.

### Browser add-on

| Browser | File |
| --- | --- |
| Chrome, Edge, Brave, Opera | `SwiftDownloadManager-Helper-<version>-chrome.zip` |
| Firefox | `SwiftDownloadManager-Helper-<version>-firefox.xpi` |

The app has to be running for downloads to go to it; otherwise the browser downloads as usual.

- **Chrome, Edge, Brave, Opera:** unzip the file into a folder you keep, open `chrome://extensions` (Edge: `edge://extensions`),
  turn on **Developer mode**, click **Load unpacked** and choose the folder.
- **Firefox:** to try it now, open `about:debugging`, click **This Firefox**, then **Load Temporary Add-on…** and choose the `.xpi`.
  Temporary add-ons are removed when Firefox closes. To keep it, Firefox needs the file signed; see
  [`browser-extension/HOW TO INSTALL.txt`](browser-extension/HOW%20TO%20INSTALL.txt).

The same steps are in the app under **Help > Browser integration**.

## Build it yourself

Install Rust from <https://rustup.rs>, then, in the [`source/`](source/) folder:

| For | Command | Result |
| --- | --- | --- |
| Run it now | `cargo run --release` | opens the app |
| Linux installer (.deb) | `bash packaging/linux/build-deb.sh` | `dist/*.deb` |
| Windows (from Windows) | `cargo build --release` | `target/release/swift_download_manager.exe` |
| Windows (from Linux) | `bash packaging/windows/build-windows.sh` | `dist/windows/*.zip` |
| Mac (on a Mac) | `bash packaging/macos/build-mac.sh` | `dist/*.dmg` |
| Browser add-on | `python3 packaging/browser/build_extension.py` | `dist/browser/` (Chrome .zip, Firefox .xpi) |
| Run the tests | `cargo test` | the automatic tests |

On Linux the app also needs the usual desktop libraries (they are already there on desktop systems).
The full build notes are in [`source/README.md`](source/README.md).

### Retaking the screenshots

The app can draw itself with sample data and save a picture of its window, which is how the screenshots above were made:

```bash
cd source
cargo run --release -- --screenshot ../docs/images/screenshot.png --theme light
cargo run --release -- --screenshot ../docs/images/screenshot-dark.png --theme dark
# a window other than the main one: --show add|batch|options|scheduler|queues|properties|progress|delete|about|browser|grabber
cargo run --release -- --screenshot /tmp/scheduler.png --theme dark --show scheduler
```

On a machine without a screen, put `xvfb-run -a` in front of the program.

## What is in this repository

| Path | What it is |
| --- | --- |
| [`source/`](source/) | The Rust source code and the installer scripts |
| [`source/src/engine.rs`](source/src/engine.rs) | The download engine: connections, splitting, resume, retries, speed limits |
| [`source/src/bridge.rs`](source/src/bridge.rs) | The local link listener for the browser add-on, and clipboard watching |
| [`source/src/app.rs`](source/src/app.rs), [`source/src/app/`](source/src/app/) | The window, dialogs, queues, scheduler and link grabber |
| [`source/src/theme.rs`](source/src/theme.rs), [`source/src/widgets/`](source/src/widgets/) | The light and dark colours, and the widget kit (buttons, switch, wheel picker, progress bars) |
| [`browser-extension/`](browser-extension/) | The browser add-on, packaged for Chrome and Firefox |
| `linux/`, `windows/`, `mac/` | The ready-made program for each system |
| [`docs/`](docs/) | Images used by this read-me |

Settings and the download list are saved in `~/.config/swift-download-manager/` on Linux and macOS,
and in `%APPDATA%\swift-download-manager\` on Windows.

## Releases

Releases are listed on the [releases page](https://github.com/Namitjain07/Swift-Download-Manager/releases), and what changed
in each one is in [`CHANGELOG.md`](CHANGELOG.md). Pushing a tag such as `v1.2.0` publishes a release; see
[`.github/workflows/release.yml`](.github/workflows/release.yml).

## Contributing

Bug reports, ideas and pull requests are welcome. Please read the [contributing guide](CONTRIBUTING.md) first, and
note that everyone taking part is expected to follow the [code of conduct](CODE_OF_CONDUCT.md). To report a problem
or ask for a feature, [open an issue](https://github.com/Namitjain07/Swift-Download-Manager/issues/new/choose).

Pull requests are built and tested automatically on Linux, Windows and macOS (see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml)).

## Security

Please do not report security problems in a public issue. See the [security policy](SECURITY.md) for how to report
them privately.

## License

Released under the Apache License 2.0. See [`LICENSE`](LICENSE).
