# out/: programs and installers to try

This folder is for things you can install and test. Everything in it except this file is ignored by git
(`/out/*` in `.gitignore`), so it never ends up in a commit.

## Made from the current code (1.9.0)

| File | What it is |
| --- | --- |
| `SwiftDownloadManager-1.9.0-setup-x64.exe` | **The Windows installer** (64-bit Intel/AMD), signed with the project's own certificate. Double-click it. Windows may say "unknown publisher": choose **More info**, then **Run anyway** (the certificate is `source/packaging/windows/swift-download-manager-codesign-2.cer`). Quiet install: `setup.exe /VERYSILENT`. It installs for you without administrator rights. |
| `SwiftDownloadManager-1.9.0-x64.exe` | The program on its own, signed. No installing: double-click it. (This is the file the in-app update replaces when it was started from a folder.) |

Things worth trying: the **welcome** on the first start (press Skip or go through the six steps), a download with
the progress window minimized (the minus button), **Cancel** in the progress window, **Help > Check for updates**
(the list of other versions is there too), the tray icon's menu, and the browser add-on in `browser-extension/chrome-unpacked`.

To see the welcome again, set `"onboarded": false` inside `"settings"` in `%APPDATA%/swift-download-manager/downloads.json` while the program is closed (or start it with `--demo --show onboarding` for a look without changing anything).

## Older: built before the last round of changes

The archives named `swift-download-manager-1.9.0-<system>-<cpu>.tar.gz` / `.zip` come from the Docker toolchain
(`make cross`, see `source/packaging/cross/README.md`) as it was **before** the faster engine, the OpenGL renderer,
the welcome, the updates and the new icon. They are good for checking that a platform builds and starts, not for
trying the newest features. Rebuild them with `make cross` (needs Docker; about an hour for all 22 platforms), or
let GitHub build the release (`release.yml`): it makes the Windows, Debian and Arch files for Intel/AMD and ARM.
