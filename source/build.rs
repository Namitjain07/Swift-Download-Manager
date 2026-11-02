//! Gives the Windows program an icon, version information and a manifest.
//!
//! A program with none of these looks anonymous, and Windows Defender's machine-learning check
//! (for example `Trojan:Win32/Bearfoos.A!ml`) then treats it with suspicion. Other systems skip this.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=packaging/windows/app.ico");
    println!("cargo:rerun-if-changed=packaging/windows/app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("packaging/windows/app.ico");
    res.set_manifest_file("packaging/windows/app.manifest");
    res.set("ProductName", "Swift Download Manager");
    res.set("FileDescription", "Swift Download Manager: a download manager with split downloads, queues and AI assistant support");
    res.set("CompanyName", "Swift Download Manager");
    res.set("LegalCopyright", "Copyright (c) Swift Download Manager contributors");
    res.set("OriginalFilename", "SwiftDownloadManager.exe");
    res.set("InternalName", "swift_download_manager");
    if let Err(e) = res.compile() {
        // Do not fail the build over this, but make it visible: the program still works without it.
        println!("cargo:warning=could not embed the Windows resources: {e}");
    }
}
