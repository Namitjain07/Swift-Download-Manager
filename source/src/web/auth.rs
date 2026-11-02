//! Who may use the web interface: user accounts for a shared server, sessions, and a brake on
//! guessing passwords.
//!
//! Passwords are stored as PBKDF2-HMAC-SHA256 hashes with a random salt (never the password), in
//! `users.json` next to the download list, readable only by the owner of the folder.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const ITERATIONS: u32 = 120_000;
pub const MIN_PASSWORD: usize = 8;
pub const SESSION_LIFE: Duration = Duration::from_secs(7 * 24 * 3600);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the system has no random numbers");
    hex(&buf)
}

/// PBKDF2 with HMAC-SHA256, one 32-byte block.
pub fn pbkdf2(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    // HMAC key: hashed when longer than a block, then padded.
    let mut key = [0u8; 64];
    if password.len() > 64 {
        key[..32].copy_from_slice(&Sha256::digest(password));
    } else {
        key[..password.len()].copy_from_slice(password);
    }
    let ipad: Vec<u8> = key.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = key.iter().map(|b| b ^ 0x5c).collect();
    let inner0 = Sha256::new().chain_update(&ipad);
    let outer0 = Sha256::new().chain_update(&opad);
    let mac = |data: &[u8]| -> [u8; 32] {
        let inner = inner0.clone().chain_update(data).finalize();
        outer0.clone().chain_update(inner).finalize().into()
    };
    let mut first = salt.to_vec();
    first.extend_from_slice(&1u32.to_be_bytes());
    let mut u = mac(&first);
    let mut out = u;
    for _ in 1..iterations.max(1) {
        u = mac(&u);
        for (o, x) in out.iter_mut().zip(u) {
            *o ^= x;
        }
    }
    out
}

/// Compare two secrets without stopping at the first difference.
pub fn same_secret(a: &str, b: &str) -> bool {
    same(a.as_bytes(), b.as_bytes())
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserRecord {
    pub name: String,
    pub salt: String,
    pub hash: String,
    pub iterations: u32,
    #[serde(default)]
    pub admin: bool,
}

/// User names: letters, digits, dot, dash and underscore, so a name is also a safe folder name.
pub fn valid_username(name: &str) -> bool {
    (1..=32).contains(&name.len()) && !name.starts_with('.') && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

pub struct Users {
    path: PathBuf,
    list: Vec<UserRecord>,
}

impl Users {
    pub fn load(dir: &Path) -> Users {
        let path = dir.join("users.json");
        let list = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Users { path, list }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn names(&self) -> Vec<(String, bool)> {
        self.list.iter().map(|u| (u.name.clone(), u.admin)).collect()
    }

    #[cfg(test)]
    pub fn is_admin(&self, name: &str) -> bool {
        self.list.iter().any(|u| u.name == name && u.admin)
    }

    fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        use std::io::Write;
        opts.open(&tmp)?.write_all(serde_json::to_string_pretty(&self.list).unwrap_or_default().as_bytes())?;
        std::fs::rename(tmp, &self.path)
    }

    /// Add a user, or change the password of an existing one.
    pub fn set(&mut self, name: &str, password: &str, admin: bool) -> Result<(), String> {
        if !valid_username(name) {
            return Err("A user name has 1 to 32 letters, digits, dots, dashes or underscores".into());
        }
        if password.chars().count() < MIN_PASSWORD {
            return Err(format!("The password needs at least {MIN_PASSWORD} characters"));
        }
        let salt = random_hex(16);
        let hash = hex(&pbkdf2(password.as_bytes(), salt.as_bytes(), ITERATIONS));
        let record = UserRecord { name: name.to_string(), salt, hash, iterations: ITERATIONS, admin };
        match self.list.iter_mut().find(|u| u.name == name) {
            Some(existing) => *existing = record,
            None => self.list.push(record),
        }
        self.save().map_err(|e| format!("Could not save the users: {e}"))
    }

    pub fn remove(&mut self, name: &str) -> Result<bool, String> {
        let before = self.list.len();
        self.list.retain(|u| u.name != name);
        let removed = self.list.len() != before;
        if removed {
            self.save().map_err(|e| format!("Could not save the users: {e}"))?;
        }
        Ok(removed)
    }

    /// Check a name and password. An unknown name costs as much time as a wrong password.
    pub fn verify(&self, name: &str, password: &str) -> Option<&UserRecord> {
        let found = self.list.iter().find(|u| u.name == name);
        let (salt, expected, iterations) = match found {
            Some(u) => (u.salt.as_str(), unhex(&u.hash).unwrap_or_default(), u.iterations),
            None => ("0000000000000000", vec![0u8; 32], ITERATIONS),
        };
        let given = pbkdf2(password.as_bytes(), salt.as_bytes(), iterations);
        let ok = same(&given, &expected);
        found.filter(|_| ok)
    }
}

// ---------------------------------------------------------------- sessions

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub user: String,
    pub admin: bool,
    expires: Instant,
}

#[derive(Default)]
pub struct Sessions {
    map: Mutex<HashMap<String, Session>>,
}

impl Sessions {
    pub fn create(&self, user: &str, admin: bool) -> String {
        self.create_at(user, admin, Instant::now())
    }

    fn create_at(&self, user: &str, admin: bool, now: Instant) -> String {
        let token = random_hex(32);
        let mut map = self.map.lock().unwrap();
        map.retain(|_, s| s.expires > now);
        map.insert(token.clone(), Session { user: user.to_string(), admin, expires: now + SESSION_LIFE });
        token
    }

    pub fn lookup(&self, token: &str) -> Option<Session> {
        self.lookup_at(token, Instant::now())
    }

    fn lookup_at(&self, token: &str, now: Instant) -> Option<Session> {
        self.map.lock().unwrap().get(token).filter(|s| s.expires > now).cloned()
    }

    pub fn destroy(&self, token: &str) {
        self.map.lock().unwrap().remove(token);
    }
}

// ---------------------------------------------------------------- guessing

const MAX_FAILS: u32 = 5;
const WINDOW: Duration = Duration::from_secs(300);

/// Slows down anyone who keeps getting the password wrong: five failures from one address block
/// that address for the rest of five minutes.
#[derive(Default)]
pub struct Throttle {
    fails: Mutex<HashMap<IpAddr, (u32, Instant)>>,
}

impl Throttle {
    pub fn blocked(&self, ip: IpAddr) -> bool {
        self.blocked_at(ip, Instant::now())
    }

    fn blocked_at(&self, ip: IpAddr, now: Instant) -> bool {
        self.fails.lock().unwrap().get(&ip).is_some_and(|(n, first)| *n >= MAX_FAILS && now.duration_since(*first) < WINDOW)
    }

    pub fn failed(&self, ip: IpAddr) {
        self.failed_at(ip, Instant::now());
    }

    fn failed_at(&self, ip: IpAddr, now: Instant) {
        let mut map = self.fails.lock().unwrap();
        map.retain(|_, (_, first)| now.duration_since(*first) < WINDOW);
        let entry = map.entry(ip).or_insert((0, now));
        entry.0 += 1;
    }

    pub fn succeeded(&self, ip: IpAddr) {
        self.fails.lock().unwrap().remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sdm-auth-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pbkdf2_matches_the_reference_values() {
        // Computed with Python's hashlib.pbkdf2_hmac('sha256', ...).
        assert_eq!(hex(&pbkdf2(b"password", b"salt", 1)), "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b");
        assert_eq!(hex(&pbkdf2(b"password", b"salt", 2)), "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43");
        assert_eq!(hex(&pbkdf2(b"Password", b"NaCl", 80000)), "4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab56");
    }

    #[test]
    fn long_passwords_work_too() {
        let long = vec![b'x'; 200];
        assert_ne!(pbkdf2(&long, b"salt", 3), pbkdf2(&long[..199], b"salt", 3));
    }

    #[test]
    fn user_names_are_safe_folder_names() {
        for ok in ["anna", "A.b-c_9", "x"] {
            assert!(valid_username(ok), "{ok}");
        }
        for bad in ["", ".hidden", "a/b", "..", "a b", "ünï", &"x".repeat(33), "a\\b"] {
            assert!(!valid_username(bad), "{bad}");
        }
    }

    #[test]
    fn users_are_saved_and_checked() {
        let dir = scratch("users");
        let mut users = Users::load(&dir);
        assert!(users.is_empty());
        users.set("anna", "correct horse", true).unwrap();
        users.set("ben", "battery staple", false).unwrap();
        assert!(users.set("x", "short", false).is_err());
        assert!(users.set("../evil", "long enough pw", false).is_err());

        let again = Users::load(&dir);
        assert_eq!(again.names(), [("anna".to_string(), true), ("ben".to_string(), false)]);
        assert!(again.verify("anna", "correct horse").is_some_and(|u| u.admin));
        assert!(again.verify("anna", "wrong horse").is_none());
        assert!(again.verify("nobody", "correct horse").is_none());
        let raw = std::fs::read_to_string(dir.join("users.json")).unwrap();
        assert!(!raw.contains("correct horse"), "the password must not be stored");

        let mut again = again;
        again.set("ben", "a brand new one", false).unwrap();
        assert!(again.verify("ben", "battery staple").is_none());
        assert!(again.verify("ben", "a brand new one").is_some());
        assert_eq!(again.names().len(), 2, "changing a password does not add a user");
        assert!(again.remove("ben").unwrap());
        assert!(!again.remove("ben").unwrap());
        assert!(Users::load(&dir).verify("ben", "a brand new one").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn the_users_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("perm");
        Users::load(&dir).set("anna", "correct horse", true).unwrap();
        assert_eq!(std::fs::metadata(dir.join("users.json")).unwrap().permissions().mode() & 0o077, 0);
    }

    #[test]
    fn sessions_expire_and_can_be_ended() {
        let s = Sessions::default();
        let t0 = Instant::now();
        let token = s.create_at("anna", true, t0);
        assert_eq!(token.len(), 64);
        assert_eq!(s.lookup_at(&token, t0).map(|x| (x.user, x.admin)), Some(("anna".into(), true)));
        assert!(s.lookup_at(&token, t0 + SESSION_LIFE + Duration::from_secs(1)).is_none());
        assert!(s.lookup("nonsense").is_none());
        let other = s.create("ben", false);
        s.destroy(&other);
        assert!(s.lookup(&other).is_none());
    }

    #[test]
    fn five_wrong_passwords_block_an_address_for_a_while() {
        let t = Throttle::default();
        let ip: IpAddr = "10.0.0.7".parse().unwrap();
        let other: IpAddr = "10.0.0.8".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..4 {
            t.failed_at(ip, t0);
        }
        assert!(!t.blocked_at(ip, t0));
        t.failed_at(ip, t0);
        assert!(t.blocked_at(ip, t0));
        assert!(!t.blocked_at(other, t0));
        assert!(!t.blocked_at(ip, t0 + WINDOW + Duration::from_secs(1)));
        t.succeeded(ip);
        assert!(!t.blocked_at(ip, t0));
    }
}
