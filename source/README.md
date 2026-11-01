# Swift Download Manager

A fast desktop download manager written in Rust (with the egui toolkit).

## What it does

- Splits each download into several connections (up to 32) and gives free connections half of the biggest remaining part, so downloads finish faster
- Pause and resume, even after closing the app; broken connections are retried; busy servers get fewer connections
- Several queues, each with its own schedule (start/stop times, daily or once, close the app when done)
- Batch downloads (a list of links, or numbered links like `img*.jpg`), import and export of link lists, and dropping link files onto the window
- A link grabber that lists the files a web page links to
- Categories with your own file types and folders, search, and sorting by any column
- Overall and per-download speed limits, a proxy, saved site logins, a referrer and login per download
- SHA-256 checksum check, a download-complete window, and refreshing a link that has expired
- Browser add-on (Chrome, Edge, Brave, Firefox) in `browser-extension/`: catches downloads, passes your browser login (cookies) along, adds a right-click "Download with…" item and spots videos and sounds a page plays
- Clipboard watching: copy a download link and the app offers to download it

## Building it yourself

Install Rust from <https://rustup.rs>, then in this folder:

| For | Command | Result |
| --- | --- | --- |
| Run it now | `cargo run --release` | opens the app |
| Linux installer (.deb) | `bash packaging/linux/build-deb.sh` | `dist/*.deb` |
| Windows (from Windows) | `cargo build --release` | `target/release/swift_download_manager.exe` |
| Windows (from Linux) | `bash packaging/windows/build-windows.sh` | `dist/windows/*.zip` |
| Mac (on a Mac) | `bash packaging/macos/build-mac.sh` | `dist/*.dmg` |
| Browser add-on | `python3 packaging/browser/build_extension.py` | `dist/browser/` (Chrome .zip, Firefox .xpi) |
| Run the tests | `cargo test` | 20 automatic tests |

On Linux the app also needs the usual desktop libraries (they are already there on desktop systems).

**All three at once on GitHub:** put this folder on GitHub, open the **Actions** tab, choose
**Build installers** and press **Run workflow**. The .deb, the Windows zip and the Mac .dmg appear under **Artifacts**.

## Where things are

- `src/engine.rs`: the download engine (connections, splitting, resume, retries, speed limits)
- `src/bridge.rs`: the local link listener for the browser add-on, and clipboard watching
- `src/app.rs` and `src/app/`: the window, dialogs, queues, scheduler and link grabber
- `browser-extension/`: the browser add-on
- `packaging/`: installer scripts for Linux, Windows and Mac

Settings and the download list are saved in `~/.config/swift-download-manager/` on Linux,
`%APPDATA%\swift-download-manager\` on Windows and `~/.config/swift-download-manager/` on Mac.
