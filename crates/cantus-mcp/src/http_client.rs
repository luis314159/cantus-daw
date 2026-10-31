//! A small HTTP/1.1 client for the loopback door: the stdio bridge's, and the tests'. One
//! kept-alive connection, opened again once if the door closed it. With [`Client::prove`]
//! set, every new connection first asks the door to prove it holds the token, and nothing
//! else is sent on a connection that has not (review 4 #3): the token never reaches
//! whatever else may be listening on the port, even after a crash left the door's file.

use crate::http_wire::{self, Message, WireError};
use std::io::{self, BufReader};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub struct Client {
    addr: SocketAddr,
    conn: Option<(BufReader<TcpStream>, TcpStream)>,
    /// The longest an answer may take (a confirmation can wait on the user).
    pub timeout: Duration,
    /// The token the door must prove it holds, on each new connection before anything is
    /// sent on it.
    pub prove: Option<String>,
    /// The longest the door may take to prove itself (review 5 #8): something that
    /// accepts and never answers is not Cantus, said in seconds, not after `timeout`.
    pub proof_timeout: Duration,
}

/// The door did not prove it holds the token: something else answers on its port.
pub fn not_cantus() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "not Cantus on this port")
}

fn wire(e: WireError) -> io::Error {
    match e {
        WireError::Io(e) => e,
        WireError::Closed => io::Error::new(io::ErrorKind::ConnectionAborted, "closed"),
        WireError::Bad(_, why) => io::Error::new(io::ErrorKind::InvalidData, why),
    }
}

impl Client {
    pub fn new(addr: SocketAddr) -> Self {
        Client {
            addr,
            conn: None,
            timeout: Duration::from_secs(660),
            prove: None,
            proof_timeout: Duration::from_secs(5),
        }
    }

    fn host(&self) -> String {
        format!("127.0.0.1:{}", self.addr.port())
    }

    fn connect(&mut self) -> io::Result<&mut (BufReader<TcpStream>, TcpStream)> {
        if self.conn.is_none() {
            let s = TcpStream::connect_timeout(&self.addr, Duration::from_secs(2))?;
            s.set_read_timeout(Some(self.timeout))?;
            s.set_nodelay(true)?;
            let mut w = s.try_clone()?;
            let mut r = BufReader::new(s);
            if let Some(token) = &self.prove {
                r.get_ref().set_read_timeout(Some(self.proof_timeout))?;
                let nonce = crate::ids::random_hex(16);
                let host = self.host();
                let h = [
                    ("Host", host.as_str()),
                    ("Mcp-Cantus-Proof", nonce.as_str()),
                ];
                http_wire::request(&mut w, "POST", "/mcp", &h, b"")?;
                // No answer in time: something that is not Cantus (-31002 at the bridge).
                let m = http_wire::read(&mut r, 1 << 16, None).map_err(|e| match e {
                    WireError::Io(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                        ) =>
                    {
                        not_cantus()
                    }
                    other => wire(other),
                })?;
                let ok = m.start.0 == "204"
                    && !m.closes()
                    && m.header(crate::http::PROOF_HEADER).is_some_and(|p| {
                        p.eq_ignore_ascii_case(&crate::http::proof(token, &nonce))
                    });
                if !ok {
                    return Err(not_cantus());
                }
                r.get_ref().set_read_timeout(Some(self.timeout))?;
            }
            self.conn = Some((r, w));
        }
        Ok(self.conn.as_mut().expect("just made"))
    }

    /// Send a request to `/mcp` and read its answer.
    pub fn send(
        &mut self,
        method: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> io::Result<Message> {
        let host = self.host();
        let mut h: Vec<(&str, &str)> = Vec::new();
        if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
            h.push(("Host", host.as_str()));
        }
        h.extend_from_slice(headers);
        for attempt in 0..2 {
            let fresh = self.conn.is_none();
            let conn = self.connect()?;
            let sent = http_wire::request(&mut conn.1, method, "/mcp", &h, body);
            let got = sent.and_then(|_| http_wire::read(&mut conn.0, 64 << 20, None).map_err(wire));
            match got {
                Ok(m) => {
                    if m.closes() {
                        self.conn = None;
                    }
                    return Ok(m);
                }
                // A kept connection the door closed meanwhile: once more on a new one
                // (which proves itself again).
                Err(e) if attempt == 0 && !fresh && e.kind() != io::ErrorKind::TimedOut => {
                    self.conn = None;
                }
                Err(e) => {
                    self.conn = None;
                    return Err(e);
                }
            }
        }
        unreachable!("two attempts at most")
    }
}
