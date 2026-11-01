Swift Download Manager 1.2.0 - written in Rust

What's in this folder
  SwiftDownloadManager-1.2.0-source.zip   All the source code, zipped
  source/                                  The same source code, unzipped to look at
  linux/swift-download-manager_1.2.0_amd64.deb
                                           Installer for Ubuntu 24.04+, Debian 13+, Mint 22+ (64-bit PC).
                                           Double-click it, or run: sudo apt install ./swift-download-manager_1.2.0_amd64.deb
  windows/SwiftDownloadManager.exe         Windows 10/11 (64-bit) program: just double-click it, no installing needed.
  windows/SwiftDownloadManager-1.2.0-windows-x64.zip
                                           The same program together with the browser add-on and read-me.
                                           Windows may warn about an unknown app: "More info", then "Run anyway".
  mac/HOW TO GET THE MAC VERSION.txt       The Mac app has to be built on a Mac (or by GitHub); see that file.

Browser add-on
  browser-extension/SwiftDownloadManager-Helper-1.2.0-chrome.zip    Chrome, Edge, Brave, Opera
  browser-extension/SwiftDownloadManager-Helper-1.2.0-firefox.xpi   Firefox
  browser-extension/chrome-unpacked/                                Chrome version, already unzipped
  Install steps: browser-extension/HOW TO INSTALL.txt (also in the app: Help > Browser integration).

Older Linux systems (e.g. Ubuntu 22.04): build from the source with
  bash packaging/linux/build-deb.sh   (see the source's README.md), or use the GitHub build.
