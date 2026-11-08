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
fn a_network_plugged_in_later_is_taken_into_use() {
    let router = Router::new(vec![route("wifi", 1)]);
    let now = Instant::now();
    // Two connections are running over Wi-Fi.
    let held: Vec<Lease> = (0..2).map(|_| router.acquire_at(now).unwrap()).collect();
    assert!(held.iter().all(|l| l.route().name == "wifi"));
    // Ethernet is plugged in: the next connection goes there, because it has none yet.
    assert_eq!(router.refresh(&[route("wifi", 1), route("eth", 2)]), 1);
    assert_eq!(router.len(), 2);
    assert_eq!(router.acquire_at(now).unwrap().route().name, "eth", "the new network has no connection yet, so it comes first");
    // Nothing new on a second look.
    assert_eq!(router.refresh(&[route("wifi", 1), route("eth", 2)]), 0);
}

#[test]
fn a_network_that_is_unplugged_gets_no_more_connections_and_is_used_again_when_it_returns() {
    let router = Router::new(vec![route("wifi", 1), route("usb", 2)]);
    let now = Instant::now();
    assert_eq!(router.refresh(&[route("wifi", 1)]), 0, "nothing new; the phone is gone");
    let status = router.status();
    assert!(status.iter().find(|s| s.name == "usb").unwrap().gone, "{status:?}");
    assert!(!status.iter().find(|s| s.name == "usb").unwrap().resting, "gone is not the same as resting");
    assert_eq!(router.usable(), 1);
    assert_eq!(names(&router, 3, now), ["wifi", "wifi", "wifi"]);
    // The phone is plugged in again.
    assert_eq!(router.refresh(&[route("wifi", 1), route("usb", 2)]), 1);
    assert_eq!(router.usable(), 2);
    assert!(!router.status().iter().any(|s| s.gone));
}

#[test]
fn a_changed_address_replaces_the_old_one_and_ends_the_rest() {
    let router = Router::new(vec![route("wifi", 1)]);
    let now = Instant::now();
    let lease = router.acquire_at(now).unwrap();
    lease.fail_at(now);
    drop(lease);
    assert!(router.acquire_at(now).is_none(), "it is resting");
    // The router handed out a new address: that is a different story from the one that failed.
    router.refresh(&[route("wifi", 9)]);
    let lease = router.acquire_at(now).expect("tried again at once");
    assert_eq!(lease.route().addr, route("wifi", 9).addr);
}

#[test]
fn a_look_at_the_networks_never_lets_the_last_one_go_missing_from_the_count() {
    let router = Router::new(vec![route("wifi", 1)]);
    // Everything gone: the router still knows its networks (so a download says "none is working" instead of
    // quietly using the normal way out).
    router.refresh(&[]);
    assert_eq!(router.len(), 1);
    assert_eq!(router.usable(), 0);
    assert!(router.acquire().is_none());
}

#[test]
fn a_connection_that_broke_midway_rests_its_network_only_briefly_and_never_the_last_one() {
    let router = Router::new(vec![route("wifi", 1), route("eth", 2)]);
    let now = Instant::now();
    let a = router.acquire_at(now).unwrap();
    let bad = a.route().name.clone();
    a.fail_soft_at(now);
    drop(a);
    // Rests for a few seconds ...
    for _ in 0..3 {
        let next = router.acquire_at(now + Duration::from_secs(1)).unwrap();
        assert_ne!(next.route().name, bad);
        std::mem::forget(next);
    }
    // ... and is back long before a failed network would be.
    assert!(SOFT_COOLDOWN < COOLDOWN);
    let after = now + SOFT_COOLDOWN + Duration::from_secs(1);
    assert!(names(&router, 2, after).contains(&bad));

    // With only one usable network left, a broken connection does not take it out.
    let router = Router::new(vec![route("wifi", 1), route("eth", 2)]);
    router.refresh(&[route("wifi", 1)]);
    let a = router.acquire_at(now).unwrap();
    assert_eq!(a.route().name, "wifi");
    a.fail_soft_at(now);
    drop(a);
    assert!(router.acquire_at(now).is_some(), "the last network keeps working");
}

#[test]
fn automatic_mode_uses_every_connected_network_except_those_switched_off() {
    let all = vec![
        Iface { name: "Wi-Fi".into(), addr: "192.168.1.5".parse().unwrap(), index: Some(8), kind: Kind::Wifi },
        Iface { name: "Ethernet".into(), addr: "10.1.1.9".parse().unwrap(), index: Some(10), kind: Kind::Ethernet },
        Iface { name: "Ethernet 3".into(), addr: "192.168.42.129".parse().unwrap(), index: Some(14), kind: Kind::Usb },
    ];
    let auto = Choice { on: true, auto: true, ..Default::default() };
    let names = |c: &Choice, list: &[Iface]| c.routes(list).into_iter().map(|r| r.name).collect::<Vec<_>>();
    assert_eq!(names(&auto, &all), ["Wi-Fi", "Ethernet", "Ethernet 3"]);
    // A phone on a metered plan can be switched off.
    let without_phone = Choice { off: vec!["Ethernet 3".into()], ..auto.clone() };
    assert_eq!(names(&without_phone, &all), ["Wi-Fi", "Ethernet"]);
    // A network that appears later is simply in the next list.
    assert_eq!(names(&auto, &all[..2]), ["Wi-Fi", "Ethernet"]);
    // Off means off, whatever else is set; by hand means the picked ones only.
    assert!(Choice { auto: true, ..Default::default() }.routes(&all).is_empty());
    let by_hand = Choice { on: true, names: vec!["Ethernet 3".into(), "Wi-Fi".into(), "gone".into()], ..Default::default() };
    assert_eq!(names(&by_hand, &all), ["Ethernet 3", "Wi-Fi"]);
}

#[test]
fn automatic_mode_stands_back_while_a_vpn_is_connected() {
    let all = vec![
        Iface { name: "Wi-Fi".into(), addr: "192.168.1.5".parse().unwrap(), index: Some(8), kind: Kind::Wifi },
        Iface { name: "Ethernet".into(), addr: "10.1.1.9".parse().unwrap(), index: Some(10), kind: Kind::Ethernet },
        Iface { name: "wg0".into(), addr: "10.8.0.2".parse().unwrap(), index: Some(20), kind: Kind::Vpn },
    ];
    // Pushing traffic out of one card by hand would go round the VPN.
    assert!(Choice { on: true, auto: true, ..Default::default() }.routes(&all).is_empty());
    // Picking by hand is a decision, and is respected.
    assert_eq!(Choice { on: true, names: vec!["Wi-Fi".into(), "Ethernet".into()], ..Default::default() }.routes(&all).len(), 2);
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
        // A phone tethered by cable, or a USB network adapter.
        ("usb0", Kind::Usb),
        ("rndis0", Kind::Usb),
        ("enx0a1b2c3d4e5f", Kind::Usb),
        ("Remote NDIS based Internet Sharing Device", Kind::Usb),
        ("Apple Mobile Device Ethernet", Kind::Usb),
        ("Ethernet 3 (USB)", Kind::Usb),
        // A phone or modem on the mobile network.
        ("rmnet_data0", Kind::Phone),
        ("wwan0", Kind::Phone),
        ("Mobile Broadband Connection", Kind::Phone),
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

// --- Windows: the card is named, not just the address -------------------------------------------

/// The index of the loopback card, which every computer has.
#[cfg(windows)]
fn loopback_index() -> u32 {
    if_addrs::get_if_addrs().unwrap().into_iter().find(|i| i.is_loopback()).and_then(|i| i.index).expect("a loopback card with an index")
}

#[cfg(windows)]
#[test]
fn windows_can_be_told_which_card_to_use_and_obeys() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let to = listener.local_addr().unwrap();
    let connect_via = |index: u32| -> io::Result<TcpStream> {
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
        force::unicast_if(&socket, true, index)?;
        socket.connect_timeout(&to.into(), Duration::from_secs(3))?;
        Ok(socket.into())
    };
    // Through the card that really carries this address: it works (so the number is written the way Windows reads it).
    connect_via(loopback_index()).expect("loopback through the loopback card");
    // Through a card that does not exist: refused, so the choice is not just ignored.
    assert!(connect_via(9999).is_err(), "a made-up card cannot carry the connection");
}

#[cfg(windows)]
#[test]
fn a_route_with_a_card_number_connects_through_that_card() {
    let (port, peers) = peer_recorder();
    let good = Route { name: "lo".into(), addr: "127.0.0.1".parse().unwrap(), index: Some(loopback_index()) };
    let mut resp = agent(config(), Some(&good), false).get(&format!("http://127.0.0.1:{port}/")).call().unwrap();
    assert_eq!(resp.body_mut().read_to_string().unwrap(), "ok");
    assert_eq!(peers.lock().unwrap().len(), 1);
}

#[test]
fn the_networks_command_lists_each_card_with_its_kind_and_whether_automatic_mode_would_use_it() {
    let list = vec![
        Iface { name: "Wi-Fi".into(), addr: "192.168.1.5".parse().unwrap(), index: Some(8), kind: Kind::Wifi },
        Iface { name: "Ethernet 3".into(), addr: "192.168.42.129".parse().unwrap(), index: Some(14), kind: Kind::Usb },
    ];
    let text = describe(&list);
    assert!(text.lines().next().unwrap().starts_with("Name"), "{text}");
    let usb = text.lines().find(|l| l.starts_with("Ethernet 3")).unwrap();
    assert!(usb.contains("USB") && usb.contains("192.168.42.129") && usb.ends_with("yes"), "{usb}");
    // A VPN makes automatic mode stand back, and the list says so.
    let mut with_vpn = list.clone();
    with_vpn.push(Iface { name: "wg0".into(), addr: "10.8.0.2".parse().unwrap(), index: Some(20), kind: Kind::Vpn });
    let text = describe(&with_vpn);
    assert!(text.lines().find(|l| l.starts_with("Wi-Fi")).unwrap().contains("stands back"), "{text}");
    assert!(text.lines().find(|l| l.starts_with("wg0")).unwrap().contains("a VPN"), "{text}");
    assert_eq!(describe(&[]), "No connected network was found.\n");
}

/// Needs internet: run with `cargo test -- --ignored real_internet`. Every connected card is used to fetch a
/// small page, with its address and (on Windows) its card number, and must get through.
#[test]
#[ignore = "needs the internet"]
fn real_internet_through_every_connected_card() {
    let cards = interfaces();
    assert!(!cards.is_empty(), "no connected network on this computer");
    for card in cards.iter().filter(|c| c.addr.is_ipv4() && matches!(c.kind, Kind::Wifi | Kind::Ethernet | Kind::Usb | Kind::Phone)) {
        let r = card.route();
        let url = "http://example.com/";
        let mut resp = agent(config(), Some(&r), false).get(url).call().unwrap_or_else(|e| panic!("{} ({}) could not fetch {url}: {e}", card.name, card.addr));
        let body = resp.body_mut().read_to_string().unwrap();
        assert!(body.contains("Example Domain"), "{}: {body:.80}", card.name);
        eprintln!("{} ({}, {}) fetched {url}", card.name, card.kind.label(), card.addr);
    }
}
