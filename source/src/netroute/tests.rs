use super::*;
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, TcpListener};
use std::thread;

fn route(name: &str, last: u8) -> Route {
    Route { name: name.into(), addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, last)), index: None }
}

fn names(router: &Arc<Router>, n: usize, now: Instant) -> Vec<String> {
    // Keep every lease alive so each pick sees the connections before it.
    let leases: Vec<Lease> = (0..n).map(|_| router.acquire_at(now).expect("a network")).collect();
    leases.iter().map(|l| l.route().name.clone()).collect()
}

fn count(list: &[String], name: &str) -> usize {
    list.iter().filter(|n| *n == name).count()
}

#[test]
fn every_network_gets_a_connection_before_any_gets_two() {
    let router = Router::new(vec![route("wifi", 1), route("eth", 2), route("phone", 3)]);
    let picked = names(&router, 3, Instant::now());
    assert_eq!(picked, ["wifi", "eth", "phone"]);
}

#[test]
fn connections_are_spread_evenly_until_speeds_are_known() {
    let router = Router::new(vec![route("wifi", 1), route("eth", 2)]);
    let picked = names(&router, 6, Instant::now());
    assert_eq!(count(&picked, "wifi"), 3);
    assert_eq!(count(&picked, "eth"), 3);
}

#[test]
fn the_faster_network_gets_more_connections() {
    let router = Router::new(vec![route("slow", 1), route("fast", 2)]);
    let t0 = Instant::now();
    // One connection each, which then report what they moved in two seconds: 10 MB/s against 1 MB/s.
    let a = router.acquire_at(t0).unwrap();
    let b = router.acquire_at(t0).unwrap();
    let (slow, fast) = if a.route().name == "slow" { (&a, &b) } else { (&b, &a) };
    slow.add_bytes(2_000_000);
    fast.add_bytes(20_000_000);
    let later = t0 + Duration::from_secs(2);
    let more: Vec<Lease> = (0..9).map(|_| router.acquire_at(later).unwrap()).collect();
    // Count everything open now, including the first connection of each network.
    let fast_total = 1 + more.iter().filter(|l| l.route().name == "fast").count();
    let slow_total = 1 + more.iter().filter(|l| l.route().name == "slow").count();
    assert!(fast_total > slow_total * 3, "fast {fast_total}, slow {slow_total}");
    assert!(slow_total >= 1, "the slow network keeps working too");
}

#[test]
fn a_released_connection_frees_its_slot() {
    let router = Router::new(vec![route("wifi", 1), route("eth", 2)]);
    let now = Instant::now();
    let a = router.acquire_at(now).unwrap();
    let _b = router.acquire_at(now).unwrap();
    let freed = a.route().name.clone();
    drop(a);
    // The network that was just handed back is the idle one, so it is next.
    assert_eq!(router.acquire_at(now).unwrap().route().name, freed);
}

#[test]
fn a_failed_network_rests_and_comes_back() {
    let router = Router::new(vec![route("wifi", 1), route("phone", 2)]);
    let now = Instant::now();
    let lease = router.acquire_at(now).unwrap();
    let bad = lease.route().name.clone();
    lease.fail_at(now);
    drop(lease);
    for _ in 0..4 {
        let next = router.acquire_at(now + Duration::from_secs(1)).unwrap();
        assert_ne!(next.route().name, bad, "a resting network is not used");
        std::mem::forget(next); // keep it busy so the picks differ
    }
    let after = now + COOLDOWN + Duration::from_secs(1);
    let picked = names(&router, 2, after);
    assert!(picked.contains(&bad), "it is used again after the rest: {picked:?}");
}

#[test]
fn no_network_when_all_are_resting() {
    let router = Router::new(vec![route("wifi", 1)]);
    let now = Instant::now();
    let lease = router.acquire_at(now).unwrap();
    lease.fail_at(now);
    drop(lease);
    assert!(router.acquire_at(now).is_none());
    assert!(router.acquire_at(now + COOLDOWN).is_some());
}

#[test]
fn the_name_tells_what_kind_of_card_it_is() {
    for (name, kind) in [
        ("wlan0", Kind::Wifi),
        ("wlp3s0", Kind::Wifi),
        ("Wi-Fi", Kind::Wifi),
        ("eth0", Kind::Ethernet),
        ("enp0s31f6", Kind::Ethernet),
        ("Ethernet 2", Kind::Ethernet),
        ("usb0", Kind::Phone),
        ("Remote NDIS based Internet Sharing Device", Kind::Phone),
        ("rmnet_data0", Kind::Phone),
        ("tun0", Kind::Vpn),
        ("wg0", Kind::Vpn),
        ("docker0", Kind::Virtual),
        ("veth1a2b3c", Kind::Virtual),
        ("br-0123abcd", Kind::Virtual),
        ("vEthernet (WSL)", Kind::Virtual),
        ("something", Kind::Other),
    ] {
        assert_eq!(kind_of(name), kind, "{name}");
    }
}

#[test]
fn virtual_cards_are_not_offered_and_a_card_with_two_addresses_is_listed_once() {
    let v4 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5));
    let v6: IpAddr = "2001:db8::5".parse().unwrap();
    let list = pick_interfaces(
        vec![("wlan0".into(), v6, Some(3)), ("wlan0".into(), v4, Some(3)), ("docker0".into(), IpAddr::V4(Ipv4Addr::new(172, 17, 0, 1)), Some(4)), ("eth0".into(), IpAddr::V4(Ipv4Addr::UNSPECIFIED), Some(2))]
            .into_iter(),
    );
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "wlan0");
    assert_eq!(list[0].addr, v4, "the IPv4 address wins");
}

#[test]
fn saved_names_become_routes_with_the_current_addresses() {
    let available = vec![
        Iface { name: "wlan0".into(), addr: "192.168.1.5".parse().unwrap(), index: Some(3), kind: Kind::Wifi },
        Iface { name: "eth0".into(), addr: "10.1.1.9".parse().unwrap(), index: Some(2), kind: Kind::Ethernet },
    ];
    let routes = resolve(&["eth0".into(), "unplugged".into(), "wlan0".into()], &available);
    assert_eq!(routes.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["eth0", "wlan0"]);
    assert_eq!(routes[0].addr, "10.1.1.9".parse::<IpAddr>().unwrap());
}

// --- a real connection through a chosen address -------------------------------------------------

/// Whether this computer lets us use another 127.x.x.x address (Linux and Windows do, macOS does not by default).
fn alias_works(ip: &str) -> bool {
    TcpListener::bind((ip, 0)).is_ok()
}

/// A tiny web server that answers "ok" and remembers who connected.
fn peer_recorder() -> (u16, Arc<Mutex<Vec<IpAddr>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peers = Arc::new(Mutex::new(Vec::new()));
    let seen = peers.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = seen.clone();
            thread::spawn(move || {
                seen.lock().unwrap().push(stream.peer_addr().unwrap().ip());
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                let mut s = stream;
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
            });
        }
    });
    (port, peers)
}

fn config() -> ureq::config::Config {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_connect(Some(Duration::from_secs(5))).build()
}

#[test]
fn a_connection_leaves_from_the_chosen_address() {
    if !alias_works("127.0.0.2") || !alias_works("127.0.0.3") {
        eprintln!("skipped: this computer has no 127.0.0.2 / 127.0.0.3");
        return;
    }
    let (port, peers) = peer_recorder();
    for ip in ["127.0.0.2", "127.0.0.3"] {
        let r = Route { name: "lo".into(), addr: ip.parse().unwrap(), index: None };
        let mut resp = agent(config(), Some(&r), false).get(&format!("http://127.0.0.1:{port}/")).call().unwrap();
        assert_eq!(resp.body_mut().read_to_string().unwrap(), "ok");
    }
    let seen = peers.lock().unwrap().clone();
    assert_eq!(seen, ["127.0.0.2".parse::<IpAddr>().unwrap(), "127.0.0.3".parse().unwrap()]);
}

#[test]
fn an_address_this_computer_does_not_have_fails_cleanly() {
    let (port, _) = peer_recorder();
    // 203.0.113.0/24 is reserved for documentation; no card owns it.
    let r = Route { name: "gone".into(), addr: "203.0.113.7".parse().unwrap(), index: None };
    let err = agent(config(), Some(&r), false).get(&format!("http://127.0.0.1:{port}/")).call().unwrap_err();
    assert!(matches!(err, Error::Io(_)), "{err:?}");
}

#[test]
fn a_card_cannot_reach_a_server_of_the_other_kind() {
    let (port, _) = peer_recorder();
    let r = Route { name: "v6only".into(), addr: "2001:db8::5".parse().unwrap(), index: None };
    // The server's only address is IPv4, so there is nothing this card could connect to.
    let err = agent(config(), Some(&r), false).get(&format!("http://127.0.0.1:{port}/")).call().unwrap_err();
    assert!(err.to_string().contains("no matching address"), "{err}");
}

// --- addresses a shared server must not fetch from -----------------------------------------------

#[test]
fn private_and_local_addresses_are_recognised() {
    for ip in [
        "127.0.0.1", "10.1.2.3", "172.16.0.9", "172.31.255.1", "192.168.1.1", "169.254.169.254", "100.64.0.1", "100.127.255.1", "0.0.0.0", "224.0.0.1", "255.255.255.255",
        "::1", "::", "fe80::1", "fc00::1", "fd12:3456::1", "::ffff:127.0.0.1", "::ffff:10.0.0.1", "ff02::1",
    ] {
        assert!(is_private(ip.parse().unwrap()), "{ip} is private or local");
    }
    for ip in ["8.8.8.8", "1.1.1.1", "172.15.0.1", "172.32.0.1", "100.63.255.255", "100.128.0.1", "93.184.216.34", "2606:4700:4700::1111", "::ffff:8.8.8.8"] {
        assert!(!is_private(ip.parse().unwrap()), "{ip} is public");
    }
}
