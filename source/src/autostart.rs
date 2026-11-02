//! Starting the program at login, in the background (with a tray icon and no window).
//!
//! Linux: a `.desktop` file in `~/.config/autostart`. macOS: a launch agent. Windows: a value
//! under `HKCU\...\Run`, written with the `reg` tool that ships with Windows.

use std::io;
use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

/// Text of the Linux autostart entry. The path is quoted for the desktop-entry `Exec` key.
#[allow(dead_code)] // each system uses its own; all are tested everywhere
pub fn desktop_entry(exe: &Path) -> String {
    let quoted = exe.display().to_string().replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$");
    format!(
        "[Desktop Entry]\nType=Application\nName=Swift Download Manager\nComment=Keeps downloading in the background\nExec=\"{quoted}\" --background\nIcon=swift-download-manager\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

/// Text of the macOS launch agent.
#[allow(dead_code)] // each system uses its own; all are tested everywhere
pub fn launch_agent_plist(exe: &Path) -> String {
    let path = exe.display().to_string().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>com.swiftdownloadmanager.background</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{path}</string>\n    <string>--background</string>\n  </array>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>ProcessType</key>\n  <string>Background</string>\n</dict>\n</plist>\n"
    )
}

/// The command stored in the Windows `Run` key.
#[allow(dead_code)] // each system uses its own; all are tested everywhere
pub fn run_value(exe: &Path) -> String {
    format!("\"{}\" --background", exe.display())
}

#[cfg(target_os = "linux")]
fn entry_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("autostart").join("swift-download-manager.desktop"))
}

#[cfg(target_os = "macos")]
fn entry_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/LaunchAgents/com.swiftdownloadmanager.background.plist"))
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn entry_path() -> Option<PathBuf> {
    None
}

/// The `reg` tool is a console program; this keeps its window from flashing.
#[cfg(windows)]
fn reg(args: &[&str]) -> io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg").args(args).creation_flags(0x0800_0000).output()
}

pub fn is_enabled() -> bool {
    #[cfg(windows)]
    {
        return reg(&["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "SwiftDownloadManager"]).map(|o| o.status.success()).unwrap_or(false);
    }
    #[cfg(not(windows))]
    entry_path().is_some_and(|p| p.exists())
}

pub fn enable() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    #[cfg(windows)]
    {
        let out = reg(&["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "SwiftDownloadManager", "/t", "REG_SZ", "/d", &run_value(&exe), "/f"])?;
        return if out.status.success() { Ok(()) } else { Err(io::Error::other("could not write the Run key")) };
    }
    #[cfg(not(windows))]
    {
        let path = entry_path().ok_or_else(|| io::Error::other("starting at login is not supported here"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = if cfg!(target_os = "macos") { launch_agent_plist(&exe) } else { desktop_entry(&exe) };
        std::fs::write(path, text)
    }
}

pub fn disable() -> io::Result<()> {
    #[cfg(windows)]
    {
        let _ = reg(&["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "SwiftDownloadManager", "/f"])?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        match entry_path() {
            Some(p) if p.exists() => std::fs::remove_file(p),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_linux_entry_runs_the_program_with_the_background_flag_and_quotes_odd_paths() {
        let text = desktop_entry(Path::new("/opt/Swift Download Manager/swift_download_manager"));
        assert!(text.starts_with("[Desktop Entry]\nType=Application\n"));
        assert!(text.contains("Exec=\"/opt/Swift Download Manager/swift_download_manager\" --background\n"), "{text}");
        assert!(text.contains("X-GNOME-Autostart-enabled=true"));
        let odd = desktop_entry(Path::new("/tmp/a\"b$c/app"));
        assert!(odd.contains(r#"Exec="/tmp/a\"b\$c/app" --background"#), "{odd}");
    }

    #[test]
    fn the_mac_launch_agent_is_valid_xml_even_with_ampersands_in_the_path() {
        let text = launch_agent_plist(Path::new("/Applications/Swift & Co/Swift Download Manager.app/Contents/MacOS/swift_download_manager"));
        assert!(text.contains("<string>/Applications/Swift &amp; Co/"), "{text}");
        assert!(text.contains("<string>--background</string>"));
        assert!(text.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(!text.contains("Swift & Co"), "a raw ampersand would break the plist");
    }

    #[test]
    fn the_windows_run_value_quotes_the_path() {
        assert_eq!(run_value(Path::new(r"C:\Program Files\Swift\SwiftDownloadManager.exe")), r#""C:\Program Files\Swift\SwiftDownloadManager.exe" --background"#);
    }
}
