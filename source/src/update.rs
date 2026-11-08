//! Updates: look for a newer version, download it, check it and install it. The same list of versions is
//! what the "install another version" choice (a rollback) is made from.
//!
//! Where the list comes from (Help > Check for updates > Advanced):
//! - **GitHub**: the releases of the project (`api.github.com`), the default;
//! - **The project's website**: the same list, published with the website (`data/releases.json`), for networks
//!   where the GitHub API is not reachable or is rate limited;
//! - **Your own server**: any address that serves a JSON list in the same shape (see `parse_feed`), for an
//!   organisation that mirrors the releases. Only `https` (or `http` to this computer) is accepted.
//!
//! Safety: a file is only installed when its SHA-256 matches the one the list gives for it (GitHub publishes one
//! for every file), it is only ever fetched from the address the list names, and nothing is sent to the server
//! except the request for the list and the file. Checking is optional and can be switched off.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as Atomic};
use std::time::Duration;

/// The project on GitHub.
pub const REPO: &str = "Namitjain07/Swift-Download-Manager";
/// This program's version.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// The release list as the website publishes it.
pub const WEBSITE_FEED: &str = "https://namitjain07.github.io/Swift-Download-Manager/data/releases.json";
/// The most a list of releases may weigh.
const FEED_LIMIT: u64 = 4 * 1024 * 1024;

/// Which versions an update may be: only finished ones, or also the pre-releases (betas).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

/// Where the list of versions is read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Server {
    #[default]
    GitHub,
    Website,
    Custom,
}

impl Server {
    pub fn label(self) -> &'static str {
        match self {
            Server::GitHub => "GitHub",
            Server::Website => "The project's website",
            Server::Custom => "My own server",
        }
    }

    /// The address of the list (`custom` is the person's own address, used for `Custom`).
    pub fn feed_url(self, custom: &str) -> String {
        match self {
            Server::GitHub => format!("https://api.github.com/repos/{REPO}/releases?per_page=100"),
            Server::Website => WEBSITE_FEED.to_string(),
            Server::Custom => custom.trim().to_string(),
        }
    }
}

// ---------------------------------------------------------------- versions

/// A version such as `1.9.0` or `1.10.0-beta.2`, ordered the way semantic versioning orders them
/// (`1.9.0-beta.1` comes before `1.9.0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pre: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Num(u64),
    Text(String),
}

impl Version {
    /// `v1.9.0`, `1.9.0` or `1.9.0-beta.1` (build metadata after a `+` is ignored). None for anything else.
    pub fn parse(text: &str) -> Option<Version> {
        let t = text.trim().trim_start_matches('v').split('+').next()?;
        let (core, pre) = match t.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (t, None),
        };
        let mut nums = core.split('.');
        let (major, minor, patch) = (nums.next()?.parse().ok()?, nums.next()?.parse().ok()?, nums.next()?.parse().ok()?);
        if nums.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(p) if p.is_empty() => return None,
            Some(p) => p.split('.').map(|s| s.parse().map(Part::Num).unwrap_or_else(|_| Part::Text(s.to_string()))).collect(),
            None => Vec::new(),
        };
        Some(Version { major, minor, patch, pre })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (i, p) in self.pre.iter().enumerate() {
            write!(f, "{}", if i == 0 { '-' } else { '.' })?;
            match p {
                Part::Num(n) => write!(f, "{n}")?,
                Part::Text(t) => write!(f, "{t}")?,
            }
        }
        Ok(())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                for (a, b) in self.pre.iter().zip(&other.pre) {
                    let o = match (a, b) {
                        (Part::Num(x), Part::Num(y)) => x.cmp(y),
                        (Part::Num(_), Part::Text(_)) => Ordering::Less,
                        (Part::Text(_), Part::Num(_)) => Ordering::Greater,
                        (Part::Text(x), Part::Text(y)) => x.cmp(y),
                    };
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                self.pre.len().cmp(&other.pre.len())
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// This program's own version.
pub fn current() -> Version {
    Version::parse(CURRENT).expect("the version in Cargo.toml is a valid version")
}

// ---------------------------------------------------------------- the list of releases

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub url: String,
    /// SHA-256 as 64 lowercase hex digits, when the list says.
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub prerelease: bool,
    /// Date of publishing, as the list gives it (`2026-11-06T12:30:15Z`).
    pub published: String,
    pub notes: String,
    pub url: String,
    pub assets: Vec<Asset>,
}

impl Release {
    /// The day of publishing (`2026-11-06`).
    pub fn day(&self) -> &str {
        self.published.get(..10).unwrap_or(&self.published)
    }
}

/// Reads a list of releases. The shape is the one of GitHub's API (and of the website's `data/releases.json`):
/// an array of `{ tag_name, prerelease, draft, published_at, body, html_url, assets: [{ name, size,
/// browser_download_url, digest }] }`. Drafts and entries whose tag is not a version are skipped. Newest first.
pub fn parse_feed(json: &str) -> Result<Vec<Release>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("The list of versions is not readable: {e}"))?;
    let items = v.as_array().ok_or("The list of versions is not a list")?;
    let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut out: Vec<Release> = items
        .iter()
        .filter(|r| !r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false))
        .filter_map(|r| {
            let tag = text(r, "tag_name");
            let version = Version::parse(&tag)?;
            let assets = r
                .get("assets")
                .and_then(|a| a.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|a| {
                            let (name, url) = (text(a, "name"), text(a, "browser_download_url"));
                            if name.is_empty() || url.is_empty() {
                                return None;
                            }
                            let sha256 = text(a, "digest").strip_prefix("sha256:").map(str::to_ascii_lowercase).filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()));
                            Some(Asset { name, size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0), url, sha256 })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Release {
                prerelease: r.get("prerelease").and_then(|p| p.as_bool()).unwrap_or(false) || version.is_prerelease(),
                version,
                tag,
                published: text(r, "published_at"),
                notes: text(r, "body"),
                url: text(r, "html_url"),
                assets,
            })
        })
        .collect();
    out.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(out)
}

/// The newest version the channel allows that is newer than `than`.
pub fn newest<'a>(releases: &'a [Release], channel: Channel, than: &Version) -> Option<&'a Release> {
    releases.iter().filter(|r| channel == Channel::Beta || !r.prerelease).filter(|r| r.version > *than).max_by(|a, b| a.version.cmp(&b.version))
}

// ---------------------------------------------------------------- which file

/// What kind of file an update is, and so how it is installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The Windows setup program: run it quietly; it closes this program, replaces it and starts it again.
    WindowsInstaller,
    /// The single Windows program: replace this very file.
    WindowsPortable,
    /// A Debian, Ubuntu or Mint package: handed to the system's installer.
    Deb,
    /// An Arch package: the command to run is shown.
    ArchPackage,
}

/// How this copy of the program was installed (decides which file an update is).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install {
    WindowsInstalled,
    WindowsPortable,
    LinuxDeb,
    LinuxArch,
    /// Not something an update can be fetched for (a build from source, macOS, another Linux): the release page is opened.
    Other,
}

impl Install {
    /// Where this program runs from tells how it got there.
    pub fn detect() -> Install {
        let exe = std::env::current_exe().ok();
        if cfg!(windows) {
            // The installer puts an uninstaller next to the program.
            return match exe.as_deref().and_then(Path::parent) {
                Some(dir) if dir.join("unins000.exe").exists() => Install::WindowsInstalled,
                _ => Install::WindowsPortable,
            };
        }
        if cfg!(target_os = "linux") && exe.as_deref().is_some_and(|e| e.starts_with("/usr/")) {
            if Path::new("/var/lib/dpkg/status").exists() {
                return Install::LinuxDeb;
            }
            if Path::new("/var/lib/pacman/local").exists() {
                return Install::LinuxArch;
            }
        }
        Install::Other
    }
}

/// The file of `release` that updates a copy installed the way `how` says, on this kind of computer.
pub fn pick_asset(release: &Release, how: Install) -> Option<(&Asset, Kind)> {
    let arm = cfg!(target_arch = "aarch64");
    let find = |suffixes: &[&str]| release.assets.iter().find(|a| suffixes.iter().any(|s| a.name.ends_with(s)));
    match how {
        Install::WindowsInstalled => find(&[if arm { "-setup-arm64.exe" } else { "-setup-x64.exe" }]).map(|a| (a, Kind::WindowsInstaller)).or_else(|| pick_asset(release, Install::WindowsPortable)),
        Install::WindowsPortable => release.assets.iter().find(|a| a.name == if arm { "SwiftDownloadManager-arm64.exe" } else { "SwiftDownloadManager.exe" }).map(|a| (a, Kind::WindowsPortable)),
        Install::LinuxDeb => find(&[if arm { "_arm64.deb" } else { "_amd64.deb" }]).map(|a| (a, Kind::Deb)),
        Install::LinuxArch => find(&[if arm { "-aarch64.pkg.tar.zst" } else { "-x86_64.pkg.tar.zst" }]).map(|a| (a, Kind::ArchPackage)),
        Install::Other => None,
    }
}

// ---------------------------------------------------------------- the network

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .user_agent(format!("SwiftDownloadManager/{CURRENT} (update check)"))
        .build()
        .into()
}

/// Only `https` addresses, or `http` ones that lead to this computer (for trying out a server of one's own).
fn check_address(url: &str) -> Result<(), String> {
    let u = url::Url::parse(url).map_err(|_| format!("“{url}” is not a web address"))?;
    let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "::1" | "[::1]"));
    match u.scheme() {
        "https" => Ok(()),
        "http" if local => Ok(()),
        _ => Err("Updates are only fetched over https (or from this computer)".to_string()),
    }
}

/// Fetch and read the list of releases from `url`.
pub fn fetch_feed(url: &str) -> Result<Vec<Release>, String> {
    check_address(url)?;
    let mut resp = agent().get(url).header("Accept", "application/vnd.github+json, application/json").call().map_err(|e| format!("Could not reach the update server: {e}"))?;
    let body = resp.body_mut().with_config().limit(FEED_LIMIT).read_to_string().map_err(|e| format!("Could not read the update server's answer: {e}"))?;
    parse_feed(&body)
}

/// What a download is doing, shared with the window that shows it.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

/// Download `asset` into `dir`, checking its SHA-256 on the way. The file only gets its real name once it is
/// complete and correct; a half-finished one is deleted. Refuses a file that has no checksum to compare with.
pub fn download(asset: &Asset, dir: &Path, progress: &Progress) -> Result<PathBuf, String> {
    check_address(&asset.url)?;
    let want = asset.sha256.clone().ok_or("The update server gives no checksum for this file, so it is not installed")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot make the folder for the update: {e}"))?;
    let name = Path::new(&asset.name).file_name().and_then(|n| n.to_str()).ok_or("The file has no usable name")?;
    let (target, part) = (dir.join(name), dir.join(format!("{name}.part")));
    let result = (|| -> Result<(), String> {
        let resp = agent().get(&asset.url).call().map_err(|e| format!("Could not download the update: {e}"))?;
        progress.total.store(asset.size.max(resp.body().content_length().unwrap_or(0)), Atomic::SeqCst);
        progress.done.store(0, Atomic::SeqCst);
        let mut reader = resp.into_body().into_reader();
        let mut file = std::fs::File::create(&part).map_err(|e| format!("Cannot write the update: {e}"))?;
        let (mut hash, mut buf) = (Sha256::new(), vec![0u8; 128 * 1024]);
        loop {
            if progress.cancel.load(Atomic::SeqCst) {
                return Err("Cancelled".to_string());
            }
            let n = reader.read(&mut buf).map_err(|e| format!("The download was interrupted: {e}"))?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| format!("Cannot write the update: {e}"))?;
            progress.done.fetch_add(n as u64, Atomic::SeqCst);
        }
        file.flush().map_err(|e| e.to_string())?;
        let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != want {
            return Err("The downloaded file does not match its checksum, so it was thrown away".to_string());
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            let _ = std::fs::remove_file(&target);
            std::fs::rename(&part, &target).map_err(|e| format!("Cannot finish the update file: {e}"))?;
            Ok(target)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------- installing

/// What happened when the update was handed over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Handover {
    /// The new version is taking over: this program should save and quit now.
    Quit,
    /// The system's installer was opened with the file; nothing more to do here.
    Opened(PathBuf),
    /// Nothing could be started: this is the command to run.
    Command(String),
}

/// Install the downloaded `file`. For the Windows kinds the new version replaces this one and starts; this program has
/// to quit when `Handover::Quit` comes back.
pub fn install(file: &Path, kind: Kind) -> Result<Handover, String> {
    match kind {
        Kind::WindowsInstaller => {
            // Quiet except for a progress window; it closes this program, replaces it and starts it again.
            spawn_detached(std::process::Command::new(file).args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS", "/RESTARTAPPLICATIONS"]))?;
            Ok(Handover::Quit)
        }
        Kind::WindowsPortable => {
            let me = std::env::current_exe().map_err(|e| e.to_string())?;
            replace_running_program(&me, file)?;
            spawn_detached(&mut std::process::Command::new(&me))?;
            Ok(Handover::Quit)
        }
        Kind::Deb => match std::process::Command::new("xdg-open").arg(file).spawn() {
            Ok(_) => Ok(Handover::Opened(file.to_path_buf())),
            Err(_) => Ok(Handover::Command(format!("sudo apt install '{}'", file.display()))),
        },
        Kind::ArchPackage => Ok(Handover::Command(format!("sudo pacman -U '{}'", file.display()))),
    }
}

/// Put `new` in place of the program `me` that is running: Windows lets a running program be renamed but not
/// overwritten, so the old one is moved aside (and deleted by the next start, see `clean_up`), the new one takes its place.
pub fn replace_running_program(me: &Path, new: &Path) -> Result<(), String> {
    let old = old_name(me);
    let staged = {
        let mut s = me.as_os_str().to_owned();
        s.push(".new");
        PathBuf::from(s)
    };
    std::fs::copy(new, &staged).map_err(|e| format!("Cannot put the new version next to this one: {e}"))?;
    let _ = std::fs::remove_file(&old);
    if let Err(e) = std::fs::rename(me, &old) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("Cannot move this version aside: {e}"));
    }
    if let Err(e) = std::fs::rename(&staged, me) {
        // Put things back so the program still works.
        let _ = std::fs::rename(&old, me);
        let _ = std::fs::remove_file(&staged);
        return Err(format!("Cannot put the new version in place: {e}"));
    }
    Ok(())
}

fn old_name(me: &Path) -> PathBuf {
    let mut s = me.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

/// Delete what an earlier update left behind (the moved-aside program and downloaded files). Called at start-up.
pub fn clean_up(updates_dir: &Path) {
    if let Ok(me) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_name(&me));
    }
    let _ = std::fs::remove_dir_all(updates_dir);
}

fn spawn_detached(cmd: &mut std::process::Command) -> Result<(), String> {
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: it goes on when this program ends.
        cmd.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("Could not start the installer: {e}"))
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s} is a version"))
    }

    #[test]
    fn versions_are_read_and_ordered_like_semantic_versions() {
        assert_eq!(v("v1.9.0"), v("1.9.0"));
        assert_eq!(v("1.9.0").to_string(), "1.9.0");
        assert_eq!(v("1.10.0-beta.2").to_string(), "1.10.0-beta.2");
        assert!(v("1.10.0") > v("1.9.9"), "numbers, not text: 1.10 is later than 1.9");
        assert!(v("1.9.0-beta.1") < v("1.9.0"), "a beta comes before its release");
        assert!(v("1.9.0-beta.2") > v("1.9.0-beta.1"));
        assert!(v("1.9.0-beta.10") > v("1.9.0-beta.9"), "beta.10 is later than beta.9");
        assert!(v("1.9.0-rc.1") > v("1.9.0-beta.5"), "rc comes after beta (alphabetical)");
        assert!(v("2.0.0") > v("1.99.99"));
        assert_eq!(v("1.2.3+build5"), v("1.2.3"), "build metadata does not count");
        for bad in ["", "1.2", "1.2.3.4", "one.two.three", "v1.2.x", "1.2.3-"] {
            assert!(Version::parse(bad).is_none(), "{bad:?}");
        }
        assert!(current() >= v("1.9.0"));
    }

    const FEED: &str = r#"[
      {"tag_name":"v1.10.0-beta.1","prerelease":true,"draft":false,"published_at":"2026-11-20T10:00:00Z","body":"- a beta","html_url":"https://github.com/x/y/releases/tag/v1.10.0-beta.1","assets":[]},
      {"tag_name":"v1.9.0","prerelease":false,"draft":false,"published_at":"2026-11-08T10:00:00Z","body":"- notes","html_url":"https://github.com/x/y/releases/tag/v1.9.0","assets":[
        {"name":"SwiftDownloadManager-1.9.0-setup-x64.exe","size":12,"browser_download_url":"https://example.com/setup-x64.exe","digest":"sha256:AAAA000000000000000000000000000000000000000000000000000000000001"},
        {"name":"SwiftDownloadManager.exe","size":5,"browser_download_url":"https://example.com/sdm.exe","digest":"sha256:0000"},
        {"name":"swift-download-manager_1.9.0_amd64.deb","size":7,"browser_download_url":"https://example.com/a.deb"},
        {"name":"swift-download-manager-1.9.0-1-x86_64.pkg.tar.zst","size":7,"browser_download_url":"https://example.com/a.zst"},
        {"name":"","size":1,"browser_download_url":"https://example.com/nameless"}]},
      {"tag_name":"v1.8.0","prerelease":false,"draft":false,"published_at":"2026-11-06T12:30:15Z","body":"","html_url":"","assets":[]},
      {"tag_name":"v9.9.9","prerelease":false,"draft":true,"published_at":"","body":"","html_url":"","assets":[]},
      {"tag_name":"nightly","prerelease":false,"draft":false,"published_at":"","body":"","html_url":"","assets":[]}
    ]"#;

    #[test]
    fn a_list_of_releases_is_read_newest_first_without_drafts_and_odd_tags() {
        let list = parse_feed(FEED).unwrap();
        let tags: Vec<&str> = list.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v1.10.0-beta.1", "v1.9.0", "v1.8.0"], "no draft, no 'nightly'");
        assert!(list[0].prerelease && !list[1].prerelease);
        assert_eq!(list[1].day(), "2026-11-08");
        let assets = &list[1].assets;
        assert_eq!(assets.len(), 4, "an asset without a name is left out");
        assert_eq!(assets[0].sha256.as_deref(), Some("aaaa000000000000000000000000000000000000000000000000000000000001"), "lowercased");
        assert_eq!(assets[1].sha256, None, "a digest of the wrong length is not a checksum");
        assert_eq!(assets[2].sha256, None);
        assert!(parse_feed("{}").is_err() && parse_feed("not json").is_err());
    }

    #[test]
    fn the_newest_version_depends_on_the_channel_and_on_what_runs_now() {
        let list = parse_feed(FEED).unwrap();
        assert_eq!(newest(&list, Channel::Stable, &v("1.8.0")).unwrap().tag, "v1.9.0");
        assert_eq!(newest(&list, Channel::Beta, &v("1.8.0")).unwrap().tag, "v1.10.0-beta.1");
        assert!(newest(&list, Channel::Stable, &v("1.9.0")).is_none(), "nothing newer than the newest");
        assert_eq!(newest(&list, Channel::Beta, &v("1.9.0")).unwrap().tag, "v1.10.0-beta.1");
        assert!(newest(&list, Channel::Beta, &v("2.0.0")).is_none());
    }

    #[test]
    fn the_right_file_is_chosen_for_how_the_program_was_installed() {
        let rel = &parse_feed(FEED).unwrap()[1];
        if cfg!(target_arch = "aarch64") {
            return; // the file names below are for 64-bit Intel/AMD
        }
        let (a, k) = pick_asset(rel, Install::WindowsInstalled).unwrap();
        assert_eq!((a.name.as_str(), k), ("SwiftDownloadManager-1.9.0-setup-x64.exe", Kind::WindowsInstaller));
        let (a, k) = pick_asset(rel, Install::WindowsPortable).unwrap();
        assert_eq!((a.name.as_str(), k), ("SwiftDownloadManager.exe", Kind::WindowsPortable));
        assert_eq!(pick_asset(rel, Install::LinuxDeb).unwrap().1, Kind::Deb);
        assert_eq!(pick_asset(rel, Install::LinuxArch).unwrap().1, Kind::ArchPackage);
        assert!(pick_asset(rel, Install::Other).is_none());
        // An installed copy of a release that has no setup program is still updated, with the single program.
        let mut no_setup = rel.clone();
        no_setup.assets.retain(|a| !a.name.contains("setup"));
        assert_eq!(pick_asset(&no_setup, Install::WindowsInstalled).unwrap().1, Kind::WindowsPortable);
    }

    #[test]
    fn only_safe_addresses_are_used() {
        assert!(check_address("https://example.org/feed.json").is_ok());
        assert!(check_address("http://127.0.0.1:8080/feed.json").is_ok());
        assert!(check_address("http://localhost/feed.json").is_ok());
        assert!(check_address("http://example.org/feed.json").is_err(), "plain http over the internet");
        assert!(check_address("file:///etc/passwd").is_err());
        assert!(check_address("ftp://example.org/x").is_err());
        assert!(check_address("not a url").is_err());
        assert_eq!(Server::GitHub.feed_url("ignored"), format!("https://api.github.com/repos/{REPO}/releases?per_page=100"));
        assert_eq!(Server::Custom.feed_url("  https://mirror.example/feed.json "), "https://mirror.example/feed.json");
    }

    /// A one-file web server: answers every request with `body`.
    fn serve(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in listener.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 2 {
                    line.clear();
                }
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/file")
    }

    fn sha(data: &[u8]) -> String {
        Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sdm-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_feed_is_fetched_and_a_file_is_downloaded_and_checked() {
        let feed = serve(FEED.as_bytes().to_vec());
        assert_eq!(fetch_feed(&feed).unwrap().len(), 3);

        let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let url = serve(data.clone());
        let dir = scratch("ok");
        let good = Asset { name: "setup.exe".into(), size: data.len() as u64, url: url.clone(), sha256: Some(sha(&data)) };
        let progress = Progress::default();
        let path = download(&good, &dir, &progress).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), data);
        assert_eq!(progress.done.load(Atomic::SeqCst), data.len() as u64);
        assert!(!dir.join("setup.exe.part").exists());

        // A wrong checksum: nothing is kept.
        let dir = scratch("bad");
        let bad = Asset { sha256: Some(sha(b"something else")), ..good.clone() };
        let e = download(&bad, &dir, &Progress::default()).unwrap_err();
        assert!(e.contains("checksum"), "{e}");
        assert!(!dir.join("setup.exe").exists() && !dir.join("setup.exe.part").exists(), "the bad file is gone");

        // No checksum to compare with: refused before anything is fetched.
        let e = download(&Asset { sha256: None, ..good.clone() }, &scratch("none"), &Progress::default()).unwrap_err();
        assert!(e.contains("no checksum"), "{e}");

        // A name that tries to leave the folder is cut down to its last part.
        let dir = scratch("name");
        let sneaky = Asset { name: "../../evil.exe".into(), ..good.clone() };
        let path = download(&sneaky, &dir, &Progress::default()).unwrap();
        assert_eq!(path, dir.join("evil.exe"));

        // Cancelling stops it and leaves nothing behind.
        let dir = scratch("cancel");
        let cancelled = Progress::default();
        cancelled.cancel.store(true, Atomic::SeqCst);
        assert_eq!(download(&good, &dir, &cancelled).unwrap_err(), "Cancelled");
        assert!(!dir.join("setup.exe.part").exists());
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("sdm-update-ok-{}", std::process::id())));
    }

    #[test]
    fn the_running_program_is_replaced_and_put_back_when_that_fails() {
        let dir = scratch("swap");
        std::fs::create_dir_all(&dir).unwrap();
        let (me, new) = (dir.join("sdm.exe"), dir.join("download.exe"));
        std::fs::write(&me, b"old version").unwrap();
        std::fs::write(&new, b"new version").unwrap();
        replace_running_program(&me, &new).unwrap();
        assert_eq!(std::fs::read(&me).unwrap(), b"new version");
        assert_eq!(std::fs::read(old_name(&me)).unwrap(), b"old version", "the old one is kept aside until the next start");
        assert!(!dir.join("sdm.exe.new").exists());
        // The new file is missing: the program is left exactly as it was.
        std::fs::write(&me, b"still the old one").unwrap();
        assert!(replace_running_program(&me, &dir.join("missing.exe")).is_err());
        assert_eq!(std::fs::read(&me).unwrap(), b"still the old one");
        let _ = std::fs::remove_dir_all(dir);
    }
}
