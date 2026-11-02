//! Download over several networks at once (Wi-Fi + Ethernet + a phone's hotspot ...).
//!
//! Every connection of a download is tied to one network card by binding its socket to that card's
//! address. A [`Router`] decides which card the next connection uses, so the speeds add up: each card
//! always gets a connection, and the faster ones get more. A card that stops working is left alone for a
//! while and the others carry on.

use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};
use ureq::Error;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{
    Buffers, ConnectProxyConnector, ConnectionDetails, Connector, DefaultConnector, Either, LazyBuffers, NextTimeout, RustlsConnector, ShutdownHandle, Transport,
};

/// How long a network that failed is left alone before it is tried again.
pub const COOLDOWN: Duration = Duration::from_secs(30);

/// One way out to the internet: the address of a network card on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// The card's name, e.g. "wlan0" or "Ethernet".
    pub name: String,
    pub addr: IpAddr,
    /// The card's number, used on macOS to pin traffic to it.
    pub index: Option<u32>,
}

/// What a network card is, guessed from its name (only used for the label and the icon).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Wifi,
    Ethernet,
    /// A phone: USB tethering, a Bluetooth/hotspot link or a mobile modem.
    Phone,
    Vpn,
    /// Docker, virtual machines and the like: never offered.
    Virtual,
    Other,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Wifi => "Wi-Fi",
            Kind::Ethernet => "Ethernet",
            Kind::Phone => "Phone",
            Kind::Vpn => "VPN",
            Kind::Virtual => "Virtual",
            Kind::Other => "Network",
        }
    }
}

pub fn kind_of(name: &str) -> Kind {
    let n = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    let starts = |words: &[&str]| words.iter().any(|w| n.starts_with(w));
    if starts(&["docker", "veth", "br-", "virbr", "vmnet", "vboxnet", "lxc", "cni", "flannel", "podman"]) || has(&["vethernet", "virtualbox", "vmware", "hyper-v", "loopback", "bluetooth network"]) {
        Kind::Virtual
    } else if starts(&["tun", "tap", "wg", "utun", "zt", "tailscale", "ppp"]) || has(&["vpn", "wireguard", "tailscale", "openvpn"]) {
        // "ppp" is also used by mobile modems, but those show up as wwan/rmnet in practice.
        Kind::Vpn
    } else if starts(&["usb", "rndis", "wwan", "rmnet", "ccmni", "ww"]) || has(&["rndis", "ndis", "tether", "iphone", "android", "cellular", "mobile broadband", "hotspot"]) {
        Kind::Phone
    } else if starts(&["wlan", "wl", "wifi", "wi-fi", "ath", "ra"]) || has(&["wi-fi", "wifi", "wireless", "wlan"]) {
        Kind::Wifi
    } else if starts(&["eth", "en", "em", "eno", "ens", "enp"]) || has(&["ethernet", "lan"]) {
        Kind::Ethernet
    } else {
        Kind::Other
    }
}

/// A network card that could carry a download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Iface {
    pub name: String,
    pub addr: IpAddr,
    pub index: Option<u32>,
    pub kind: Kind,
}

impl Iface {
    pub fn route(&self) -> Route {
        Route { name: self.name.clone(), addr: self.addr, index: self.index }
    }
}

/// The network cards that are up and have a usable address: one entry per card (its IPv4 address when
/// it has one). Loopback, link-local and virtual cards are left out.
pub fn interfaces() -> Vec<Iface> {
    pick_interfaces(
        if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .filter(|i| i.is_oper_up() && !i.is_loopback() && !i.is_link_local())
            .map(|i| (i.name.clone(), i.ip(), i.index)),
    )
}

fn pick_interfaces(all: impl Iterator<Item = (String, IpAddr, Option<u32>)>) -> Vec<Iface> {
    let mut out: Vec<Iface> = Vec::new();
    for (name, addr, index) in all {
        let kind = kind_of(&name);
        if kind == Kind::Virtual || addr.is_unspecified() || addr.is_multicast() {
            continue;
        }
        match out.iter_mut().find(|i| i.name == name) {
            // Keep the IPv4 address of a card that has both.
            Some(existing) => {
                if existing.addr.is_ipv6() && addr.is_ipv4() {
                    existing.addr = addr;
                }
            }
            None => out.push(Iface { name, addr, index, kind }),
        }
    }
    out
}

/// Turn the names saved in the settings into routes with the cards' current addresses
/// (addresses change, names do not). Cards that are not connected right now are skipped.
pub fn resolve(names: &[String], available: &[Iface]) -> Vec<Route> {
    names.iter().filter_map(|n| available.iter().find(|i| &i.name == n)).map(Iface::route).collect()
}

// ---------------------------------------------------------------------------------------------
// Choosing a network for each connection
// ---------------------------------------------------------------------------------------------

struct Slot {
    route: Route,
    active: usize,
    bytes: u64,
    first_use: Option<Instant>,
    down_until: Option<Instant>,
}

impl Slot {
    fn usable(&self, now: Instant) -> bool {
        self.down_until.is_none_or(|t| now >= t)
    }
    /// Bytes per second since the card was first used (0 until there is data).
    fn speed(&self, now: Instant) -> f64 {
        match self.first_use {
            Some(t) if self.bytes > 0 => self.bytes as f64 / now.saturating_duration_since(t).as_secs_f64().max(1.0),
            _ => 0.0,
        }
    }
}

/// Hands out networks to connections.
pub struct Router {
    slots: Mutex<Vec<Slot>>,
}

/// What the router knows about one network, for showing in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteStatus {
    pub name: String,
    pub connections: usize,
    pub bytes_per_sec: f64,
    pub resting: bool,
}

impl Router {
    pub fn new(routes: Vec<Route>) -> Arc<Router> {
        let slots = routes.into_iter().map(|route| Slot { route, active: 0, bytes: 0, first_use: None, down_until: None }).collect();
        Arc::new(Router { slots: Mutex::new(slots) })
    }

    pub fn len(&self) -> usize {
        self.slots.lock().unwrap().len()
    }

    /// How many networks can be used right now.
    pub fn usable(&self) -> usize {
        let now = Instant::now();
        self.slots.lock().unwrap().iter().filter(|s| s.usable(now)).count()
    }

    pub fn status(&self) -> Vec<RouteStatus> {
        let now = Instant::now();
        self.slots
            .lock()
            .unwrap()
            .iter()
            .map(|s| RouteStatus { name: s.route.name.clone(), connections: s.active, bytes_per_sec: s.speed(now), resting: !s.usable(now) })
            .collect()
    }

    /// The network for the next connection, or `None` when every network is resting.
    pub fn acquire(self: &Arc<Self>) -> Option<Lease> {
        self.acquire_at(Instant::now())
    }

    fn acquire_at(self: &Arc<Self>, now: Instant) -> Option<Lease> {
        let mut slots = self.slots.lock().unwrap();
        let usable: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].usable(now)).collect();
        // Every network gets a connection before any gets a second one.
        let idle = usable.iter().copied().find(|i| slots[*i].active == 0);
        let pick = idle.or_else(|| {
            let speeds: Vec<f64> = usable.iter().map(|i| slots[*i].speed(now)).collect();
            // Once every network has shown its speed, connections are shared out in that proportion.
            let measured = !speeds.is_empty() && speeds.iter().all(|s| *s > 0.0);
            usable
                .iter()
                .zip(&speeds)
                .map(|(i, s)| (*i, slots[*i].active as f64 / if measured { *s } else { 1.0 }))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        })?;
        let slot = &mut slots[pick];
        slot.active += 1;
        slot.first_use.get_or_insert(now);
        Some(Lease { router: self.clone(), index: pick, route: slot.route.clone() })
    }
}

/// A connection's claim on one network. Dropping it hands the network back.
pub struct Lease {
    router: Arc<Router>,
    index: usize,
    route: Route,
}

impl Lease {
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Count downloaded bytes, so the router knows how fast this network is.
    pub fn add_bytes(&self, n: u64) {
        self.router.slots.lock().unwrap()[self.index].bytes += n;
    }

    /// This network did not work: leave it alone for a while.
    pub fn fail(&self) {
        self.fail_at(Instant::now());
    }

    fn fail_at(&self, now: Instant) {
        let mut slots = self.router.slots.lock().unwrap();
        let slot = &mut slots[self.index];
        slot.down_until = Some(now + COOLDOWN);
        // Measure it afresh when it comes back.
        slot.bytes = 0;
        slot.first_use = None;
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut slots = self.router.slots.lock().unwrap();
        let slot = &mut slots[self.index];
        slot.active = slot.active.saturating_sub(1);
    }
}

// ---------------------------------------------------------------------------------------------
// Opening a connection through one network
// ---------------------------------------------------------------------------------------------

/// Open a TCP connection that leaves through the given network.
fn connect_from(route: &Route, to: SocketAddr, timeout: Option<Duration>) -> io::Result<TcpStream> {
    let socket = Socket::new(Domain::for_address(to), Type::STREAM, Some(Protocol::TCP))?;
    // Linux: tie the socket to the card, so the traffic really leaves through it whatever the routing table
    // says. This needs a recent kernel (5.7+) or CAP_NET_RAW, so it is best effort; the source address below
    // is what works everywhere.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let _ = socket.bind_device(Some(route.name.as_bytes()));
    // macOS: the same idea, by the card's number.
    #[cfg(target_os = "macos")]
    if let Some(index) = route.index.and_then(std::num::NonZeroU32::new) {
        let _ = if to.is_ipv4() { socket.bind_device_by_index_v4(Some(index)) } else { socket.bind_device_by_index_v6(Some(index)) };
    }
    socket.bind(&SocketAddr::new(route.addr, 0).into())?;
    match timeout {
        Some(t) => socket.connect_timeout(&to.into(), t.max(Duration::from_millis(10)))?,
        None => socket.connect(&to.into())?,
    }
    Ok(socket.into())
}

/// ureq connector that opens its sockets through one network card.
#[derive(Debug, Clone)]
pub struct BoundTcp {
    route: Route,
}

impl<In: Transport> Connector<In> for BoundTcp {
    type Out = Either<In, BoundTransport>;

    fn connect(&self, details: &ConnectionDetails, chained: Option<In>) -> Result<Option<Self::Out>, Error> {
        if chained.is_some() {
            // Something earlier in the chain already made the connection.
            return Ok(chained.map(Either::A));
        }
        let config = details.config;
        // Only addresses of the same kind as the card's own can be reached from it.
        let targets: Vec<SocketAddr> = details.addrs.iter().copied().filter(|a| a.is_ipv4() == self.route.addr.is_ipv4()).collect();
        if targets.is_empty() {
            return Err(Error::Io(io::Error::new(io::ErrorKind::AddrNotAvailable, format!("{} cannot reach this server (no matching address)", self.route.name))));
        }
        let budget = details.timeout.not_zero().map(|t| *t);
        let started = Instant::now();
        let mut last: Option<Error> = None;
        for to in targets {
            let left = budget.map(|b| b.saturating_sub(started.elapsed()));
            match connect_from(&self.route, to, left) {
                Ok(stream) => {
                    if config.no_delay() {
                        stream.set_nodelay(true)?;
                    }
                    let transport = BoundTransport::new(stream, LazyBuffers::new(config.input_buffer_size(), config.output_buffer_size()));
                    (details.register_shutdown)(transport.shutdown_handle())?;
                    return Ok(Some(Either::B(transport)));
                }
                Err(e) if e.kind() == io::ErrorKind::TimedOut => last = Some(Error::Timeout(ureq::Timeout::Connect)),
                Err(e) => last = Some(Error::Io(e)),
            }
        }
        Err(last.unwrap_or_else(|| Error::Io(io::ErrorKind::ConnectionRefused.into())))
    }
}

/// A socket opened by [`BoundTcp`].
pub struct BoundTransport {
    stream: Arc<TcpStream>,
    buffers: LazyBuffers,
    timeout_write: Option<Duration>,
    timeout_read: Option<Duration>,
}

impl std::fmt::Debug for BoundTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundTransport").field("local", &self.stream.local_addr().ok()).field("peer", &self.stream.peer_addr().ok()).finish()
    }
}

impl BoundTransport {
    fn new(stream: TcpStream, buffers: LazyBuffers) -> Self {
        BoundTransport { stream: Arc::new(stream), buffers, timeout_write: None, timeout_read: None }
    }

    fn shutdown_handle(&self) -> Weak<dyn ShutdownHandle> {
        // The agent keeps only a weak reference, so the socket still closes when the transport is dropped.
        Arc::downgrade(&self.stream) as Weak<dyn ShutdownHandle>
    }
}

fn io_error(e: io::Error, reason: ureq::Timeout) -> Error {
    match e.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Error::Timeout(reason),
        _ => Error::Io(e),
    }
}

impl Transport for BoundTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), Error> {
        let timeout = timeout.for_write();
        if timeout.after.is_zero() {
            return Err(Error::Timeout(timeout.reason));
        }
        let wanted = timeout.not_zero().map(|t| *t);
        if wanted != self.timeout_write {
            self.stream.set_write_timeout(wanted)?;
            self.timeout_write = wanted;
        }
        let mut stream = &*self.stream;
        stream.write_all(&self.buffers.output()[..amount]).map_err(|e| io_error(e, timeout.reason))
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, Error> {
        let timeout = timeout.for_read();
        if timeout.after.is_zero() {
            return Err(Error::Timeout(timeout.reason));
        }
        let wanted = timeout.not_zero().map(|t| *t);
        if wanted != self.timeout_read {
            self.stream.set_read_timeout(wanted)?;
            self.timeout_read = wanted;
        }
        let mut stream = &*self.stream;
        let amount = loop {
            match stream.read(self.buffers.input_append_buf()) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_error(e, timeout.reason)),
                Ok(n) => break n,
            }
        };
        self.buffers.input_appended(amount);
        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        // Nothing should be waiting on a connection that is idle; a closed one reads as end of file.
        if self.stream.set_nonblocking(true).is_err() {
            return false;
        }
        let mut one = [0u8];
        let open = matches!((&*self.stream).read(&mut one), Err(e) if e.kind() == io::ErrorKind::WouldBlock);
        open && self.stream.set_nonblocking(false).is_ok()
    }
}

/// Whether an address is on a private network or this computer itself: somewhere a public download
/// server has no business fetching from (the cloud "metadata" address 169.254.169.254, the office network,
/// services that listen on localhost).
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                // 100.64.0.0/10: shared address space used by carriers and some clouds.
                || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 unique local and fe80::/10 link local.
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

/// Looks names up like ureq does, then drops the private addresses. Doing it here (and not before the request)
/// means it also covers redirects and cannot be raced by a name that changes its answer.
#[derive(Debug)]
struct Guarded {
    inner: DefaultResolver,
    block_private: bool,
}

impl Resolver for Guarded {
    fn resolve(&self, uri: &ureq::http::Uri, config: &ureq::config::Config, timeout: NextTimeout) -> Result<ResolvedSocketAddrs, Error> {
        let found = self.inner.resolve(uri, config, timeout)?;
        if !self.block_private {
            return Ok(found);
        }
        let mut allowed = self.inner.empty();
        found.iter().filter(|a| !is_private(a.ip())).for_each(|a| allowed.push(*a));
        if allowed.is_empty() {
            return Err(Error::Io(io::Error::new(io::ErrorKind::PermissionDenied, "this server may not download from private or local addresses")));
        }
        Ok(allowed)
    }
}

/// An HTTP client whose connections leave through `route` (when there is one) and that can be forbidden
/// from reaching private addresses.
pub fn agent(config: ureq::config::Config, route: Option<&Route>, block_private: bool) -> ureq::Agent {
    let resolver = Guarded { inner: DefaultResolver::default(), block_private };
    match route {
        // The same steps as ureq's own default (a proxy tunnel if one is set, a socket, then TLS), with the
        // socket opened through the chosen network.
        Some(route) => {
            let connector = ().chain(ConnectProxyConnector::default()).chain(BoundTcp { route: route.clone() }).chain(RustlsConnector::default());
            ureq::Agent::with_parts(config, connector, resolver)
        }
        None => ureq::Agent::with_parts(config, DefaultConnector::default(), resolver),
    }
}

#[cfg(test)]
mod tests;
