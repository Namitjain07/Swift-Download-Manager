# Changelog

All notable changes to Swift Download Manager are listed here, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.7.0-beta.1] - 2026-11-06

**Pre-release.** The stable version stays 1.4.0; 1.5.0-beta.1 (several networks at once) and 1.6.0-beta.1 (web page and `--server`) are included.

All of it comes in a redesigned window.

### A new design
- **One design** replaces Drishti and iOS (the choice under View > Appearance > Design and in Options is gone, and so is `--style`). It is inspired by
  Airbnb: white cards on a soft grey page, near-black text, one **coral** accent, 16 px rounded cards with soft shadows, a header with the menus, a
  search box and the main action, and flat colours only (no gradients). Light and dark; a new install follows the system (settings saved by older
  versions are read as before)
- **iOS-style controls**: capsule buttons (primary, quiet, outlined, plain, red), a 51 x 31 switch, a stepper instead of sliders, a segmented control,
  round tick boxes, round icon buttons and wheel pickers
- **Icons from the Phosphor set** (line weight, with solid play, pause and stop) replace the hand-drawn ones, and text is set in **Inter**. Both fonts are
  bundled (licences in `source/assets/fonts/`), so the window looks the same on every system
- Every dialog is a sheet with a header, a scrolling body and a footer that always stays in view. Dialogs dim the page, close with Escape or the x and do
  not close by a stray click, so a half-filled form is never lost. Forms put the label over the field and use the same 40 px height for every input
- Menus have one row height, labels and shortcuts that never run together, check marks and red destructive rows. On narrow windows the five menus fold
  into one button
- The web page has the same design (light, dark, phone), and its sign-in page too
- The table leaves out its last two columns (when it was last tried, and the description) on medium-width windows instead of running off the edge; a
  failed download's reason is then shown when you point at its Error chip

### Fixed
- **Dialog buttons were cut off** when the window was short: the Scheduler (and others) lost their Save and Cancel buttons. They now stay in view and the
  body scrolls
- Fields, buttons and labels were not aligned in the dialogs (inputs of different heights and fill, "Browse…" sticking out of its column, the main
  action on the wrong side of a row of buttons, labels and their fields on different lines). Everything now comes from one set of widgets with fixed
  sizes
- Shortcuts (Delete, Ctrl+N…) no longer act behind an open dialog
- A download with an empty "Save to" folder is refused instead of being saved relative to the program's folder
- The toolbar says **Pause** and **Pause all** (it said Stop, which was not what it did)

## [1.6.0-beta.1] - 2026-11-02

**Pre-release.** The stable version stays 1.4.0; 1.5.0-beta.1 (several networks at once) is included.

### Use it from a web browser
- Options > Web interface: turn it on and open the link (it carries a secret key that is swapped for a session cookie). Only this computer can reach it
  (`127.0.0.1`, port 9615 by default). Add links, pause, resume, remove and download finished files from any browser, in light or dark, on a phone or a desktop
- The page is one self-contained file that loads nothing from other sites; everything shown is put on the page as plain text
- It also works while the program runs in the background with only a tray icon

### Run it for your organisation
- `swift_download_manager --server` runs with no window: several people sign in with their own accounts, add links, watch them download **on the server**
  and fetch the finished files through the web address. Each person sees only their own downloads and gets a folder of their own; administrators see everybody's
- Accounts are made with `--add-user NAME [--admin]` (the password comes from `SDM_PASSWORD` or the keyboard), `--remove-user NAME` and `--list-users`.
  Passwords are stored as salted PBKDF2-HMAC-SHA256 hashes; five wrong passwords block an address for a few minutes
- Links that lead to private or local addresses (the office network, `localhost`, the cloud metadata address) are **refused by default**, including after redirects;
  `--allow-private` turns that off
- Finished files are sent in pieces on request, so a broken transfer can carry on
- A **Docker image** is published to `ghcr.io/namitjain07/swift-download-manager` (pre-releases get the `beta` tag, never `latest`); see the README
- Put it behind an HTTPS reverse proxy before it leaves a trusted network: it speaks plain HTTP itself

### Under the hood
- Downloads have an owner; an AI assistant (MCP) and the personal web page are not tied to a person
- Fixed: the .deb of a pre-release now has a file name GitHub keeps as it is

## [1.5.0-beta.1] - 2026-11-02

**Pre-release.** Try it, and tell us what you find; the stable version stays 1.4.0.

### Download over several networks at once
- Options > Networks: turn on **Download over several networks at once** and tick the network cards to use (Wi-Fi, Ethernet, a phone's USB
  or hotspot connection, ...). Every download is then spread over all of them, so the speeds add up
- Each connection is tied to one network card. Every card always gets a connection and the faster cards get more, so a slow card never holds
  the others back; when a card stops working (cable pulled, phone out of range) it is left alone for 30 seconds while the rest carry on
- The progress window shows what each network is carrying
- Nothing falls back to the normal connection when networks are chosen: if none of them works the download says so
- Linux ties the traffic to the card with `SO_BINDTODEVICE` (kernel 5.7 or newer, otherwise the card's address is used), macOS with `IP_BOUND_IF`,
  Windows with the card's address. How well this works on a given Windows or macOS computer has not been tried on real hardware yet

### Fixed
- The Options window was cut short at the same height however big the window was, hiding most of its settings behind a tiny scroll area;
  it now uses the room there is

## [1.4.0] - 2026-11-02

### A responsive window with cards
- Downloads are shown as **cards** in a grid that gains columns as the window widens: file tile, name, state chip, progress, speed and time left, and
  pause/resume, open, folder and remove buttons on each card. The classic table is one click away (View > Show downloads as)
- Summary tiles for speed, active, queued and finished downloads, and a proper empty state with a button
- The window adapts to its width: the sidebar becomes an icon rail and then a drawer, the toolbar goes from labelled tiles to icon tiles to a few
  essentials plus an overflow menu, the status bar sheds details, dialogs never grow past the window. Smaller minimum window size
- New reusable components (icon buttons, stat tiles, empty state, an icon-only segmented control) and ten new line icons

### Runs in the background
- Closing the window while downloads run asks whether to **keep downloading in the background**; remember the answer under Options > Background
- The background process shows an icon in the system tray (Windows, macOS, Linux): click to open the window again (it takes the downloads over without
  interrupting them), pause or resume everything, or quit. Queues, schedules and the browser add-on keep working with no window
- Optional **start with the system**, in the background
- Tasks > Keep running in the background; Tasks > Exit always quits

### Downloads in this release
- Windows 10/11 (64-bit): `SwiftDownloadManager.exe`, signed, and a `.zip` that also holds the browser add-on
- Ubuntu 22.04+, Debian 12+, Mint 21+ (64-bit): `.deb` installer
- Browser add-on packages and the source code as a `.zip`
- `SHA256SUMS` to check the files
- The Mac app has to be built on a Mac; see `mac/HOW TO GET THE MAC VERSION.txt`

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

[1.4.0]: https://github.com/Namitjain07/Swift-Download-Manager/releases/tag/v1.4.0
[1.3.0]: https://github.com/Namitjain07/Swift-Download-Manager/releases/tag/v1.3.0
[1.2.0]: https://github.com/Namitjain07/Swift-Download-Manager/releases/tag/v1.2.0
