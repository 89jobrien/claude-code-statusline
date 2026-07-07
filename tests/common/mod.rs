use std::io::Write;
use std::process::{Command, Stdio};

pub fn run_statusline(json: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_statusline");
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn statusline binary");

    child
        .stdin
        .as_mut()
        .expect("stdin not piped")
        .write_all(json.as_bytes())
        .expect("failed to write stdin");

    let out = child.wait_with_output().expect("failed to wait for output");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
