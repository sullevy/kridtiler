# kridtiler

> **Popup grid-tiling for KDE Plasma 6 / Wayland — like [gTile] or [Divvy], but native to KWin.**

[gTile]: https://github.com/gTile/gTile
[Divvy]: https://mizage.com/divvy/

> 中文文档: [README.zh-CN.md](README.zh-CN.md)

A small CLI window-tiler for **KDE Plasma 6 / Wayland**. Inspired by **gTile** (GNOME) and **Divvy** (macOS): pop a grid, drag a rectangle, snap the active window. No daemon, no hacks — just KWin's official Scripting D-Bus API. End-to-end latency: **14–22 ms** per call.

## Demo

https://github.com/user-attachments/assets/c65fa23d-68b8-4adc-9a1b-7250b2fd85a6

## Highlights

- **Three modes** — named presets (bind to shortcuts), `--show` drag-select popup (gTile-style), `--overlay` fullscreen grid (Divvy-style).
- **Multi-monitor aware** — always tiles on the screen owning the focused window; respects panels/docks via `MaximizeArea`.
- **No focus stealing** — popup/overlay are mouse-only, preserving fcitx5 / IBus state in GTK/Qt apps.
- **Configurable** — grid, colors, popup size, and user-defined presets via `~/.config/kridtiler/config.toml`.
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

## CLI

### Modes

| Mode | When to use | Example |
|------|-------------|---------|
| **Preset / explicit coords** | You know the target slot — best for keybinds | `kridtiler half-left` |
| **`--show` popup** | Pick a slot ad-hoc without memorizing coords | `kridtiler --show` |
| **`--overlay` fullscreen** | Fine-grained partitioning on a large grid | `kridtiler --overlay 12 8` |

### Commands

```bash
# —— Direct positioning ——
kridtiler half-left                  # built-in preset (full list below)
kridtiler work-left                  # user-defined preset (from config)
kridtiler --tile 0 0 1 0             # left half of default 2×1 grid
kridtiler --tile 0 0 5 7 --cols 12 --rows 8

# —— Interactive ——
kridtiler --show                     # small popup, centered (config can switch to cursor)
kridtiler --show 10 6                # override grid to 10×6 for this call
kridtiler --show --at cursor         # follow the cursor for this call
kridtiler --overlay                  # fullscreen overlay
kridtiler --overlay 16 12            # fullscreen + 16×12 grid

# —— Utilities ——
kridtiler --list-presets             # list all presets (built-in + user)
kridtiler --list-monitors            # list monitors (name / geometry / active marker)
kridtiler --help
```

### Flags

| Flag | Meaning |
|------|---------|
| `<PRESET>` | Positional: preset name |
| `--tile X1 Y1 X2 Y2` | 0-indexed inclusive grid coordinates |
| `--cols N` / `--rows M` | Override grid resolution (beats config / preset) |
| `--show [COLS [ROWS]]` | Popup mode; optional grid override |
| `--overlay [COLS [ROWS]]` | Fullscreen overlay mode |
| `--at center\|cursor` | Popup placement (only affects `--show`) |
| `--log LEVEL` | `error` \| `warn` \| `info` \| `debug` \| `trace` |
| `--wait-ms N` | Reply timeout (default 800; auto-extended to 60s in interactive modes) |
| `--no-wait` | Fire-and-forget; do not wait for reply |
| `--list-presets` | List presets and exit |
| `--list-monitors` | List monitors and exit (`*` marks the active one) |

### Built-in presets

| Name | Grid | Description |
|------|------|-------------|
| `half-left` / `half-right` | 2×1 | Left / right half |
| `half-top` / `half-bottom` | 1×2 | Top / bottom half |
| `maximize` | 1×1 | Full screen (`MaximizeArea`, panels excluded) |
| `center` | 4×4 → inner 2×2 | Centered ~50% |
| `top-left` `top-right` `bottom-left` `bottom-right` | 2×2 | Quadrants (aliases: `tl` `tr` `bl` `br`) |
| `thirds-left` `thirds-center` `thirds-right` | 3×1 | Equal thirds |
| `two-thirds-left` `two-thirds-right` | 3×1 | 2/3 + 1/3 split |

## Configuration `~/.config/kridtiler/config.toml`

All sections are optional; missing fields fall back to built-in defaults. Precedence: **CLI flags > config > defaults**.

```toml
[general]
log_level     = "info"        # or debug / warn / error
default_cols  = 12            # used when --tile omits --cols
default_rows  = 1

[overlay]
cols = 12
rows = 8

[popup]
cols   = 8
rows   = 6
anchor = "center"             # "center" (default) | "cursor"

[appearance]
background_color   = "#1e1e2e"
background_opacity = 0.92
cell_color         = "#2a2a3a"
selection_color    = "#3a7ab0"   # fill while drag-selecting
anchor_color       = "#4d9de0"   # anchor cell / hover border
border_color       = "#4a4a6a"
popup_width        = 320          # px; used only if popup_width_pct is unset
popup_width_pct    = 0.30         # 30% of screen width (preferred over popup_width)

# User presets (override built-ins on name clash)
[presets.work-left]
cols = 12
rows = 1
rect = [0, 0, 7, 0]               # left 8/12 ≈ 67%

[presets.work-right]
cols = 12
rows = 1
rect = [8, 0, 11, 0]              # right 4/12 ≈ 33%
```

## Multi-monitor

- `workspace.activeWindow.output` decides the target screen — kridtiler always operates on **the monitor owning the focused window**.
- `clientArea(MaximizeArea, ...)` excludes panels/docks; the grid fills the remaining area.
- `--show` centered → screen center; cursor mode → wherever the cursor is (naturally lands on the right screen).

## Interactive popup behavior

Both interactive modes are **focus-preserving** and mouse-only — protects fcitx5 / IBus state in GTK/Qt apps.

**`--show` (small popup, gTile-style)**
- Mouse: **drag** to select a rectangle → release to apply; or **click anchor → click opposite corner** to apply.
- **Right-click** or click outside the popup = cancel.
- No keyboard by default. (`[popup] grab_focus = true` enables Esc/Enter but breaks IME.)

**`--overlay` (fullscreen)**
- Mouse: **left-drag** to select → release to apply.
- **Right-click** = cancel.
- No keyboard support (the global-shortcut cost outweighs the missing keybind).

> **Why no Esc/Enter?**
> Wayland `text-input-v3` on KWin 6 + GTK/Qt apps has a known bug: when the overlay grabs focus and then closes, fcitx5 / IBus loses its text-input association — the candidate window stays visible but **input does not commit**. kridtiler avoids this by never grabbing focus, at the cost of no keyboard interaction.

## Known limitations

- `--show` / `--overlay` have no D-Bus reply (KWin's declarative QML scripts don't expose `callDBus`, and Qt sandboxes XHR file writes). Feedback is visual + via `journalctl`; the CLI returns immediately.
- Re-invoking `--show` / `--overlay` automatically displaces any previous instance (fixed plugin names: `kridtiler-popup` / `kridtiler-overlay`).
- Cross-monitor flicker: previously seen with `OnScreenDisplay` type; greatly reduced after switching to `PopupMenu` and not forcing `Workspace.activeWindow`. Report any residual flicker.
- Tracks `activeWindow` only — cannot target an arbitrary window (Wayland has no window-picker API).

## Debugging

```bash
kridtiler --show --log debug              # CLI-side logs
journalctl --user --since "1 minute ago" | grep kridtiler   # KWin-side console.warn

# Run a KWin script manually (handy when iterating on QML):
qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.loadDeclarativeScript /path/to/foo.qml my-id
qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.start
qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.unloadScript my-id

# Generated scripts live here
ls /tmp/kridtiler/
```

## How it works

```
kridtiler  ──D-Bus──>  org.kde.KWin /Scripting
   ▲                          │
   │   callDBus(TileResult)   │ workspace.activeWindow
   └──────────────────────────┘ .frameGeometry = {...}
```

The CLI fills a JS/QML template, drops it into `/tmp/kridtiler/`, calls `loadScript` / `loadDeclarativeScript`, and (in `--tile` mode) waits for the script's `callDBus` reply. 100% on KWin's official API — no internal-state hacks.

## Project layout

```
kridtiler/
├── Cargo.toml                          # workspace
├── crates/kridtiler-cli/
│   └── src/
│       ├── main.rs                     # CLI entry, mode dispatch, D-Bus client
│       ├── preset.rs                   # built-in preset table
│       └── config.rs                   # ~/.config/kridtiler/config.toml parser
├── kwin-script/
│   ├── tile.js.tmpl                    # KWin JS template for --tile / presets
│   ├── overlay.qml.tmpl                # QML for --overlay
│   └── popup.qml.tmpl                  # QML for --show / gTile-style popup
├── config.example.toml
└── README.md
```

## Keywords

KDE, KDE Plasma 6, KWin, Wayland, window tiling, window manager, gTile alternative, Divvy alternative, grid tiling, popup tiler, keyboard shortcuts, Rust CLI.
