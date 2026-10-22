//! The helper dies with the process that started it (ADR 0063, ADR 0066): Linux's
//! parent-death signal, macOS's kqueue watch on the parent's exit. A shell starts the
//! helper in the background, its input a pipe that stays open (so the helper would serve
//! for ever), says the helper's id and exits; the helper must be gone soon after.

#![cfg(unix)]

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only asks whether the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[test]
fn the_helper_ends_when_its_parent_does() {
    let helper = env!("CARGO_BIN_EXE_cantus-sandbox");
    let mut sh = Command::new("sh")
        .arg("-c")
        .arg("sleep 5 | \"$0\" --serve 1 & echo $!; sleep 0.3")
        .arg(helper)
        .stdout(Stdio::piped())
        .spawn()
        .expect("start the shell");
    let mut line = String::new();
    std::io::BufReader::new(sh.stdout.take().expect("piped"))
        .read_line(&mut line)
        .expect("the helper's id");
    let pid: i32 = line.trim().parse().expect("a process id");
    assert!(alive(pid), "the helper runs while its parent does");
    sh.wait().expect("the shell ends");
    let start = Instant::now();
    while alive(pid) && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let took = start.elapsed();
    if alive(pid) {
        // SAFETY: our own child's child, which the test must not leave behind.
        unsafe { libc::kill(pid, libc::SIGKILL) };
        panic!("the helper outlived its parent by {took:?}");
    }
}
