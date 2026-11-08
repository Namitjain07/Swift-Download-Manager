//! What an AI assistant is allowed to do, and the limits it works within.
//!
//! The person decides in Options > AI assistants; the app enforces it on every call, so a client that ignores
//! the hints in a tool's description cannot get around it. Everything that comes from an assistant is treated as
//! untrusted: it may have been talked into it by a web page or a document it read (prompt injection).

use super::error::{ErrorKind, ToolError};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How much an assistant may do. Each level includes the one before.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Look at downloads, queues and settings; check links and files. Change nothing.
    ReadOnly,
    /// Also add, pause, resume, move and remove downloads, and run queues.
    #[default]
    Standard,
    /// Also the overall speed limit, the list of networks and clearing finished downloads in one go.
    Full,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::ReadOnly => "read-only",
            Level::Standard => "standard",
            Level::Full => "full",
        }
    }

    /// For `--level` on the command line.
    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_lowercase().as_str() {
            "read" | "read-only" | "read_only" | "readonly" => Some(Level::ReadOnly),
            "standard" | "default" => Some(Level::Standard),
            "full" => Some(Level::Full),
            _ => None,
        }
    }
}

/// The rules for links and the limits on adding, as the app has them set. The `--mcp` process asks for a copy
/// (`GetPolicy`) so that it can check a link itself before touching the network.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Policy {
    pub level: Level,
    /// Only these sites (and their subdomains); empty means any public site.
    pub allow_hosts: Vec<String>,
    /// Never these sites (and their subdomains).
    pub block_hosts: Vec<String>,
    /// Refuse addresses on private networks and this computer itself.
    pub block_private: bool,
    /// What an assistant adds waits in its queue until the person starts it.
    pub hold: bool,
    /// The most downloads an assistant may add in an hour; 0 means no limit.
    pub adds_per_hour: u32,
}

impl Default for Policy {
    fn default() -> Self {
        Policy { level: Level::Standard, allow_hosts: Vec::new(), block_hosts: Vec::new(), block_private: true, hold: false, adds_per_hour: 60 }
    }
}

/// A site as people write it (`example.com`, `https://Example.com/path`, `*.example.com`) in the form that is compared.
pub fn normalize_site(entry: &str) -> String {
    let e = entry.trim().to_ascii_lowercase();
    let e = e.strip_prefix("https://").or_else(|| e.strip_prefix("http://")).unwrap_or(&e);
    let e = e.split(['/', '?', '#']).next().unwrap_or("");
    e.trim_start_matches("*.").trim_start_matches('.').trim_end_matches('.').to_string()
}

/// The sites in a text box, one per line (commas and spaces also separate), cleaned and without repeats.
pub fn parse_sites(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split(['\n', ',', ' ', '\t', ';']) {
        let site = normalize_site(part);
        if !site.is_empty() && !out.contains(&site) {
            out.push(site);
        }
    }
    out
}

fn host_matches(host: &str, site: &str) -> bool {
    let site = normalize_site(site);
    !site.is_empty() && (host == site || host.ends_with(&format!(".{site}")))
}

/// Is this a name or address that points inside a network, or at this computer, as far as the text alone shows?
/// (The connection itself is checked again when it is made, which also covers redirects and names that change their answer.)
fn private_host(host: &url::Host<&str>) -> bool {
    match host {
        url::Host::Ipv4(ip) => crate::netroute::is_private(std::net::IpAddr::V4(*ip)),
        url::Host::Ipv6(ip) => crate::netroute::is_private(std::net::IpAddr::V6(*ip)),
        url::Host::Domain(name) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            // A name with no dot only resolves through a local search domain; the others are for local use by convention.
            !name.contains('.') || ["localhost", "local", "internal", "intranet", "lan", "home", "corp", "home.arpa"].iter().any(|s| name == *s || name.ends_with(&format!(".{s}")))
        }
    }
}

impl Policy {
    /// May an assistant have this link downloaded (or look at it)? Says why not, in words that tell it what to do.
    pub fn check_link(&self, link: &str) -> Result<(), ToolError> {
        let parsed = url::Url::parse(link.trim()).map_err(|_| ToolError::invalid(format!("\"{}\" doesn't look like a web address.", untrusted(link, 120))))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(ToolError::invalid("Only http:// and https:// links are supported."));
        }
        let Some(host) = parsed.host() else {
            return Err(ToolError::invalid("The link has no host name."));
        };
        let name = parsed.host_str().unwrap_or("").trim_end_matches('.').to_ascii_lowercase();
        if self.block_hosts.iter().any(|s| host_matches(&name, s)) {
            return Err(ToolError::forbidden(format!("Downloads from {name} are blocked in Options > AI assistants.")).hint("Do not try to get around this; ask the person."));
        }
        if !self.allow_hosts.is_empty() && !self.allow_hosts.iter().any(|s| host_matches(&name, s)) {
            return Err(ToolError::forbidden(format!("Assistants may only download from: {}.", self.allow_hosts.join(", "))).hint("Ask the person to add the site in Options > AI assistants."));
        }
        if self.block_private && private_host(&host) {
            return Err(ToolError::forbidden(format!("{name} is on a private network or on this computer, which assistants may not reach.")).hint("The person can allow this in Options > AI assistants."));
        }
        Ok(())
    }
}

/// How many things happened in the last `span`: for limits such as "60 downloads an hour".
#[derive(Default, Debug)]
pub struct Window {
    times: VecDeque<Instant>,
}

impl Window {
    /// Records one more thing, unless `max` have already happened within `span`; then says how long to wait.
    pub fn allow(&mut self, now: Instant, span: Duration, max: usize) -> Result<(), Duration> {
        while self.times.front().is_some_and(|t| now.duration_since(*t) >= span) {
            self.times.pop_front();
        }
        if self.times.len() >= max {
            return Err(match self.times.front() {
                Some(oldest) => span.saturating_sub(now.duration_since(*oldest)),
                None => span,
            });
        }
        self.times.push_back(now);
        Ok(())
    }

    /// How long to wait before one more may happen, or `None` when one may happen now. Records nothing.
    pub fn wait_needed(&self, now: Instant, span: Duration, max: usize) -> Option<Duration> {
        let recent: Vec<&Instant> = self.times.iter().filter(|t| now.duration_since(**t) < span).collect();
        if recent.len() < max {
            return None;
        }
        Some(recent.first().map_or(span, |oldest| span.saturating_sub(now.duration_since(**oldest))))
    }
}

pub fn rate_limited(what: &str, wait: Duration) -> ToolError {
    let secs = wait.as_secs().max(1);
    let when = if secs >= 120 { format!("{} minutes", secs.div_ceil(60)) } else { format!("{secs} seconds") };
    ToolError::new(ErrorKind::RateLimited, format!("Too many {what} in a short time.")).hint(format!("Wait {when} and try again, or ask the person to raise the limit in Options > AI assistants."))
}

/// Text from outside (a file name, an error from a server, a description) made safe to show to an assistant and to the
/// person: control characters and the invisible characters that reorder or hide text are dropped, runs of white space
/// become one space, and it is cut at `max` characters. It is still not to be trusted as an instruction.
pub fn untrusted(text: &str, max: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max + 4));
    let mut space = false;
    let mut count = 0;
    for c in text.chars() {
        let hidden = matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206F}' | '\u{FEFF}');
        if hidden || (c.is_control() && !c.is_whitespace()) {
            continue;
        }
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            count += 1;
            space = false;
        }
        if count >= max {
            out.push('…');
            return out;
        }
        out.push(c);
        count += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        Policy::default()
    }

    #[test]
    fn levels_are_ordered_and_parse() {
        assert!(Level::ReadOnly < Level::Standard && Level::Standard < Level::Full);
        assert_eq!(Level::parse("READ"), Some(Level::ReadOnly));
        assert_eq!(Level::parse("read-only"), Some(Level::ReadOnly));
        assert_eq!(Level::parse(" full "), Some(Level::Full));
        assert_eq!(Level::parse("root"), None);
        assert_eq!(Level::default(), Level::Standard);
    }

    #[test]
    fn sites_are_normalised() {
        assert_eq!(normalize_site("https://Example.COM/some/path?x=1"), "example.com");
        assert_eq!(normalize_site("*.example.com"), "example.com");
        assert_eq!(normalize_site(" .example.com. "), "example.com");
        assert_eq!(parse_sites("a.com\nb.com, A.com  https://c.org/x"), vec!["a.com", "b.com", "c.org"]);
        assert!(parse_sites("  \n ").is_empty());
    }

    #[test]
    fn private_and_local_links_are_refused_by_default() {
        let p = policy();
        for link in [
            "http://127.0.0.1/a",
            "http://localhost:9615/",
            "http://LOCALHOST./x",
            "http://192.168.1.1/admin",
            "http://10.0.0.5/",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/",
            "http://[fd00::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://2130706433/",
            "http://0x7f.1/",
            "http://router/",
            "http://printer.local/",
            "http://db.internal/dump",
            "http://100.64.0.1/",
        ] {
            let e = p.check_link(link).expect_err(link);
            assert_eq!(e.kind, ErrorKind::Forbidden, "{link}: {e}");
        }
        for link in ["https://example.com/a.zip", "http://93.184.216.34/a", "https://sub.example.org:8443/x?y=1"] {
            assert!(p.check_link(link).is_ok(), "{link}");
        }
        let open = Policy { block_private: false, ..policy() };
        assert!(open.check_link("http://127.0.0.1/a").is_ok());
    }

    #[test]
    fn bad_links_are_invalid_arguments() {
        for link in ["", "not a link", "ftp://example.com/a", "file:///etc/passwd", "javascript:alert(1)", "data:text/plain,hi"] {
            assert_eq!(policy().check_link(link).expect_err(link).kind, ErrorKind::InvalidArgument, "{link}");
        }
    }

    #[test]
    fn site_lists_apply_to_subdomains_not_to_lookalikes() {
        let p = Policy { allow_hosts: vec!["example.com".into(), "*.cdn.test".into()], block_hosts: vec!["bad.example.com".into()], ..policy() };
        assert!(p.check_link("https://example.com/a").is_ok());
        assert!(p.check_link("https://files.example.com/a").is_ok());
        assert!(p.check_link("https://x.cdn.test/a").is_ok());
        assert_eq!(p.check_link("https://notexample.com/a").unwrap_err().kind, ErrorKind::Forbidden);
        assert_eq!(p.check_link("https://example.com.evil.net/a").unwrap_err().kind, ErrorKind::Forbidden);
        assert_eq!(p.check_link("https://bad.example.com/a").unwrap_err().kind, ErrorKind::Forbidden);
        assert_eq!(p.check_link("https://deep.bad.example.com/a").unwrap_err().kind, ErrorKind::Forbidden);
        let e = p.check_link("https://other.org/a").unwrap_err();
        assert!(e.message.contains("example.com") && e.hint.is_some());
    }

    #[test]
    fn window_limits_and_forgets() {
        let mut w = Window::default();
        let t0 = Instant::now();
        let hour = Duration::from_secs(3600);
        assert!(w.allow(t0, hour, 2).is_ok());
        assert!(w.allow(t0 + Duration::from_secs(10), hour, 2).is_ok());
        let wait = w.allow(t0 + Duration::from_secs(20), hour, 2).unwrap_err();
        assert_eq!(wait, Duration::from_secs(3580));
        assert_eq!(w.wait_needed(t0 + Duration::from_secs(20), hour, 2), Some(Duration::from_secs(3580)));
        assert_eq!(w.wait_needed(t0 + Duration::from_secs(20), hour, 3), None);
        assert!(w.allow(t0 + hour, hour, 2).is_ok(), "the first one is an hour old");
        assert!(w.allow(t0 + hour, hour, 2).is_err());
        // A limit of nothing never lets anything through; callers treat 0 as "no limit" before asking.
        assert!(Window::default().allow(t0, hour, 0).is_err());
    }

    #[test]
    fn rate_limit_message_says_when_to_come_back() {
        let e = rate_limited("downloads", Duration::from_secs(90));
        assert_eq!(e.kind, ErrorKind::RateLimited);
        assert!(e.hint.unwrap().contains("90 seconds"));
        assert!(rate_limited("calls", Duration::from_secs(600)).hint.unwrap().contains("10 minutes"));
    }

    #[test]
    fn untrusted_text_loses_controls_and_tricks() {
        assert_eq!(untrusted("a\nb\r\n\tc", 100), "a b c");
        assert_eq!(untrusted("  spaced   out  ", 100), "spaced out");
        assert_eq!(untrusted("evil\u{202E}txt.exe", 100), "eviltxt.exe", "right-to-left override removed");
        assert_eq!(untrusted("zero\u{200B}width", 100), "zerowidth");
        assert_eq!(untrusted("bell\u{7}\u{0}x", 100), "bellx");
        assert_eq!(untrusted("abcdefghij", 4), "abcd…");
        assert_eq!(untrusted("ünï", 10), "ünï");
        assert_eq!(untrusted("", 5), "");
    }
}
