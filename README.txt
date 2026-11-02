Swift Download Manager - written in Rust

The ready-made programs are on the Releases page, not in this folder:
  https://github.com/Namitjain07/Swift-Download-Manager/releases/latest

They are built, and for Windows signed, by GitHub from the source code in the release (see
.github/workflows/release.yml), so what you download is what this code builds. A SHA256SUMS file
comes with every release.

What's in each release
  SwiftDownloadManager.exe                    Windows 10/11 (64-bit) program: just double-click it, no installing needed.
                                              It is signed with the project's own certificate, so Windows may say the
                                              publisher is unknown: "More info", then "Run anyway" (see SECURITY.md).
  SwiftDownloadManager-<version>-windows-x64.zip
                                              The same program together with the browser add-on and the certificate.
  swift-download-manager_<version>_amd64.deb  Installer for Ubuntu 22.04+, Debian 12+, Mint 21+ (64-bit PC).
                                              Double-click it, or run: sudo apt install ./swift-download-manager_<version>_amd64.deb
  SwiftDownloadManager-Helper-<version>-chrome.zip    Browser add-on for Chrome, Edge, Brave, Opera
  SwiftDownloadManager-Helper-<version>-firefox.xpi   Browser add-on for Firefox
  SwiftDownloadManager-<version>-source.zip   All the source code, zipped

What's in this folder
  source/                                The source code
  browser-extension/chrome-unpacked/     The Chrome add-on, unzipped (for "Load unpacked")
  browser-extension/HOW TO INSTALL.txt   How to install the add-on (also in the app: Help > Browser integration)
  mac/HOW TO GET THE MAC VERSION.txt     The Mac app has to be built on a Mac; see that file.
