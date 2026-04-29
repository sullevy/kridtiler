# kridtiler

> 中文文档: [README.zh-CN.md](README.zh-CN.md)

A window-tiling CLI for **KDE Plasma 6 / Wayland**. No daemon, no hacks — just KWin's official Scripting D-Bus API. End-to-end latency: **14–22 ms** per call.

## Demo

https://github.com/user-attachments/assets/c65fa23d-68b8-4adc-9a1b-7250b2fd85a6

## Highlights

- **Three modes**: named presets (bind to shortcuts), `--show` drag-select popup, `--overlay` fullscreen grid.
- **Multi-monitor aware** — always tiles on the screen owning the focused window; respects panels/docks via `MaximizeArea`.
- **No focus stealing** — popup/overlay are pure mouse, preserving fcitx5 / IBus state in GTK/Qt apps.
- **Configurable** grid, colors, popup size, and user-defined presets via `~/.config/kridtiler/config.toml`.
- **No daemon** — single-shot CLI loads a templated KWin script over D-Bus and exits.

## Install

Requires Rust 1.75+, Plasma 6, KWin Wayland.

```bash
cargo build --release
install -Dm755 target/release/kridtiler ~/.local/bin/kridtiler

# optional: sample config
mkdir -p ~/.config/kridtiler
cp config.example.toml ~/.config/kridtiler/config.toml
```

## Quick Start

```bash
# Direct positioning (great for keybinds)
kridtiler half-left
kridtiler --tile 0 0 5 7 --cols 12 --rows 8

# Interactive
kridtiler --show              # small popup, drag-select rectangle
kridtiler --overlay 16 12     # fullscreen 16×12 grid

# Inspect
kridtiler --list-presets
kridtiler --list-monitors
kridtiler --help
```

### Built-in presets

`half-left` `half-right` `half-top` `half-bottom` `maximize` `center`
`top-left` `top-right` `bottom-left` `bottom-right` (`tl` `tr` `bl` `br`)
`thirds-left` `thirds-center` `thirds-right` `two-thirds-left` `two-thirds-right`

See [README.zh-CN.md](README.zh-CN.md) for the full reference (configuration, every flag, KWin internals, debugging tips, known limitations).

## How it works

```
kridtiler  ──D-Bus──>  org.kde.KWin /Scripting
   ▲                          │
   │   callDBus(TileResult)   │ workspace.activeWindow
   └──────────────────────────┘ .frameGeometry = {...}
```

The CLI fills a JS/QML template, drops it into `/tmp/kridtiler/`, calls `loadScript` / `loadDeclarativeScript`, and (in `--tile` mode) waits for the script's `callDBus` reply.

## License

See [LICENSE](LICENSE) if present, otherwise all rights reserved by the author.
