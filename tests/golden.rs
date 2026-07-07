use std::io::Write;
use std::process::{Command, Stdio};

fn run_statusline(json: &str) -> String {
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

/// Strip ANSI SGR escape sequences for plain-text assertions.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            // consume until first letter (m, A-Z, a-z)
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

// ── normal context (50% remaining → 60% used) ────────────────────────────────

#[test]
fn golden_normal_components_present() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    let out = strip_ansi(&raw);
    // expected plain text (order matters):
    // dir statusline-test | (not a git repository) | mdl Test Model | ctx [...] 60% 100K/200K
    assert!(out.contains("dir"), "missing dir prefix");
    assert!(out.contains("statusline-test"), "missing directory name");
    assert!(
        out.contains("(not a git repository)"),
        "missing git fallback"
    );
    assert!(out.contains("mdl"), "missing mdl prefix");
    assert!(out.contains("Test Model"), "missing model name");
    assert!(out.contains("ctx"), "missing ctx prefix at normal level");
    assert!(out.contains("60%"), "wrong context percent");
    assert!(out.contains("100K/200K"), "missing token counts");
    assert!(!out.contains('$'), "cost must be hidden when zero");
}

#[test]
fn golden_normal_component_order() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    let out = strip_ansi(&raw);
    let dir_pos = out.find("dir").unwrap();
    let git_pos = out.find("(not a git repository)").unwrap();
    let mdl_pos = out.find("mdl").unwrap();
    let ctx_pos = out.find("ctx").unwrap();
    assert!(dir_pos < git_pos, "dir must come before git");
    assert!(git_pos < mdl_pos, "git must come before mdl");
    assert!(mdl_pos < ctx_pos, "mdl must come before ctx");
}

#[test]
fn golden_normal_no_blink() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    // SGR 5 (blink) must not appear at 60% usage
    assert!(
        !raw.contains("\x1b[5m"),
        "no blink expected at 60%: {raw:?}"
    );
}

// ── warning tier (25% remaining → 90% used) ──────────────────────────────────

#[test]
fn golden_warning_single_bang() {
    let raw = run_statusline(include_str!("fixtures/ctx-warning.json"));
    let out = strip_ansi(&raw);
    assert!(out.contains("! ["), "warning must show single !: {out}");
    assert!(
        !out.contains("!! ["),
        "warning must not show double !!: {out}"
    );
    assert!(
        out.contains("90%"),
        "wrong context percent at warning: {out}"
    );
}

#[test]
fn golden_warning_blinks() {
    let raw = run_statusline(include_str!("fixtures/ctx-warning.json"));
    assert!(raw.contains("\x1b[5m"), "warning ! must blink: {raw:?}");
    assert!(raw.contains("\x1b[25m"), "blink must be reset: {raw:?}");
}

#[test]
fn golden_warning_no_ctx_prefix() {
    let raw = run_statusline(include_str!("fixtures/ctx-warning.json"));
    let out = strip_ansi(&raw);
    assert!(
        !out.contains("ctx"),
        "ctx prefix must be absent at warning level: {out}"
    );
}

// ── critical tier (3% remaining → 100% used) ─────────────────────────────────

#[test]
fn golden_critical_double_bang() {
    let raw = run_statusline(include_str!("fixtures/ctx-critical.json"));
    let out = strip_ansi(&raw);
    assert!(out.contains("!! ["), "critical must show double !!: {out}");
    assert!(
        out.contains("100%"),
        "wrong context percent at critical: {out}"
    );
}

#[test]
fn golden_critical_blinks() {
    let raw = run_statusline(include_str!("fixtures/ctx-critical.json"));
    assert!(raw.contains("\x1b[5m"), "critical !! must blink: {raw:?}");
    assert!(raw.contains("\x1b[25m"), "blink must be reset: {raw:?}");
}

// ── cost component ────────────────────────────────────────────────────────────

#[test]
fn golden_cost_shown_when_nonzero() {
    let raw = run_statusline(include_str!("fixtures/with-cost.json"));
    let out = strip_ansi(&raw);
    assert!(out.contains("$ 1.23"), "cost must show amount: {out}");
}

#[test]
fn golden_cost_after_ctx() {
    let raw = run_statusline(include_str!("fixtures/with-cost.json"));
    let out = strip_ansi(&raw);
    let ctx_pos = out.find("ctx").unwrap();
    let cost_pos = out.find("$ 1.23").unwrap();
    assert!(ctx_pos < cost_pos, "cost must come after ctx");
}

#[test]
fn golden_cost_hidden_when_zero() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    let out = strip_ansi(&raw);
    // zero cost — no $ component should appear
    // the separator ' | ' only appears between actual components
    let parts: Vec<&str> = out.trim_end_matches('\n').split(" | ").collect();
    assert!(
        !parts.iter().any(|p| p.starts_with("$ ")),
        "cost segment must be absent when zero: {parts:?}"
    );
}

// ── separators and structure ──────────────────────────────────────────────────

#[test]
fn golden_separators_between_components() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    let out = strip_ansi(&raw);
    // every non-empty component is joined by " | "
    assert!(out.contains(" | "), "components must be separated by ' | '");
    // no leading or trailing separator
    assert!(!out.starts_with(" | "), "no leading separator");
    assert!(
        !out.trim_end_matches('\n').ends_with(" | "),
        "no trailing separator"
    );
}

#[test]
fn golden_ends_with_newline() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    assert!(raw.ends_with('\n'), "output must end with newline");
}

// ── progress bar structure ────────────────────────────────────────────────────

#[test]
fn golden_bar_uses_block_chars() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    assert!(
        raw.contains('█') || raw.contains('░'),
        "bar must contain block chars"
    );
}

#[test]
fn golden_bar_60pct_has_filled_and_empty() {
    let raw = run_statusline(include_str!("fixtures/ctx-normal.json"));
    assert!(raw.contains('█'), "60% bar must have filled chars");
    assert!(raw.contains('░'), "60% bar must have empty chars");
}

#[test]
fn golden_bar_100pct_is_fully_filled() {
    let raw = run_statusline(include_str!("fixtures/ctx-critical.json"));
    // 100% used: all 15 chars should be filled, none empty
    assert!(raw.contains('█'), "100% bar must have filled chars");
    assert!(!raw.contains('░'), "100% bar must not have empty chars");
}
