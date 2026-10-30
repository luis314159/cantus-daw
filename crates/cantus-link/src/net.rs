//! The sockets (ADR 0078 §2), opened as the SDK's `platforms/asio/Context.hpp` opens them,
//! one set per interface:
//!
//! - a **multicast receiver** bound to `0.0.0.0:20808` with `SO_REUSEADDR` (two Link apps
//!   on one machine share the port), joined to `224.76.78.75` on the interface, multicast
//!   loop on only for the loopback interface — so two apps on one machine hear each other
//!   over `127.0.0.1` and an app never hears itself twice;
//! - a **sender** bound to the interface's address, multicast out of that interface (the
//!   unicast responses come back to it);
//! - a **measurement socket** bound to the interface's address, whose port is the `mep4`
//!   the node advertises.
//!
//! Each socket has a thread blocked on it (a short timeout, to notice the stop), which
//! stamps every datagram with the host clock the moment it arrives. Pings are answered on
//! the measurement socket's own thread at once, so the responder's ghost time is taken as
//! close to the ping's arrival as the system allows.

use crate::node::{Node, Via, MULTICAST};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A datagram heard: the interface, which socket, from where, its bytes and when (host µs).
#[derive(Debug)]
pub struct Heard {
    pub iface: usize,
    pub via: Via,
    pub from: SocketAddrV4,
    pub bytes: Vec<u8>,
    pub at: i64,
}

/// One interface's three sockets.
#[derive(Debug)]
pub struct Iface {
    pub addr: Ipv4Addr,
    pub sender: UdpSocket,
    pub measure: UdpSocket,
    multicast: UdpSocket,
}

impl Iface {
    /// Open the sockets for `addr`, multicast on `group` (Link's, or a test's port).
    /// `local` (the tests, `Config::local`) binds nothing to every address: the receiver is
    /// bound to the loopback on Windows (where a socket bound to all addresses asks the
    /// firewall) and to the group itself elsewhere, so a test never listens on a network.
    pub fn open(addr: Ipv4Addr, group: SocketAddrV4, local: bool) -> io::Result<Iface> {
        let lo = addr.is_loopback();
        let rx = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        rx.set_reuse_address(true)?;
        #[cfg(target_os = "linux")]
        rx.set_multicast_all_v4(false)?;
        let _ = rx.set_broadcast(!lo);
        rx.set_multicast_loop_v4(lo)?;
        rx.set_multicast_if_v4(&addr)?;
        let at = if !local {
            Ipv4Addr::UNSPECIFIED
        } else if cfg!(windows) {
            Ipv4Addr::LOCALHOST
        } else {
            *group.ip()
        };
        rx.bind(&SockAddr::from(SocketAddrV4::new(at, group.port())))?;
        rx.join_multicast_v4(group.ip(), &addr)?;
        let tx = unicast(addr, lo)?;
        let measure = unicast(addr, lo)?;
        Ok(Iface {
            addr,
            sender: tx,
            measure,
            multicast: rx.into(),
        })
    }

    /// Every socket's own address (the tests: a local handle listens on no network).
    pub fn bound(&self) -> Vec<SocketAddr> {
        [&self.multicast, &self.sender, &self.measure]
            .iter()
            .filter_map(|s| s.local_addr().ok())
            .collect()
    }

    /// The measurement socket's endpoint (the `mep4`).
    pub fn measure_endpoint(&self) -> Option<SocketAddrV4> {
        match self.measure.local_addr().ok()? {
            SocketAddr::V4(a) => Some(a),
            SocketAddr::V6(_) => None,
        }
    }

    /// Send `bytes` to `to` from the socket `via` names.
    pub fn send(&self, via: Via, to: SocketAddrV4, bytes: &[u8]) {
        let s = match via {
            Via::Discovery => &self.sender,
            Via::Measure => &self.measure,
        };
        if let Err(e) = s.send_to(bytes, to) {
            log::debug!("link: send to {to} failed: {e}");
        }
    }

    /// Start a thread on each socket, until `stop`; what they hear goes to `out`. Pings are
    /// answered on the measurement thread through `node`.
    pub fn listen(
        &self,
        index: usize,
        clock: fn() -> i64,
        node: Arc<Mutex<Node>>,
        stop: Arc<AtomicBool>,
        out: SyncSender<Heard>,
    ) -> io::Result<Vec<std::thread::JoinHandle<()>>> {
        let mut threads = Vec::new();
        for (sock, via) in [
            (self.multicast.try_clone()?, Via::Discovery),
            (self.sender.try_clone()?, Via::Discovery),
            (self.measure.try_clone()?, Via::Measure),
        ] {
            sock.set_read_timeout(Some(Duration::from_millis(100)))?;
            let (stop, out, node) = (stop.clone(), out.clone(), node.clone());
            let answer = via == Via::Measure;
            let t = std::thread::Builder::new()
                .name("cantus-link-rx".into())
                .spawn(move || {
                    let mut buf = [0u8; 2048];
                    while !stop.load(Ordering::Relaxed) {
                        let (n, from) = match sock.recv_from(&mut buf) {
                            Ok(x) => x,
                            Err(_) => continue,
                        };
                        let at = clock();
                        let SocketAddr::V4(from) = from else { continue };
                        let bytes = &buf[..n];
                        if answer && bytes.get(8) == Some(&crate::wire::PING) {
                            // Answer at once: the ghost time read as the ping arrived.
                            let reply = node.lock().ok().and_then(|n| {
                                if !n.is_enabled() {
                                    return None;
                                }
                                let ghost = n.xform().host_to_ghost(clock());
                                crate::measure::answer(bytes, &n.session(), ghost)
                            });
                            if let Some(r) = reply {
                                let _ = sock.send_to(&r, from);
                            }
                            continue;
                        }
                        // A bounded queue: a flood is dropped here, never queued without
                        // end (review r33 #10).
                        let _ = out.try_send(Heard {
                            iface: index,
                            via,
                            from,
                            bytes: bytes.to_vec(),
                            at,
                        });
                    }
                })?;
            threads.push(t);
        }
        Ok(threads)
    }
}

fn unicast(addr: Ipv4Addr, lo: bool) -> io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    s.set_multicast_loop_v4(lo)?;
    s.set_multicast_if_v4(&addr)?;
    s.bind(&SockAddr::from(SocketAddrV4::new(addr, 0)))?;
    Ok(s.into())
}

/// How many datagrams wait for the node's loop; past it, the newest are dropped.
pub const HEARD_QUEUE: usize = 1024;

/// The interfaces to speak on, as the SDK scans them (`InterfaceScanner.hpp`): the
/// loopback (other apps on this machine) and every IPv4 address of every interface that is
/// up — Wi-Fi, Ethernet, a virtual switch — so a peer on any of them is heard (review r33
/// #6: only the interface routing the group was used, on this machine a WSL switch). When
/// the system cannot list them, the one it routes Link's group through (connecting a UDP
/// socket sends nothing). `only_loopback` keeps to the machine (the tests).
pub fn interfaces(only_loopback: bool) -> Vec<Ipv4Addr> {
    let mut v = vec![Ipv4Addr::LOCALHOST];
    if only_loopback {
        return v;
    }
    match if_addrs::get_if_addrs() {
        Ok(list) => {
            for i in list {
                if let if_addrs::IfAddr::V4(a) = i.addr {
                    let ip = a.ip;
                    if !ip.is_loopback() && !ip.is_unspecified() && !v.contains(&ip) {
                        v.push(ip);
                    }
                }
            }
        }
        Err(e) => {
            log::warn!("link: cannot list the interfaces: {e}");
            let primary = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
                .and_then(|s| {
                    s.connect(MULTICAST)?;
                    s.local_addr()
                })
                .ok()
                .and_then(|a| match a {
                    SocketAddr::V4(a) if !a.ip().is_loopback() && !a.ip().is_unspecified() => {
                        Some(*a.ip())
                    }
                    _ => None,
                });
            if let Some(p) = primary {
                v.push(p);
            }
        }
    }
    v
}

/// Write the send time into a ping's host time (bytes 17..25: the header, the type, the
/// `__ht` key and size): a ping is stamped as it leaves, not when it was made.
pub fn restamp_ping(bytes: &mut [u8], now: i64) {
    if bytes.len() >= 25
        && bytes[..8] == crate::wire::LINK_HEADER
        && bytes[8] == crate::wire::PING
        && bytes[9..13] == crate::wire::KEY_HOST_TIME.to_be_bytes()
    {
        bytes[17..25].copy_from_slice(&now.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The integrator's rule (round 33): a test's sockets never listen on a network — no
    /// firewall prompt — so `Config::local` speaks on the loopback alone, every socket
    /// bound to it (or, off Windows, the multicast receiver to the group itself).
    #[test]
    fn a_local_handle_binds_the_loopback_only() {
        let cfg = crate::Config::local(20_935);
        assert_eq!(interfaces(cfg.only_loopback), vec![Ipv4Addr::LOCALHOST]);
        let i = Iface::open(Ipv4Addr::LOCALHOST, cfg.group, true).expect("sockets");
        for a in i.bound() {
            let SocketAddr::V4(a) = a else { panic!("{a}") };
            assert!(
                a.ip().is_loopback() || (!cfg!(windows) && *a.ip() == *cfg.group.ip()),
                "bound to {a}"
            );
        }
    }
}
