// Tests of the newer parts of the API: health check, many links at once, bulk buttons, networks and the speed
// limit, access tokens, managing accounts from the page, and metrics. Included at the end of `tests.rs`, so
// it shares that file's helpers (`site`, `send`, `login` ...).

// ---------------------------------------------------------------- health check

#[test]
fn the_health_check_needs_no_sign_in_and_says_nothing_about_the_downloads() {
    for (name, mode) in [("health-shared", Mode::Shared), ("health-personal", Mode::Personal)] {
        let s = site(name, mode, &[("anna", "correct horse", false)]);
        let a = get(&s, "/healthz", "");
        assert_eq!((a.code, a.json()), (200, json!({ "status": "ok" })));
        assert!(s.log.lock().unwrap().is_empty(), "the app was not asked anything");
        assert_eq!(get(&s, "/api/status", "").code, 401, "everything else still needs a sign-in");
    }
}

// ---------------------------------------------------------------- many links at once, and bulk buttons

#[test]
fn a_list_of_links_becomes_one_command_and_blank_or_commented_lines_are_skipped() {
    let s = site("batch", Mode::Personal, &[]);
    let c = personal_session(&s);
    let text = "https://e.com/a.zip\n\n  https://e.com/b.zip  \r\n# a comment\nhttps://e.com/c.zip";
    let a = post(&s, "/api/downloads/batch", &c, &json!({ "text": text, "urls": ["https://e.com/d.zip"], "category": "Programs", "queue": "Main Queue", "start": false }).to_string());
    assert_eq!(a.code, 200);
    let log = s.log.lock().unwrap();
    let (cmd, _) = log.last().unwrap();
    assert_eq!(
        *cmd,
        Command::AddMany {
            urls: ["https://e.com/d.zip", "https://e.com/a.zip", "https://e.com/b.zip", "https://e.com/c.zip"].map(String::from).to_vec(),
            category: Some("Programs".into()),
            queue: Some("Main Queue".into()),
            start: Some(false)
        }
    );
}

#[test]
fn bulk_buttons_become_commands_for_the_person_who_pressed_them() {
    let s = site("bulk", Mode::Shared, &[("anna", "correct horse", false)]);
    let c = login(&s, "anna", "correct horse").cookie().unwrap();
    for path in ["/api/downloads/pause-all", "/api/downloads/resume-all", "/api/downloads/clear-completed"] {
        assert_eq!(post(&s, path, &c, "").code, 200, "{path}");
    }
    let log = s.log.lock().unwrap();
    let seen: Vec<_> = log.iter().map(|(c, w)| (c.clone(), w.clone())).collect();
    let anna = Some(Who { name: "anna".into(), admin: false });
    assert_eq!(seen, vec![(Command::PauseAll, anna.clone()), (Command::ResumeAll, anna.clone()), (Command::ClearCompleted, anna)]);
    drop(log);
    // Like every change, they need the custom header.
    assert_eq!(send(&s, "POST", "/api/downloads/pause-all", &[("Cookie", &c)], "").code, 403);
}

// ---------------------------------------------------------------- networks and the speed limit

#[test]
fn networks_and_the_speed_limit_become_commands_and_bad_input_is_refused_before_the_app_hears_of_it() {
    let s = site("networks", Mode::Personal, &[]);
    let c = personal_session(&s);
    assert_eq!(get(&s, "/api/networks", &c).code, 200);
    assert_eq!(post(&s, "/api/networks", &c, &json!({ "enabled": true, "automatic": false, "chosen": ["Wi-Fi", "Ethernet"], "off": ["Phone"] }).to_string()).code, 200);
    assert_eq!(post(&s, "/api/networks", &c, r#"{"automatic":true}"#).code, 200);
    assert_eq!(post(&s, "/api/speed-limit", &c, r#"{"kbps":2048}"#).code, 200);
    assert_eq!(post(&s, "/api/speed-limit", &c, r#"{"kbps":null}"#).code, 200);
    let cmds: Vec<Command> = s.log.lock().unwrap().iter().map(|(c, _)| c.clone()).collect();
    assert_eq!(
        cmds,
        vec![
            Command::ListNetworks,
            Command::SetNetworks { on: Some(true), auto: Some(false), names: Some(vec!["Wi-Fi".into(), "Ethernet".into()]), off: Some(vec!["Phone".into()]) },
            Command::SetNetworks { on: None, auto: Some(true), names: None, off: None },
            Command::SetSpeedLimit { kbps: Some(2048) },
            Command::SetSpeedLimit { kbps: None },
        ]
    );
    let before = s.log.lock().unwrap().len();
    for body in [r#"{"chosen":"Wi-Fi"}"#, r#"{"off":[1,2]}"#, r#"{"chosen":{"a":1}}"#] {
        assert_eq!(post(&s, "/api/networks", &c, body).code, 400, "{body}");
    }
    for body in [r#"{"kbps":"fast"}"#, r#"{"kbps":-5}"#, r#"{"kbps":1.5}"#] {
        assert_eq!(post(&s, "/api/speed-limit", &c, body).code, 400, "{body}");
    }
    assert_eq!(s.log.lock().unwrap().len(), before, "none of those reached the app");
}

#[test]
fn what_the_app_refuses_an_ordinary_user_comes_back_as_an_error_not_a_success() {
    let s = site_with("net-refused", Mode::Shared, &[("anna", "correct horse", false)], |c| match c {
        Command::ListNetworks | Command::SetNetworks { .. } | Command::SetSpeedLimit { .. } => Err("Only an administrator can do that.".into()),
        _ => Ok(json!({})),
    });
    let c = login(&s, "anna", "correct horse").cookie().unwrap();
    assert_eq!(get(&s, "/api/networks", &c).code, 400);
    let a = post(&s, "/api/speed-limit", &c, r#"{"kbps":1}"#);
    assert_eq!(a.code, 400);
    assert!(a.json()["error"].as_str().unwrap().contains("administrator"));
}

// ---------------------------------------------------------------- access tokens

fn make_token(s: &Site, cookie: &str, label: &str) -> (String, String) {
    let a = post(s, "/api/tokens", cookie, &json!({ "label": label }).to_string());
    assert_eq!(a.code, 200, "{}", String::from_utf8_lossy(&a.body));
    let j = a.json();
    (j["id"].as_str().unwrap().to_string(), j["token"].as_str().unwrap().to_string())
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[test]
fn a_token_lets_a_program_act_as_its_owner_and_nobody_else() {
    let s = site("tokens", Mode::Shared, &[("anna", "correct horse", false), ("root", "battery staple", true)]);
    let anna = login(&s, "anna", "correct horse").cookie().unwrap();
    let (_, token) = make_token(&s, &anna, "laptop add-on");
    assert!(token.starts_with("sdm_"));
    // No cookie, no page: just the header.
    let a = send(&s, "POST", "/api/downloads", &[("Authorization", &bearer(&token)), ("Content-Type", "application/json")], r#"{"url":"https://e.com/a.zip"}"#);
    assert_eq!(a.code, 200, "{}", String::from_utf8_lossy(&a.body));
    assert_eq!(s.log.lock().unwrap().last().unwrap().1, Some(Who { name: "anna".into(), admin: false }), "the app hears who it is");
    // It has Anna's rights and no more.
    let before = s.log.lock().unwrap().len();
    assert_eq!(send(&s, "GET", "/api/users", &[("Authorization", &bearer(&token))], "").code, 403);
    assert_eq!(s.log.lock().unwrap().len(), before);
    // A wrong or made-up token is nobody.
    for bad in ["sdm_".to_string() + &"0".repeat(48), "nonsense".into(), token[..token.len() - 1].to_string()] {
        assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", &bearer(&bad))], "").code, 401, "{bad}");
    }
    // The personal key does not open a shared server.
    assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", &bearer(&s.handle.key))], "").code, 401);
}

#[test]
fn a_token_works_from_a_program_with_an_origin_header_but_a_cookie_from_another_site_does_not() {
    let s = site("tokens-origin", Mode::Shared, &[("anna", "correct horse", false)]);
    let anna = login(&s, "anna", "correct horse").cookie().unwrap();
    let (_, token) = make_token(&s, &anna, "add-on");
    // The browser add-on sends the address of the add-on as Origin; it carries a token, not the visitor's cookies.
    let from_addon = send(&s, "POST", "/api/downloads", &[("Authorization", &bearer(&token)), ("Origin", "chrome-extension://abcdef")], r#"{"url":"https://e.com/a.zip"}"#);
    assert_eq!(from_addon.code, 200);
    // A web page on another site riding on the visitor's cookie is still turned away.
    let riding = send(&s, "POST", "/api/downloads", &[("Cookie", &anna), ("X-SDM-Request", "1"), ("Origin", "https://evil.example")], r#"{"url":"https://e.com/b.zip"}"#);
    assert_eq!(riding.code, 403);
}

#[test]
fn tokens_are_listed_without_their_secret_revoked_at_once_and_only_by_their_owner() {
    let s = site("tokens-manage", Mode::Shared, &[("anna", "correct horse", false), ("ben", "another password", false)]);
    let (anna, ben) = (login(&s, "anna", "correct horse").cookie().unwrap(), login(&s, "ben", "another password").cookie().unwrap());
    let (id, token) = make_token(&s, &anna, "script");
    let list = get(&s, "/api/tokens", &anna);
    assert_eq!(list.code, 200);
    let text = String::from_utf8_lossy(&list.body).to_string();
    assert!(text.contains("script") && text.contains(&id) && !text.contains(&token), "the secret is shown only when it is made: {text}");
    assert_eq!(get(&s, "/api/tokens", &ben).json()["tokens"], json!([]), "Ben sees none of Anna's");
    // Ben cannot revoke it by id.
    assert_eq!(send(&s, "DELETE", &format!("/api/tokens/{id}"), &[("Cookie", &ben), ("X-SDM-Request", "1")], "").code, 404);
    assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", &bearer(&token))], "").code, 200);
    // Anna can; it stops working at once.
    assert_eq!(send(&s, "DELETE", &format!("/api/tokens/{id}"), &[("Cookie", &anna), ("X-SDM-Request", "1")], "").code, 200);
    assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", &bearer(&token))], "").code, 401);
}

#[test]
fn a_token_cannot_make_or_manage_tokens() {
    let s = site("tokens-no-chain", Mode::Shared, &[("anna", "correct horse", false)]);
    let anna = login(&s, "anna", "correct horse").cookie().unwrap();
    let (id, token) = make_token(&s, &anna, "leaky");
    let auth = bearer(&token);
    assert_eq!(send(&s, "POST", "/api/tokens", &[("Authorization", &auth)], r#"{"label":"more"}"#).code, 403);
    assert_eq!(send(&s, "GET", "/api/tokens", &[("Authorization", &auth)], "").code, 403);
    assert_eq!(send(&s, "DELETE", &format!("/api/tokens/{id}"), &[("Authorization", &auth)], "").code, 403);
    // The page is told not to offer tokens to something that came in with one.
    assert_eq!(send(&s, "GET", "/api/me", &[("Authorization", &auth)], "").json()["tokens"], false);
    assert_eq!(get(&s, "/api/me", &anna).json()["tokens"], true);
}

#[test]
fn tokens_have_rules_and_a_personal_page_has_none() {
    let s = site("tokens-rules", Mode::Shared, &[("anna", "correct horse", false)]);
    let anna = login(&s, "anna", "correct horse").cookie().unwrap();
    for bad in ["", "   ", &"x".repeat(41)] {
        assert_eq!(post(&s, "/api/tokens", &anna, &json!({ "label": bad }).to_string()).code, 400, "{bad:?}");
    }
    assert_eq!(post(&s, "/api/tokens", &anna, "{}").code, 400);
    for i in 0..crate::web::auth::MAX_TOKENS {
        make_token(&s, &anna, &format!("t{i}"));
    }
    let e = post(&s, "/api/tokens", &anna, r#"{"label":"eleven"}"#);
    assert_eq!(e.code, 400);
    assert!(e.json()["error"].as_str().unwrap().contains("at most"));
    // Nothing without a sign-in.
    assert_eq!(get(&s, "/api/tokens", "").code, 401);
    let p = site("tokens-personal", Mode::Personal, &[]);
    let c = personal_session(&p);
    assert_eq!(get(&p, "/api/tokens", &c).code, 404);
    assert_eq!(post(&p, "/api/tokens", &c, r#"{"label":"x"}"#).code, 404);
}

#[test]
fn a_token_follows_its_account_it_stops_when_the_account_is_removed_and_rights_are_read_afresh() {
    let s = site("tokens-account", Mode::Shared, &[("anna", "correct horse", false), ("root", "battery staple", true)]);
    let anna = login(&s, "anna", "correct horse").cookie().unwrap();
    let (_, token) = make_token(&s, &anna, "script");
    let auth = bearer(&token);
    assert_eq!(send(&s, "GET", "/api/me", &[("Authorization", &auth)], "").json()["admin"], false);
    // Promoted while the token exists: the token acts with the account's rights as they are now.
    Users::load(&s.dir).set("anna", "correct horse", true).unwrap();
    assert_eq!(send(&s, "GET", "/api/me", &[("Authorization", &auth)], "").json()["admin"], true);
    // Removed: the token is dead, even though it is still in the file.
    Users::load(&s.dir).remove("anna").unwrap();
    assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", &auth)], "").code, 401);
}

// ---------------------------------------------------------------- managing accounts from the page

fn usage_answer(c: &Command) -> Result<Value, crate::mcp::error::ToolError> {
    match c {
        Command::Usage => Ok(json!({ "users": [
            { "name": "anna", "downloads": 3, "active": 1, "complete": 2, "downloaded_bytes": 5000, "speed_bytes_per_sec": 1200.0 },
        ] })),
        _ => Ok(json!({})),
    }
}

#[test]
fn administrators_see_every_account_with_its_usage_and_nobody_else_does() {
    let s = site_with("users-list", Mode::Shared, &[("anna", "correct horse", false), ("root", "battery staple", true), ("ben", "another password", false)], usage_answer);
    let (root, anna) = (login(&s, "root", "battery staple").cookie().unwrap(), login(&s, "anna", "correct horse").cookie().unwrap());
    let (_, _token) = make_token(&s, &anna, "script");
    let a = get(&s, "/api/users", &root);
    assert_eq!(a.code, 200);
    let users = a.json()["users"].as_array().unwrap().clone();
    let by = |n: &str| users.iter().find(|u| u["name"] == n).unwrap().clone();
    assert_eq!((by("anna")["downloads"].clone(), by("anna")["complete"].clone(), by("anna")["tokens"].clone(), by("anna")["admin"].clone()), (json!(3), json!(2), json!(1), json!(false)));
    assert_eq!((by("ben")["downloads"].clone(), by("ben")["tokens"].clone()), (json!(0), json!(0)), "an account with nothing yet is still listed");
    assert_eq!(by("root")["admin"], true);
    let text = String::from_utf8_lossy(&a.body).to_string();
    assert!(!text.contains("hash") && !text.contains("salt"), "no password material: {text}");
    assert_eq!(get(&s, "/api/users", &anna).code, 403);
    assert_eq!(get(&s, "/api/users", "").code, 401);
    let p = site("users-personal", Mode::Personal, &[]);
    assert_eq!(get(&p, "/api/users", &personal_session(&p)).code, 404);
}

#[test]
fn an_administrator_makes_accounts_and_changes_passwords_from_the_page() {
    let s = site_with("users-make", Mode::Shared, &[("root", "battery staple", true), ("anna", "correct horse", false)], usage_answer);
    let (root, anna) = (login(&s, "root", "battery staple").cookie().unwrap(), login(&s, "anna", "correct horse").cookie().unwrap());
    let made = post(&s, "/api/users", &root, &json!({ "name": "ben", "password": "a new password", "admin": false }).to_string());
    assert_eq!((made.code, made.json()["user"].clone()), (200, json!({ "name": "ben", "admin": false })));
    assert_eq!(login(&s, "ben", "a new password").code, 200, "he can sign in at once");
    // A new password for an existing account.
    assert_eq!(post(&s, "/api/users", &root, &json!({ "name": "ben", "password": "another one entirely", "admin": false }).to_string()).code, 200);
    assert_eq!(login(&s, "ben", "a new password").code, 401);
    assert_eq!(login(&s, "ben", "another one entirely").code, 200);
    // The rules of accounts apply (the same as for --add-user).
    for (name, pw) in [("", "long enough pw"), ("../etc", "long enough pw"), ("carl", "short"), ("x y", "long enough pw")] {
        assert_eq!(post(&s, "/api/users", &root, &json!({ "name": name, "password": pw }).to_string()).code, 400, "{name:?}");
    }
    // Only administrators.
    assert_eq!(post(&s, "/api/users", &anna, &json!({ "name": "carl", "password": "long enough pw" }).to_string()).code, 403);
    assert!(!Users::load(&s.dir).names().iter().any(|(n, _)| n == "carl"));
}

#[test]
fn the_server_always_keeps_one_administrator_and_nobody_removes_themselves() {
    let s = site_with("users-last-admin", Mode::Shared, &[("root", "battery staple", true), ("anna", "correct horse", false)], usage_answer);
    let root = login(&s, "root", "battery staple").cookie().unwrap();
    // Demoting the only administrator is refused.
    let e = post(&s, "/api/users", &root, &json!({ "name": "root", "password": "battery staple", "admin": false }).to_string());
    assert_eq!(e.code, 400);
    assert!(e.json()["error"].as_str().unwrap().contains("administrator"));
    assert!(Users::load(&s.dir).is_admin("root"));
    // Removing yourself is refused.
    let del = |name: &str| send(&s, "DELETE", &format!("/api/users/{name}"), &[("Cookie", &root), ("X-SDM-Request", "1")], "");
    assert_eq!(del("root").code, 400);
    assert_eq!(del("nobody").code, 404);
    // With a second administrator, either may go.
    assert_eq!(post(&s, "/api/users", &root, &json!({ "name": "anna", "password": "correct horse", "admin": true }).to_string()).code, 200);
    assert_eq!(del("anna").code, 200);
    assert!(!Users::load(&s.dir).names().iter().any(|(n, _)| n == "anna"));
}

#[test]
fn removing_an_account_ends_its_sessions_and_its_tokens() {
    let s = site_with("users-remove", Mode::Shared, &[("root", "battery staple", true), ("anna", "correct horse", false)], usage_answer);
    let (root, anna) = (login(&s, "root", "battery staple").cookie().unwrap(), login(&s, "anna", "correct horse").cookie().unwrap());
    let (_, token) = make_token(&s, &anna, "script");
    assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", &bearer(&token))], "").code, 200);
    // A normal user may not remove anybody.
    assert_eq!(send(&s, "DELETE", "/api/users/root", &[("Cookie", &anna), ("X-SDM-Request", "1")], "").code, 403);
    let d = send(&s, "DELETE", "/api/users/anna", &[("Cookie", &root), ("X-SDM-Request", "1")], "");
    assert_eq!(d.code, 200);
    assert!(d.json()["note"].as_str().unwrap().contains("stay"), "the answer says what happens to their files");
    assert_eq!(get(&s, "/api/status", &anna).code, 401, "signed-in page");
    assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", &bearer(&token))], "").code, 401, "token");
    assert!(!std::fs::read_to_string(s.dir.join("tokens.json")).unwrap_or_default().contains("script"), "the token record is gone from disk");
}

// ---------------------------------------------------------------- metrics

#[test]
fn metrics_are_numbers_for_administrators_in_the_prometheus_format() {
    let s = site_with("metrics", Mode::Shared, &[("root", "battery staple", true), ("anna", "correct horse", false)], |c| match c {
        Command::GetStatus => Ok(json!({ "active": 2, "queued": 3, "paused": 1, "complete": 7, "failed": 1, "speed_bytes_per_sec": 1500000.0, "speed_limit_kbps": 2048 })),
        _ => Ok(json!({})),
    });
    let root = login(&s, "root", "battery staple").cookie().unwrap();
    let a = get(&s, "/metrics", &root);
    assert_eq!(a.code, 200);
    assert!(a.header("content-type").starts_with("text/plain"), "{}", a.header("content-type"));
    let text = String::from_utf8_lossy(&a.body).to_string();
    for line in ["sdm_downloads{state=\"active\"} 2", "sdm_downloads{state=\"queued\"} 3", "sdm_downloads{state=\"complete\"} 7", "sdm_downloads{state=\"failed\"} 1", "sdm_speed_bytes_per_second 1500000", "sdm_speed_limit_kbps 2048"] {
        assert!(text.lines().any(|l| l == line), "missing {line:?} in\n{text}");
    }
    assert!(text.contains("# TYPE sdm_downloads gauge"));
    // A bearer token of an administrator works too (that is how a monitoring system reads it).
    let (_, token) = make_token(&s, &root, "prometheus");
    assert_eq!(send(&s, "GET", "/metrics", &[("Authorization", &bearer(&token))], "").code, 200);
    // Nobody else, and nobody without a sign-in.
    let anna = login(&s, "anna", "correct horse").cookie().unwrap();
    assert_eq!(get(&s, "/metrics", &anna).code, 403);
    assert_eq!(get(&s, "/metrics", "").code, 401);
}
