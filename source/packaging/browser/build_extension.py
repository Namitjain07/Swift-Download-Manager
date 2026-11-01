"""Builds the browser add-on packages.

Run from the app folder:  python3 packaging/browser/build_extension.py
Makes, in dist/browser/:
  SwiftDownloadManager-Helper-<version>-chrome.zip   Chrome, Edge, Brave, Opera (upload to their add-on
                                                     stores, or unzip and use "Load unpacked")
  SwiftDownloadManager-Helper-<version>-firefox.xpi  Firefox (upload to addons.mozilla.org to sign it,
                                                     or load it in about:debugging)
  chrome/ and firefox/                               The same, unpacked
"""

import json
import os
import shutil
import zipfile

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
SRC = os.path.join(ROOT, "browser-extension")
OUT = os.path.join(ROOT, "dist", "browser")


def files():
    for d, _, fs in os.walk(SRC):
        for f in sorted(fs):
            full = os.path.join(d, f)
            yield full, os.path.relpath(full, SRC).replace(os.sep, "/")


def build(kind, manifest):
    folder = os.path.join(OUT, kind)
    shutil.rmtree(folder, ignore_errors=True)
    os.makedirs(folder)
    for full, rel in files():
        if rel == "manifest.json":
            continue
        dst = os.path.join(folder, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(full, dst)
    with open(os.path.join(folder, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    ext = "xpi" if kind == "firefox" else "zip"
    package = os.path.join(OUT, f"SwiftDownloadManager-Helper-{manifest['version']}-{kind}.{ext}")
    with zipfile.ZipFile(package, "w", zipfile.ZIP_DEFLATED) as z:
        # Stores want manifest.json at the top of the zip.
        for d, _, fs in os.walk(folder):
            for f in sorted(fs):
                full = os.path.join(d, f)
                z.write(full, os.path.relpath(full, folder).replace(os.sep, "/"))
    return package


def main():
    with open(os.path.join(SRC, "manifest.json")) as f:
        base = json.load(f)
    # Chrome-family browsers use a background service worker and don't know Firefox's settings.
    chrome = json.loads(json.dumps(base))
    chrome["background"] = {"service_worker": "background.js"}
    chrome.pop("browser_specific_settings", None)
    # Firefox runs the same script as a background page.
    firefox = json.loads(json.dumps(base))
    firefox["background"] = {"scripts": ["background.js"]}
    os.makedirs(OUT, exist_ok=True)
    for kind, m in (("chrome", chrome), ("firefox", firefox)):
        print("Made", os.path.relpath(build(kind, m), ROOT))


if __name__ == "__main__":
    main()
