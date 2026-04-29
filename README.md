# clipboard\_multiline\_fixer

A Windows system tray app that **auto-fixes wrapped terminal commands** when you copy and paste.

When a long command wraps to multiple lines in the terminal, copying it includes artificial line breaks that break the command. This app silently joins them back.

## Features

- **Auto-fix** — runs in the background, automatically fixes clipboard content on copy
- **Smart heuristics** — recognizes wrapped lines, `\`/`^`/`` ` `` continuations, `|`/`&&`/`||` pipelines, flag continuations (`--flag`), and 80+ common commands
- **Heredoc aware** — preserves heredoc blocks (`<< 'EOF'` ... `EOF`) without joining
- **Hotkey** — `Ctrl+Shift+Ins` fixes clipboard + simulates paste in one step
- **System tray** — Pause/Resume monitoring, Start with Windows toggle, Quit

## Usage

1. Run `clipboard_multiline_fixer.exe` (no console window in release mode)
2. It appears in the system tray as a green clipboard icon
3. Copy normally — clipboard is auto-fixed
4. Press `Ctrl+Shift+Ins` for fix + paste in one action

### Tray Menu

```
[Icon: clipboard_multiline_fixer]
─────────────────────────
⏸ Pause / ▶ Resume    (toggle — pauses auto-fix + hotkey)
🔁 Start with Windows  (toggle — HKCU\Run registry)
📄 Show Console        (toggle — opens debug console to see what's happening)
─────────────────────────
❌ Quit
```

## Build

```bash
cargo build --release
```

The binary will be at `target/release/clipboard_multiline_fixer.exe`.

## How It Works

| Heuristic | Example |
|---|---|
| Explicit continuation (`\`, `^`, `` ` ``) | `npm install \` → `npm install --save-dev` |
| Pipeline (`\|`, `&&`, `\|\|`) | `git log \|` → `git log \| grep fix` |
| Flag continuation | `cargo build` → `cargo build --release` |
| Terminal width (auto-detected) | long line wraps → rejoined |
| Next line starts with `\|`, `>`, `&&` | continuation prompt → joined |
| Inside heredoc `<< 'TOKEN'` | **never joined** — content preserved |

## Dependencies

- [winit](https://crates.io/crates/winit) — event loop
- [tray-icon](https://crates.io/crates/tray-icon) — system tray
- [global-hotkey](https://crates.io/crates/global-hotkey) — hotkey registration
- [arboard](https://crates.io/crates/arboard) — clipboard access
- [enigo](https://crates.io/crates/enigo) — keystroke simulation
- [winreg](https://crates.io/crates/winreg) — Windows registry for autostart
