# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust binary statusline for Claude Code CLI displaying (in order):

- Directory
- Git branch (when in a Git repository)
- File changes broken down by type: `M` modified, `A` added, `D` deleted, `?` untracked (when present)
- Worktree count (when more than one worktree exists)
- Model name
- Context usage visualization with progress bar
- Cost tracking (when present and enabled)
- Open PR number and review status for current branch (opt-in, requires `gh` CLI)

**Primary file**: `src/main.rs`
**Language**: Rust 1.96, edition 2024 (pinned via `rust-toolchain.toml`; requires `rustup`)
**Binary name**: `statusline`
**GitHub repo**: `89jobrien/claude-code-statusline`

## Development Commands

### Testing

```bash
# Run all tests (unit + integration)
cargo test

# Run only unit tests (inline #[cfg(test)] in src/*.rs)
cargo test --bins

# Run integration tests (builds the binary first)
cargo test --test integration

# Manual testing
echo '{"model":{"display_name":"Test"},"workspace":{"current_dir":"/tmp"},"context_window":{"context_window_size":200000,"remaining_percentage":72,"current_usage":{"input_tokens":1000,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"cost":{"total_cost_usd":0.42}}' | cargo run --quiet
```

### Building

```bash
# Dev build
cargo build

# Release build
cargo build --release

# Dev install (build debug binary and copy manually)
cargo build && cp target/debug/statusline ~/.claude/statusline
```

### Linting

```bash
cargo clippy -- -D warnings
```

## Architecture

### Data Flow

```
JSON stdin → input.rs (parse) → config.rs (load TOML) → git.rs (git status) → components.rs (build) → render.rs (assemble) → stdout
```

### Module Map

| Module                 | Responsibility                                                                                                                                                                  |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/main.rs`          | Entry point: `--print-defaults`, `--version`, `--configure-settings` flags or parse→build→render pipeline                                                                       |
| `src/configure.rs`     | `--configure-settings <settings_path> <command_path>` — reads/merges/writes `~/.claude/settings.json` atomically                                                                |
| `src/input.rs`         | JSON parsing via `serde_json`; `validate_directory()` for security                                                                                                              |
| `src/config.rs`        | TOML config loading; `BarStyle`/`Language` enums with fallback warnings; `print_defaults()`                                                                                     |
| `src/git.rs`           | Single `git status --porcelain=v2 --branch --untracked-files=all` call; `parse_porcelain_v2()`                                                                                  |
| `src/components.rs`    | All component builders (`build_model`, `build_directory`, `build_git`, `build_files`, `build_worktrees`, `build_context`, `build_cost`, `build_pr`); `build_all()` orchestrator |
| `src/render.rs`        | ANSI color constants, `BAR_FILLED`/`BAR_EMPTY`/`BAR_WIDTH`, `assemble()`                                                                                                        |
| `tests/integration.rs` | End-to-end tests spawning the compiled binary with fixture JSON                                                                                                                 |

### Key Design Decisions

- **Single git call**: `git status --porcelain=v2 --branch --untracked-files=all` provides branch and per-type file status (M/A/D/?) in one subprocess. Requires git 2.11+ (Dec 2016).
- **Worktree count**: A second `git worktree list` call runs after the status call. Hidden when count is 1 (no extra worktrees).
- **PR status**: `build_pr()` shells out to `gh pr list` with a 30-second `/tmp` cache to avoid repeated API calls. Disabled by default; enable with `pr_status = true`.
- **Config via binary-adjacent TOML**: `config.rs` loads `statusline.toml` from the directory containing the running binary (`~/.claude/statusline.toml`). Falls back to defaults silently on missing file; logs warning on parse error.
- **`--print-defaults`**: Prints commented TOML defaults to stdout. Used by installers to seed `statusline.toml`.
- **`--configure-settings <settings_path> <command_path>`**: Reads `settings.json`, backs it up, merges the `statusLine` key, and writes atomically. Used by installers instead of external JSON tooling.
- **No runtime overhead from i18n**: Messages are compiled-in static string slices in `config.rs`.

## Configuration

`~/.claude/statusline.toml` (generated by `--print-defaults`):

```toml
# cost = true               # show cost tracker [true|false]
# messages = false          # show context messages [true|false]
# messages_language = "en"  # message language ["en"|"pt"|"es"]
# usage_bar_style = "plain" # usage bar style ["plain"|"rainbow"|"gradient"|"gsd"]
# pr_status = false         # show open PR for current branch [true|false] (requires gh CLI)
```

All fields are optional. Shown values are defaults.

**Context window display:** The progress bar is scaled against the _usable_ portion of the context window. Claude Code reserves ~16.5% as an autocompact buffer; the bar reaches 100% when autocompact triggers, not when the raw window is exhausted. Formula: `used = round((1 - max(0, remaining - 16.5) / 83.5) * 100)`.

**Blink at Critical (all bar styles):** At ≥86% the fire emoji blinks via ANSI SGR 5 (`\x1b[5m`). At ≥96% the skull emoji blinks as well (same SGR 5 codes). Both apply regardless of `usage_bar_style`. This works in iTerm2, macOS Terminal, and kitty. **Ghostty does not render SGR 5 text blink** by design — the emoji appears without blinking. This is a known Ghostty limitation ([discussion #4258](https://github.com/ghostty-org/ghostty/discussions/4258)), not a bug in this binary.

## Security

**`validate_directory()`** in `src/input.rs`:

- Requires: absolute paths (must start with `/`)
- Rejects: `..` traversal, `~` prefix, shell metacharacters (`$`, backtick, `;`), null bytes, relative paths
- Used on: `workspace.current_dir` from JSON before passing to `git` subprocess

## Testing Strategy

### Unit Tests (`src/*.rs` with `#[cfg(test)]`)

Each module has inline tests. Run with `cargo test --bins`.

- `src/components.rs`: tests for each builder function (model, directory, git, files breakdown, worktrees, context, cost, PR display, progress bar, number formatting)
- `src/git.rs`: `parse_porcelain_v2()` tests (clean, dirty with M/A/D/? breakdown, detached HEAD, no upstream, empty)
- `src/input.rs`: JSON parsing tests, `validate_directory()` security tests
- `src/render.rs`: `assemble()` tests (separator joining, empty part filtering)
- `src/config.rs`: `print_defaults()` completeness test
- `src/configure.rs`: `run()` tests (create, merge, overwrite, backup, invalid JSON, non-object, output format, command path)

### Integration Tests (`tests/integration.rs`)

Spawns the compiled binary with fixture JSON via stdin, asserts on stdout. Uses `CARGO_BIN_EXE_statusline` to locate the binary. Run with `cargo test --test integration`.

Test fixtures in `tests/fixtures/`:

- `test-input.json`: minimal valid JSON payload
- `claude-input-real.json`: real payload captured from Claude Code

## Adding New Components

1. Add a builder function to `src/components.rs`:

```rust
pub fn build_new_component(input: &ClaudeInput) -> String {
    if condition {
        return String::new();
    }
    format!("{CYAN}{}{NC}", input.some_field)
}
```

2. Add it to `build_all()` in `src/components.rs`:

```rust
pub fn build_all(input: &ClaudeInput, git: &GitInfo, config: &Config) -> Vec<String> {
    let wave_time = if matches!(config.usage_bar_style, BarStyle::Rainbow) {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    } else {
        0
    };
    vec![
        build_directory(input),
        build_git(git),
        build_files(git),
        build_worktrees(git.worktrees),
        build_model(input),
        build_context(input, config, wave_time),
        build_cost(input.cost_usd, config),
        build_pr(&git.branch, config),
        build_new_component(input),  // add here
    ]
}
```

3. Add inline unit tests in the same file.

## Performance

- Binary startup: ~5ms
- Git operations: two subprocess calls (`git status`, `git worktree list`), ~10-50ms depending on repo size
- PR status: one `gh pr list` call per branch, cached in `/tmp` for 30 seconds
- Config loading: reads one file on startup, ~1ms

## Dependencies

| Crate                  | Version | Purpose                                    |
| ---------------------- | ------- | ------------------------------------------ |
| `serde` + `serde_json` | 1       | JSON parsing (stdin input + settings.json) |
| `toml`                 | 1       | TOML config file parsing                   |
| `anyhow`               | 1       | Error handling with context                |

## File Locations

```
/
├── Cargo.toml              # Rust project manifest
├── Cargo.lock              # Locked dependencies
├── rust-toolchain.toml     # Pinned Rust version (1.96.0)
├── src/
│   ├── main.rs             # Entry point
│   ├── configure.rs        # --configure-settings: settings.json merge
│   ├── input.rs            # JSON parsing + security validation
│   ├── config.rs           # TOML config + messages + enums
│   ├── git.rs              # Git status (porcelain v2)
│   ├── components.rs       # All component builders
│   └── render.rs           # ANSI constants + assembly
├── tests/
│   ├── integration.rs      # End-to-end binary tests
│   └── fixtures/
│       ├── test-input.json         # Minimal test fixture
│       └── claude-input-real.json  # Real Claude Code payload
├── install.sh              # macOS/Linux/WSL installer — downloads release binary from
│                           #   github.com/89jobrien/claude-code-statusline, writes settings.json
├── install.ps1             # Windows PowerShell installer — self-contained, no external deps,
│                           #   downloads .exe release binary and writes settings.json
├── statusline.toml.example # Example config (generated by --print-defaults)
└── assets/                 # Logo and demo images

After installation (~/.claude/):
├── statusline              # Deployed binary (macOS/Linux)
├── statusline.exe          # Deployed binary (Windows)
└── statusline.toml         # User configuration
```
