//! The real HTTPS client, not a recorded answer (the owner, 2026-10-22: 0.1.0-beta.1's
//! update check stayed on "Asking GitHub…" for good — ureq 3.4 asks for its `native-tls`
//! feature where the crate enabled only `native-tls-no-default`, and panicked on the
//! check's thread; every other test of the updater fetched through a fake).

use cantus_update::net::{Fetch, Https};
use std::io::Read;
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// A server that takes the connection and says nothing sensible: the TLS handshake fails.
/// The client answers with an error, at once — never a panic.
#[test]
fn an_https_request_that_fails_is_an_error_never_a_panic() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            let _ = s.read(&mut buf);
            // Plain words where a TLS server hello belongs, then gone.
            let _ = std::io::Write::write_all(&mut s, b"HTTP/1.1 400 Not TLS\r\n\r\n");
        }
    });
    let https = Https::new(None);
    let t = Instant::now();
    let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        https.get(&format!("https://127.0.0.1:{port}/releases"), None)
    }));
    let _ = server.join();
    let answer = got.expect("the client did not panic");
    assert!(
        answer.is_err(),
        "a failed handshake is an error: {answer:?}"
    );
    assert!(
        t.elapsed() < Duration::from_secs(20),
        "and said at once: {:?}",
        t.elapsed()
    );
}
