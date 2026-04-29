use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, warn};

/// User config (~/.config/kridtiler/config.toml). All sections are optional —
/// missing fields fall back to built-in defaults.
///
/// Example:
/// ```toml
/// [general]
/// log_level = "info"
///
/// [overlay]
/// cols = 12
/// rows = 8
///
/// [popup]
/// cols = 8
/// rows = 6
///
/// [appearance]
/// background_color = "#1e1e2e"
/// background_opacity = 0.92
/// selection_color = "#4d9de0"
/// border_color = "#4a4a6a"
///
/// [presets.work-left]
/// cols = 12
/// rows = 1
/// rect = [0, 0, 7, 0]   # left 8/12 of screen
/// ```
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Popup {
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    /// "center" (default) — popup at active monitor center
    /// "cursor" — popup follows the mouse cursor
    pub anchor: Option<String>,
    /// Whether the popup grabs keyboard focus when shown. Default false because
    /// taking focus breaks fcitx5/IBus text-input-v3 reattachment for GTK/Qt
    /// apps — typing into the captured window after dismiss fails. Set true
    /// only if you want Esc/Enter to work and don't use IME.
    pub grab_focus: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub overlay: Grid,
    pub popup: Popup,
    pub appearance: Appearance,
    #[serde(default)]
    pub presets: HashMap<String, PresetEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct General {
    pub log_level: Option<String>,
    pub default_cols: Option<u32>,
    pub default_rows: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Grid {
    pub cols: Option<u32>,
    pub rows: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub background_color: Option<String>,
    pub background_opacity: Option<f32>,
    pub cell_color: Option<String>,
    pub selection_color: Option<String>,
    pub anchor_color: Option<String>,
    pub border_color: Option<String>,
    pub popup_width: Option<u32>,
    /// Popup width as a fraction of the active screen width (0.0..1.0). Wins
    /// over `popup_width` when set; useful for keeping the widget legible across
    /// HiDPI / multi-monitor setups.
    pub popup_width_pct: Option<f32>,
}

/// User-defined preset. Mirrors built-in `preset::Preset`.
#[derive(Debug, Deserialize)]
pub struct PresetEntry {
    pub cols: u32,
    pub rows: u32,
    pub rect: [u32; 4],
}

pub fn config_path() -> PathBuf {
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    xdg.join("kridtiler").join("config.toml")
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => match toml::from_str::<Config>(&s) {
            Ok(c) => {
                debug!(?path, "loaded config");
                c
            }
            Err(e) => {
                warn!("config {} parse error: {e}; using defaults", path.display());
                Config::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!(?path, "no config file; using built-in defaults");
            Config::default()
        }
        Err(e) => {
            warn!("config {} read error: {e}; using defaults", path.display());
            Config::default()
        }
    }
}

/// Load and parse a config from an explicit path — used for `--config` overrides
/// or by tests. Errors propagate (unlike `load()` which is best-effort).
#[allow(dead_code)]
pub fn load_from(path: &PathBuf) -> Result<Config> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&s).with_context(|| format!("parse {}", path.display()))
}
