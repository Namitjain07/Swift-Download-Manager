# Changelog

All notable changes to Swift Download Manager are listed here, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.3.0] - 2026-11-02

### Two designs: Drishti (new default) and iOS
- New **Drishti** design, built from the Drishti design system: dark-first palette with one blue accent, `-ink` text colours that hold their
  contrast in both modes, 36 px rounded controls, a 220 px sidebar rail with a blue-tinted active row, the download list on its own rounded
  surface with 48 px rows, state chips that always carry a word, 20 px dialogs with the design system's sheet shadow, and a 44 x 26 switch
- The flat **iOS** design stays as an option: capsule buttons, bordered grouped cards, compact rows. Switch under **View > Appearance > Design**
  or in **Options**; each design has Automatic, Light and Dark modes. New installs start in Drishti, dark
- `--style drishti|ios` for screenshots

### A new look
- Flat, iOS-style design with **light and dark mode**; it follows the system by default and can be changed under **View → Appearance**
- New widgets: capsule buttons, switches, segmented controls, grouped cards, wheel pickers for the scheduler times,
  spinner wheels and gliding progress bars
- Toolbar and sidebar redrawn: line icons that follow the theme and a selection highlight that slides between rows
- Download list with taller rows, status dots and thin progress bars; dialogs redrawn as sheets
- Animations: buttons fade and press, windows fade in, toasts slide up. The window only repaints while something moves
- `--theme light|dark` and `--show <window>` for taking screenshots

### Windows and a new icon
- **Fixes the Windows Defender false positive** `Trojan:Win32/Bearfoos.A!ml` reported for 1.2.0. The Windows program is now built with
  Microsoft's own toolchain on GitHub (it was cross-compiled before), carries an icon, version information and a manifest (it never asks
  for administrator rights), has the C runtime built in, and is **signed** with the project's own code-signing certificate
  (fingerprint in SECURITY.md; the private key is kept out of the repository)
- New app icon: a glowing globe with a green arrow, in the window, the About window, the installers and the browser add-on
- Every release file is now built by GitHub (Windows, Linux `.deb`, browser add-ons, source); the old binaries are no longer kept in the
  repository. The `.deb` now works on Ubuntu 22.04+ and Debian 12+

### AI assistants (MCP)
- New `--mcp` mode: AI assistants such as Claude can list, add, pause, resume and remove downloads, and run queues
  (ten tools, marked read-only, destructive or idempotent so clients can ask first)
- Works with the window open (the assistant uses it) or closed (the downloads run without a window); opening the
  window while a download runs hands everything over and carries on, byte for byte
- On by default, with a switch under Options and Help > AI assistants; a status-bar chip shows the state
- Safe by design: assistants cannot delete files, choose folders, or see passwords and cookies; only `http` and `https`
  links; the control door listens on your computer only and needs a secret token, and refuses web pages
- Closing the window now saves the download list

### Downloads in this release
- Windows 10/11 (64-bit): `SwiftDownloadManager.exe`, and a `.zip` that also holds the browser add-on
- Ubuntu 22.04+, Debian 12+, Mint 21+ (64-bit): `.deb` installer
- Browser add-on packages and the source code as a `.zip`
- `SHA256SUMS` to check the files
- The Mac app has to be built on a Mac; see `mac/HOW TO GET THE MAC VERSION.txt`

## [1.2.0]

First release published on GitHub.

### Download engine
- Splits each download into up to 32 connections, and gives a free connection half of the biggest remaining part
- Pause and resume, also after the app was closed; broken connections are retried and busy servers get fewer connections
- SHA-256 checksum check and refreshing a link that has expired
- Overall and per-download speed limits, a proxy, saved site logins, and a referrer and login per download

### Organising downloads
- Several queues, each with its own schedule (start and stop times, daily or once, close the app when done)
- Categories with your own file types and folders, search, and sorting by any column
- Batch downloads (a list of links, or numbered links like `img*.jpg`), import and export of link lists,
  and dropping link files onto the window
- A link grabber that lists the files a web page links to
- Clipboard watching: copy a download link and the app offers to download it

### Browser add-on
- Chrome, Edge, Brave, Opera (`.zip`) and Firefox (`.xpi`)
- Catches downloads, passes the browser login (cookies) along, adds a right-click "Download with Swift Download Manager"
  item and spots videos and sounds a page plays

### Downloads in this release
- Windows 10/11 (64-bit): `SwiftDownloadManager.exe`, and a `.zip` that also holds the browser add-on
- Ubuntu 24.04+, Debian 13+, Mint 22+ (64-bit): `.deb` installer
- Browser add-on packages and the source code as a `.zip`
- `SHA256SUMS` to check the files
- The Mac app has to be built on a Mac; see `mac/HOW TO GET THE MAC VERSION.txt`

[1.3.0]: https://github.com/Namitjain07/Swift-Download-Manager/releases/tag/v1.3.0
[1.2.0]: https://github.com/Namitjain07/Swift-Download-Manager/releases/tag/v1.2.0
