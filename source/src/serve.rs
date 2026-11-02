//! `swift_download_manager --server`: a download server for a team or an organisation.
//!
//! There is no window. Several people sign in to the web interface with their own accounts, add links,
//! watch them download on the server and fetch the finished files from their browser. Everybody sees only
//! their own downloads (administrators see all). Run it behind an HTTPS reverse proxy when it leaves a
//! trusted network. The Docker image runs exactly this.

use crate::mcp::server::{Backend, Engine};
use crate::web::auth::Users;
use crate::web::server::{Config, Mode};
use std::io::{BufRead, IsTerminal};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum Task {
    Serve,
    AddUser { name: String, admin: bool },
    RemoveUser { name: String },
    ListUsers,
}

#[derive(Debug, PartialEq)]
pub struct Options {
    pub task: Task,
    pub bind: IpAddr,
    pub port: u16,
    /// Settings, the download list and the user accounts.
    pub data: Option<PathBuf>,
    /// Where finished downloads go (a folder per person inside it).
    pub downloads: Option<PathBuf>,
    pub secure_cookie: bool,
    /// Let people have the server fetch from private and local addresses (off by default).
    pub allow_private: bool,
}

/// Read the command line; `env` gives the environment variables (a parameter so it can be tested).
pub fn parse(args: &[String], env: &dyn Fn(&str) -> Option<String>) -> Result<Options, String> {
    let value = |name: &str| -> Result<Option<String>, String> {
        match args.iter().position(|a| a == name) {
            Some(i) => args.get(i + 1).filter(|v| !v.starts_with("--")).cloned().map(Some).ok_or_else(|| format!("{name} needs a value")),
            None => Ok(None),
        }
    };
    let flag = |name: &str, var: &str| args.iter().any(|a| a == name) || env(var).is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
    let bind = value("--bind")?.or_else(|| env("SDM_BIND")).unwrap_or_else(|| "0.0.0.0".into());
    let bind: IpAddr = bind.parse().map_err(|_| format!("\"{bind}\" is not an address (for example 0.0.0.0 or 127.0.0.1)"))?;
    let port = value("--port")?.or_else(|| env("SDM_PORT")).unwrap_or_else(|| "8080".into());
    let port: u16 = port.parse().map_err(|_| format!("\"{port}\" is not a port number"))?;
    let task = if let Some(name) = value("--add-user")? {
        Task::AddUser { name, admin: args.iter().any(|a| a == "--admin") }
    } else if let Some(name) = value("--remove-user")? {
        Task::RemoveUser { name }
    } else if args.iter().any(|a| a == "--list-users") {
        Task::ListUsers
    } else {
        Task::Serve
    };
    Ok(Options {
        task,
        bind,
        port,
        data: value("--data")?.or_else(|| env("SDM_DATA")).map(PathBuf::from),
        downloads: value("--downloads")?.or_else(|| env("SDM_DOWNLOADS")).map(PathBuf::from),
        secure_cookie: flag("--secure-cookie", "SDM_SECURE_COOKIE"),
        allow_private: flag("--allow-private", "SDM_ALLOW_PRIVATE"),
    })
}

fn read_password() -> Result<String, String> {
    if let Ok(p) = std::env::var("SDM_PASSWORD") {
        return Ok(p);
    }
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprint!("Password: ");
    }
    let mut line = String::new();
    stdin.lock().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

/// Create the first administrator from the environment when there are no accounts yet (for a container's first start).
fn bootstrap_admin(users: &mut Users, env: &dyn Fn(&str) -> Option<String>) -> Result<bool, String> {
    if !users.is_empty() {
        return Ok(false);
    }
    match (env("SDM_ADMIN_USER"), env("SDM_ADMIN_PASSWORD")) {
        (Some(name), Some(password)) => users.set(&name, &password, true).map(|_| true),
        _ => Ok(false),
    }
}

pub fn run(args: &[String]) -> i32 {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let opts = match parse(args, &env) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("swift-download-manager: {e}");
            return 2;
        }
    };
    let data = opts.data.clone().unwrap_or_else(crate::app::default_config_dir);
    let mut users = Users::load(&data);
    match &opts.task {
        Task::ListUsers => {
            for (name, admin) in users.names() {
                println!("{name}{}", if admin { "  (administrator)" } else { "" });
            }
            return 0;
        }
        Task::RemoveUser { name } => {
            return match users.remove(name) {
                Ok(true) => {
                    println!("Removed {name}. Their downloads stay on disk.");
                    0
                }
                Ok(false) => {
                    eprintln!("There is no user {name}.");
                    1
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            };
        }
        Task::AddUser { name, admin } => {
            let result = read_password().and_then(|p| users.set(name, &p, *admin));
            return match result {
                Ok(()) => {
                    println!("Saved {name}{}.", if *admin { " as an administrator" } else { "" });
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            };
        }
        Task::Serve => {}
    }

    match bootstrap_admin(&mut users, &env) {
        Ok(true) => println!("Created the first administrator from SDM_ADMIN_USER."),
        Ok(false) => {}
        Err(e) => {
            eprintln!("swift-download-manager: {e}");
            return 2;
        }
    }
    if users.is_empty() {
        eprintln!(
            "swift-download-manager: there are no user accounts yet, so nobody could sign in.\n\
             Create one first:  swift_download_manager --server --add-user NAME --admin   (the password is read from SDM_PASSWORD or the keyboard)\n\
             or set SDM_ADMIN_USER and SDM_ADMIN_PASSWORD for the first start."
        );
        return 2;
    }

    let cfg = Config { bind: opts.bind, port: opts.port, mode: Mode::Shared, dir: data.clone(), secure_cookie: opts.secure_cookie };
    let mut engine = Engine::for_server(data.clone(), cfg);
    if let Err(e) = engine.ensure_local(Duration::from_secs(5)) {
        eprintln!("swift-download-manager: {e}");
        return 1;
    }
    let downloads = opts.downloads.clone().unwrap_or_else(|| data.join("downloads"));
    if let Some(app) = engine.app_mut() {
        app.configure_server(downloads.clone(), !opts.allow_private);
    }
    println!("Swift Download Manager server: http://{}:{}/  (downloads go to {})", opts.bind, opts.port, downloads.display());
    if !opts.allow_private {
        println!("Links to private and local addresses are refused (use --allow-private to change that).");
    }

    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    if let Err(e) = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)) {
        eprintln!("swift-download-manager: could not watch for Ctrl+C: {e}");
    }
    while !stop.load(Ordering::SeqCst) {
        engine.idle();
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("Stopping: pausing the downloads and saving.");
    engine.shutdown();
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn defaults_listen_everywhere_on_8080() {
        let o = parse(&args(&["--server"]), &no_env).unwrap();
        assert_eq!(o.task, Task::Serve);
        assert_eq!((o.bind.to_string().as_str(), o.port), ("0.0.0.0", 8080));
        assert!(o.data.is_none() && o.downloads.is_none() && !o.secure_cookie && !o.allow_private);
    }

    #[test]
    fn options_and_environment() {
        let o = parse(&args(&["--server", "--bind", "127.0.0.1", "--port", "9000", "--data", "/d", "--secure-cookie"]), &no_env).unwrap();
        assert_eq!((o.bind.to_string().as_str(), o.port, o.secure_cookie), ("127.0.0.1", 9000, true));
        assert_eq!(o.data, Some(PathBuf::from("/d")));
        let env = |k: &str| match k {
            "SDM_PORT" => Some("8181".to_string()),
            "SDM_DOWNLOADS" => Some("/files".to_string()),
            "SDM_ALLOW_PRIVATE" => Some("true".to_string()),
            _ => None,
        };
        let o = parse(&args(&["--server"]), &env).unwrap();
        assert_eq!((o.port, o.allow_private), (8181, true));
        assert_eq!(o.downloads, Some(PathBuf::from("/files")));
        let o = parse(&args(&["--server", "--port", "7000"]), &env).unwrap();
        assert_eq!(o.port, 7000, "the command line wins over the environment");
    }

    #[test]
    fn mistakes_are_explained() {
        assert!(parse(&args(&["--server", "--port", "http"]), &no_env).unwrap_err().contains("not a port"));
        assert!(parse(&args(&["--server", "--bind", "everywhere"]), &no_env).unwrap_err().contains("not an address"));
        assert!(parse(&args(&["--server", "--data"]), &no_env).unwrap_err().contains("needs a value"));
        assert!(parse(&args(&["--server", "--data", "--port", "1"]), &no_env).unwrap_err().contains("needs a value"));
    }

    #[test]
    fn user_tasks() {
        assert_eq!(parse(&args(&["--server", "--add-user", "anna", "--admin"]), &no_env).unwrap().task, Task::AddUser { name: "anna".into(), admin: true });
        assert_eq!(parse(&args(&["--add-user", "ben"]), &no_env).unwrap().task, Task::AddUser { name: "ben".into(), admin: false });
        assert_eq!(parse(&args(&["--remove-user", "ben"]), &no_env).unwrap().task, Task::RemoveUser { name: "ben".into() });
        assert_eq!(parse(&args(&["--list-users"]), &no_env).unwrap().task, Task::ListUsers);
    }

    #[test]
    fn the_first_administrator_comes_from_the_environment() {
        let dir = std::env::temp_dir().join(format!("sdm-serve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut users = Users::load(&dir);
        let none = |_: &str| None;
        assert_eq!(bootstrap_admin(&mut users, &none), Ok(false));
        assert!(users.is_empty());
        let env = |k: &str| match k {
            "SDM_ADMIN_USER" => Some("root".to_string()),
            "SDM_ADMIN_PASSWORD" => Some("a long enough password".to_string()),
            _ => None,
        };
        assert_eq!(bootstrap_admin(&mut users, &env), Ok(true));
        assert!(users.is_admin("root"));
        assert_eq!(bootstrap_admin(&mut users, &env), Ok(false), "only when there are no accounts yet");
        let weak = |k: &str| match k {
            "SDM_ADMIN_USER" => Some("x".to_string()),
            "SDM_ADMIN_PASSWORD" => Some("short".to_string()),
            _ => None,
        };
        let mut empty = Users::load(&dir.join("other"));
        assert!(bootstrap_admin(&mut empty, &weak).is_err());
    }
}
