use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgAction, Parser};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use zbus::{connection, interface, proxy, Connection};

mod config;
mod preset;

const TILE_TEMPLATE: &str = include_str!("../../../kwin-script/tile.js.tmpl");
const OVERLAY_TEMPLATE: &str = include_str!("../../../kwin-script/overlay.qml.tmpl");
const POPUP_TEMPLATE: &str = include_str!("../../../kwin-script/popup.qml.tmpl");
const MONITORS_TEMPLATE: &str = include_str!("../../../kwin-script/monitors.js.tmpl");

#[proxy(
    interface = "org.kde.kwin.Scripting",
    default_service = "org.kde.KWin",
    default_path = "/Scripting"
)]
trait Scripting {
    #[zbus(name = "loadScript")]
    fn load_script(&self, file_path: &str, plugin_name: &str) -> zbus::Result<i32>;
    #[zbus(name = "loadDeclarativeScript")]
    fn load_declarative_script(&self, file_path: &str, plugin_name: &str) -> zbus::Result<i32>;
    #[zbus(name = "start")]
    fn start(&self) -> zbus::Result<()>;
    #[zbus(name = "unloadScript")]
    fn unload_script(&self, plugin_name: &str) -> zbus::Result<bool>;
    #[zbus(name = "isScriptLoaded")]
    fn is_script_loaded(&self, plugin_name: &str) -> zbus::Result<bool>;
}

#[derive(Parser, Debug)]
#[command(
    name = "kridtiler",
    version,
    about = "KWin-driven window tiler — Phase 0 (--tile + presets)"
)]
struct Cli {
    /// Built-in preset name (e.g. half-left, maximize, center, top-right).
    /// Some presets accept a second positional arg, e.g. `center 70` =
    /// 70% × 70% centered. Run `kridtiler --list-presets` to see all.
    #[arg(value_names = ["PRESET", "PARAM"], num_args = 1..=2, conflicts_with = "tile")]
    preset: Option<Vec<String>>,

    /// Grid rect: x1 y1 x2 y2 (0-indexed, inclusive). Example: --tile 0 0 0 0 = top-left cell.
    #[arg(long, num_args = 4, value_names = ["X1", "Y1", "X2", "Y2"])]
    tile: Option<Vec<u32>>,

    /// Grid columns override. Default depends on preset; if neither --tile nor preset
    /// implies a grid, falls back to 2.
    #[arg(long)]
    cols: Option<u32>,

    /// Grid rows override.
    #[arg(long)]
    rows: Option<u32>,

    /// List all built-in presets and exit.
    #[arg(long, action = ArgAction::SetTrue)]
    list_presets: bool,

    /// List all connected monitors (name, geometry, active flag) and exit.
    /// Useful for picking a value for --monitor.
    #[arg(long, action = ArgAction::SetTrue)]
    list_monitors: bool,

    /// Log level: error|warn|info|debug|trace. Honors RUST_LOG if unset.
    #[arg(long)]
    log: Option<String>,

    /// How long to wait for the KWin script to call back (ms). 0 = fire-and-forget.
    #[arg(long, default_value_t = 800)]
    wait_ms: u64,

    /// Show the small popup picker (gTile-style). Optionally takes COLS [ROWS]
    /// after the flag (e.g. `--show 6 4`). Defaults: 6x4.
    #[arg(long, num_args = 0..=2, value_names = ["COLS", "ROWS"],
          conflicts_with_all = ["preset", "tile", "list_presets", "overlay", "popup"])]
    show: Option<Vec<u32>>,

    /// Show the full-screen interactive grid overlay. Optionally takes COLS [ROWS]
    /// after the flag (e.g. `--overlay 12 8`). Defaults: 8x6.
    #[arg(long, num_args = 0..=2, value_names = ["COLS", "ROWS"],
          conflicts_with_all = ["preset", "tile", "list_presets", "show", "popup"])]
    overlay: Option<Vec<u32>>,

    /// Backward-compat alias for --show. Prefer --show.
    #[arg(long, num_args = 0..=2, value_names = ["COLS", "ROWS"], hide = true,
          conflicts_with_all = ["preset", "tile", "list_presets", "show", "overlay"])]
    popup: Option<Vec<u32>>,

    /// Where the popup appears: "center" (active monitor) or "cursor". Overrides
    /// `[popup] anchor = ...` from config. Has no effect on --overlay.
    #[arg(long, value_name = "WHERE", value_parser = ["center", "cursor"])]
    at: Option<String>,

    /// Skip the D-Bus result listener entirely (useful when iterating).
    #[arg(long, action = ArgAction::SetTrue)]
    no_wait: bool,
}

fn init_logging(level: Option<&str>) {
    let filter = match level {
        Some(l) => tracing_subscriber::EnvFilter::try_new(l)
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        None => tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

/// Resolve CLI args into a final (cols, rows, rect). Explicit --cols/--rows always win
/// over preset defaults so users can override grid resolution per call.
fn resolve_command(cli: &Cli, cfg: &config::Config) -> Result<(u32, u32, [u32; 4])> {
    let (mut cols, mut rows, rect) = match (&cli.preset, &cli.tile) {
        (Some(args), _) => {
            let name = args.first().map(|s| s.as_str())
                .ok_or_else(|| anyhow!("internal: preset args empty"))?;
            let param = args.get(1);

            // Special: `center N` → centered rect of N% × N%. Uses a fine 1000x1000
            // grid so any integer percentage maps exactly.
            if name == "center" && param.is_some() {
                let pct: u32 = param.unwrap().parse()
                    .map_err(|_| anyhow!("center param must be an integer percent (10..95), got '{}'", param.unwrap()))?;
                if !(10..=95).contains(&pct) {
                    bail!("center param must be in 10..=95, got {pct}");
                }
                let span = pct * 10;            // cells out of 1000
                let pad = (1000 - span) / 2;
                let x1 = pad;
                let x2 = pad + span - 1;
                (1000_u32, 1000_u32, [x1, x1, x2, x2])
            } else {
                if let Some(p) = param {
                    bail!("preset '{name}' does not accept a parameter (got '{p}')");
                }
                if let Some(p) = cfg.presets.get(name) {
                    (p.cols, p.rows, p.rect)
                } else if let Some(p) = preset::lookup(name) {
                    (p.cols, p.rows, p.rect)
                } else {
                    let mut all = preset::names().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    all.extend(cfg.presets.keys().cloned());
                    bail!("unknown preset '{name}'. Try: {}", all.join(", "));
                }
            }
        }
        (None, Some(rect)) => {
            let r: [u32; 4] = rect
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("--tile needs exactly 4 ints"))?;
            // Default grid for raw --tile: prefer config, then PRD's example (2x1).
            let dc = cfg.general.default_cols.unwrap_or(2);
            let dr = cfg.general.default_rows.unwrap_or(1);
            (cli.cols.unwrap_or(dc), cli.rows.unwrap_or(dr), r)
        }
        (None, None) => bail!(
            "need a preset or --tile X1 Y1 X2 Y2 (try `kridtiler --list-presets`)"
        ),
    };

    if let Some(c) = cli.cols { cols = c; }
    if let Some(r) = cli.rows { rows = r; }

    if cols == 0 || rows == 0 {
        bail!("cols and rows must be positive");
    }
    let [x1, y1, x2, y2] = rect;
    if x2 < x1 || y2 < y1 {
        bail!("tile rect must satisfy x1<=x2 and y1<=y2");
    }
    if x2 >= cols || y2 >= rows {
        bail!("tile rect ({x1},{y1})..({x2},{y2}) out of grid {cols}x{rows}");
    }
    Ok((cols, rows, rect))
}

enum Mode {
    Tile { cols: u32, rows: u32, rect: [u32; 4] },
    Overlay { cols: u32, rows: u32 },
    Popup { cols: u32, rows: u32 },
}

struct Appearance {
    bg_color: String,
    bg_opacity: f32,
    cell_color: String,
    selection_color: String,
    anchor_color: String,
    border_color: String,
    popup_width: u32,
    popup_width_pct: f32,
}

impl Appearance {
    fn from_config(c: &config::Appearance) -> Self {
        Self {
            bg_color: c.background_color.clone().unwrap_or_else(|| "#1e1e2e".into()),
            bg_opacity: c.background_opacity.unwrap_or(0.95).clamp(0.0, 1.0),
            cell_color: c.cell_color.clone().unwrap_or_else(|| "#2a2a3a".into()),
            selection_color: c.selection_color.clone().unwrap_or_else(|| "#3a7ab0".into()),
            anchor_color: c.anchor_color.clone().unwrap_or_else(|| "#4d9de0".into()),
            border_color: c.border_color.clone().unwrap_or_else(|| "#4a4a6a".into()),
            popup_width: c.popup_width.unwrap_or(320),
            // Default 0 = use popup_width px instead. Clamp to a sane fraction.
            popup_width_pct: c.popup_width_pct.unwrap_or(0.0).clamp(0.0, 0.95),
        }
    }
    fn apply(&self, mut s: String) -> String {
        s = s.replace("__BG_COLOR__", &self.bg_color);
        s = s.replace("__BG_OPACITY__", &self.bg_opacity.to_string());
        s = s.replace("__CELL_COLOR__", &self.cell_color);
        s = s.replace("__SEL_COLOR__", &self.selection_color);
        s = s.replace("__ANCHOR_COLOR__", &self.anchor_color);
        s = s.replace("__BORDER_COLOR__", &self.border_color);
        s = s.replace("__POPUP_WIDTH__", &self.popup_width.to_string());
        s = s.replace("__POPUP_WIDTH_PCT__", &self.popup_width_pct.to_string());
        s
    }
}

fn render_tile(cols: u32, rows: u32, rect: [u32; 4], req_id: &str) -> String {
    TILE_TEMPLATE
        .replace("__COLS__", &cols.to_string())
        .replace("__ROWS__", &rows.to_string())
        .replace("__X1__", &rect[0].to_string())
        .replace("__Y1__", &rect[1].to_string())
        .replace("__X2__", &rect[2].to_string())
        .replace("__Y2__", &rect[3].to_string())
        .replace("__REQ_ID__", req_id)
}

fn render_overlay(
    cols: u32, rows: u32, req_id: &str, plugin: &str, result_path: &str, ap: &Appearance,
) -> String {
    let s = OVERLAY_TEMPLATE
        .replace("__COLS__", &cols.to_string())
        .replace("__ROWS__", &rows.to_string())
        .replace("__REQ_ID__", req_id)
        .replace("__PLUGIN__", plugin)
        .replace("__RESULT_PATH__", result_path);
    ap.apply(s)
}

fn render_popup(
    cols: u32, rows: u32, req_id: &str, plugin: &str,
    anchor: &str, grab_focus: bool, ap: &Appearance,
) -> String {
    let s = POPUP_TEMPLATE
        .replace("__COLS__", &cols.to_string())
        .replace("__ROWS__", &rows.to_string())
        .replace("__REQ_ID__", req_id)
        .replace("__PLUGIN__", plugin)
        .replace("__ANCHOR__", anchor)
        .replace("__GRAB_FOCUS__", if grab_focus { "true" } else { "false" });
    ap.apply(s)
}

fn write_temp(contents: &str, req_id: &str, ext: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("kridtiler");
    std::fs::create_dir_all(&dir).context("create temp dir")?;
    let prefix = if ext == "qml" { "ui" } else { "tile" };
    let path = dir.join(format!("{}-{}.{}", prefix, req_id, ext));
    std::fs::write(&path, contents).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Listen for the script's TileResult method-call on our owned bus name.
/// Returns the (status, payload-json) once a matching req_id arrives, or None on timeout.
async fn await_result(
    inbox: Arc<Mutex<ResultInbox>>,
    req_id: String,
    timeout: Duration,
) -> Result<Option<(String, String)>> {
    let deadline = Instant::now() + timeout;
    loop {
        {
            let mut guard = inbox.lock().await;
            if let Some(v) = guard.take_match(&req_id) {
                return Ok(Some(v));
            }
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Poll a file path until it's readable + parseable, or timeout. Reserved for a
/// future IPC mode (declarative QML scripts can't talk D-Bus); not wired in
/// right now since Qt blocks XHR PUT to local file:// URLs by default.
#[allow(dead_code)]
async fn await_file_result(
    path: PathBuf,
    timeout: Duration,
) -> Result<Option<(String, String)>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(bytes) = std::fs::read(&path) {
            if !bytes.is_empty() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let status = v
                        .get("status")
                        .and_then(|x| x.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let payload = v
                        .get("payload")
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "{}".to_string());
                    return Ok(Some((status, payload)));
                }
            }
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Buffer for results that may arrive before await_result polls.
#[derive(Default)]
struct ResultInbox {
    results: Vec<(String, String, String)>, // (req_id, status, payload)
}

impl ResultInbox {
    fn push(&mut self, req_id: String, status: String, payload: String) {
        self.results.push((req_id, status, payload));
    }
    fn take_match(&mut self, req_id: &str) -> Option<(String, String)> {
        if let Some(idx) = self.results.iter().position(|(id, _, _)| id == req_id) {
            let (_, s, p) = self.results.swap_remove(idx);
            Some((s, p))
        } else {
            None
        }
    }
}

/// D-Bus interface served at /org/kridtile/Control under bus name org.kridtile.Control.
/// The KWin script calls TileResult via callDBus when finished.
struct ControlIface {
    inbox: Arc<Mutex<ResultInbox>>,
}

#[interface(name = "org.kridtile.Control")]
impl ControlIface {
    async fn tile_result(&self, req_id: String, status: String, payload: String) {
        debug!(req_id, status, "got TileResult");
        self.inbox.lock().await.push(req_id, status, payload);
    }
}

/// Enumerate connected monitors via a one-shot KWin JS script that calls back
/// over org.kridtile.Control. Used by `--list-monitors`.
async fn list_monitors() -> Result<()> {
    let req_id = uuid::Uuid::new_v4().simple().to_string();
    let plugin = format!("kridtiler-monitors-{}", req_id);
    let body = MONITORS_TEMPLATE.replace("__REQ_ID__", &req_id);
    let path = write_temp(&body, &req_id, "js")?;
    let path_str = path.to_str().ok_or_else(|| anyhow!("non-utf8 path"))?;

    let inbox: Arc<Mutex<ResultInbox>> = Arc::new(Mutex::new(ResultInbox::default()));
    let _service = match connection::Builder::session()?
        .name("org.kridtile.Control")?
        .serve_at("/org/kridtile/Control", ControlIface { inbox: inbox.clone() })?
        .build()
        .await
    {
        Ok(c) => c,
        Err(e) => bail!("could not own org.kridtile.Control: {e}"),
    };

    let conn = Connection::session().await.context("session bus")?;
    let scripting = ScriptingProxy::new(&conn).await.context("Scripting proxy")?;
    let _id = scripting.load_script(path_str, &plugin).await.context("loadScript")?;
    scripting.start().await.context("start")?;

    let result = await_result(inbox, req_id, Duration::from_millis(800)).await?;
    let _ = scripting.unload_script(&plugin).await;
    let _ = std::fs::remove_file(&path);

    let (_status, payload) = result.ok_or_else(|| anyhow!("no response from KWin in 800ms"))?;
    let v: serde_json::Value = serde_json::from_str(&payload).context("parse monitors json")?;
    let monitors = v
        .get("monitors")
        .and_then(|m| m.as_array())
        .ok_or_else(|| anyhow!("missing monitors[] in payload"))?;

    println!("{:>3}  {:8}  {:>5}x{:<5}  {:>5},{:<5}  {:1}  {}",
             "idx", "name", "w", "h", "x", "y", "*", "model");
    for m in monitors {
        let idx     = m.get("index").and_then(|x| x.as_u64()).unwrap_or(0);
        let name    = m.get("name").and_then(|x| x.as_str()).unwrap_or("?");
        let active  = m.get("active").and_then(|x| x.as_bool()).unwrap_or(false);
        let model   = m.get("model").and_then(|x| x.as_str()).unwrap_or("");
        let mfr     = m.get("manufacturer").and_then(|x| x.as_str()).unwrap_or("");
        let g = m.get("geometry").cloned().unwrap_or(serde_json::json!({}));
        let gx = g.get("x").and_then(|x| x.as_i64()).unwrap_or(0);
        let gy = g.get("y").and_then(|x| x.as_i64()).unwrap_or(0);
        let gw = g.get("width").and_then(|x| x.as_i64()).unwrap_or(0);
        let gh = g.get("height").and_then(|x| x.as_i64()).unwrap_or(0);
        let mark = if active { "*" } else { " " };
        let label = if !mfr.is_empty() && !model.is_empty() {
            format!("{} {}", mfr, model)
        } else {
            model.to_string()
        };
        println!("{:>3}  {:8}  {:>5}x{:<5}  {:>5},{:<5}  {:1}  {}",
                 idx, name, gw, gh, gx, gy, mark, label);
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Restore default SIGPIPE so `kridtiler --list-presets | head` doesn't
    // panic — Rust installs an "ignore" handler that turns broken pipes into
    // EPIPE write errors, which `println!` then panics on.
    #[cfg(unix)]
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }

    let cli = Cli::parse();
    let cfg = config::load();
    // Log level: --log > config.general.log_level > RUST_LOG > info.
    let log_level = cli.log.clone().or_else(|| cfg.general.log_level.clone());
    init_logging(log_level.as_deref());

    if cli.list_presets {
        for n in preset::names() {
            let p = preset::lookup(n).unwrap();
            println!(
                "{:20}  grid {}x{}  rect ({},{})..({},{})  [built-in]",
                n, p.cols, p.rows, p.rect[0], p.rect[1], p.rect[2], p.rect[3]
            );
        }
        for (n, p) in &cfg.presets {
            println!(
                "{:20}  grid {}x{}  rect ({},{})..({},{})  [user]",
                n, p.cols, p.rows, p.rect[0], p.rect[1], p.rect[2], p.rect[3]
            );
        }
        return Ok(());
    }

    if cli.list_monitors {
        return list_monitors().await;
    }

    let appearance = Appearance::from_config(&cfg.appearance);

    // Resolve interactive-mode grid args: positional COLS [ROWS] after the flag
    // beat --cols/--rows, which beat config defaults, which beat built-in defaults.
    fn pick_grid(
        args: &Option<Vec<u32>>,
        flag_cols: Option<u32>, flag_rows: Option<u32>,
        cfg_cols:  Option<u32>, cfg_rows:  Option<u32>,
        default_cols: u32, default_rows: u32,
    ) -> (u32, u32) {
        let mut c = cfg_cols.unwrap_or(default_cols);
        let mut r = cfg_rows.unwrap_or(default_rows);
        if let Some(v) = args {
            if let Some(&x) = v.first() { c = x; }
            if let Some(&x) = v.get(1) { r = x; }
        }
        if let Some(x) = flag_cols { c = x; }
        if let Some(x) = flag_rows { r = x; }
        (c.max(1), r.max(1))
    }

    let mode = if let Some(args) = cli.overlay.clone() {
        let (cols, rows) = pick_grid(
            &Some(args), cli.cols, cli.rows,
            cfg.overlay.cols, cfg.overlay.rows, 8, 6,
        );
        Mode::Overlay { cols, rows }
    } else if let Some(args) = cli.show.clone().or_else(|| cli.popup.clone()) {
        let (cols, rows) = pick_grid(
            &Some(args), cli.cols, cli.rows,
            cfg.popup.cols, cfg.popup.rows, 6, 4,
        );
        Mode::Popup { cols, rows }
    } else {
        let (cols, rows, rect) = resolve_command(&cli, &cfg)?;
        Mode::Tile { cols, rows, rect }
    };

    let req_id = uuid::Uuid::new_v4().simple().to_string();
    // Interactive modes use fixed plugin names so a re-invocation auto-replaces
    // any stuck previous instance. Tile mode keeps unique names so concurrent
    // calls coexist.
    let plugin_name = match &mode {
        Mode::Tile { .. } => format!("kridtiler-{}", req_id),
        Mode::Overlay { .. } => "kridtiler-overlay".to_string(),
        Mode::Popup { .. } => "kridtiler-popup".to_string(),
    };
    let result_path = std::env::temp_dir()
        .join("kridtiler")
        .join(format!("result-{}.json", req_id));

    let (script_path, declarative) = match &mode {
        Mode::Tile { cols, rows, rect } => {
            let body = render_tile(*cols, *rows, *rect, &req_id);
            (write_temp(&body, &req_id, "js")?, false)
        }
        Mode::Overlay { cols, rows } => {
            std::fs::create_dir_all(result_path.parent().unwrap()).ok();
            let _ = std::fs::remove_file(&result_path);
            let result_str = result_path
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 result path"))?;
            let body = render_overlay(*cols, *rows, &req_id, &plugin_name, result_str, &appearance);
            (write_temp(&body, &req_id, "qml")?, true)
        }
        Mode::Popup { cols, rows } => {
            // CLI --at > config [popup].anchor > "center".
            let anchor = cli.at.clone()
                .or_else(|| cfg.popup.anchor.clone())
                .unwrap_or_else(|| "center".to_string());
            let anchor = match anchor.as_str() {
                "cursor" => "cursor",
                _ => "center",
            };
            // Default: don't grab focus — protects fcitx5/IBus text-input on
            // GTK/Qt apps. Users opt-in via config when they need Esc/Enter.
            let grab_focus = cfg.popup.grab_focus.unwrap_or(false);
            let body = render_popup(*cols, *rows, &req_id, &plugin_name, anchor, grab_focus, &appearance);
            (write_temp(&body, &req_id, "qml")?, true)
        }
    };
    debug!(?script_path, plugin_name, declarative, "wrote temp script");

    let t0 = Instant::now();

    // Stand up our service first so the script's callDBus has somewhere to land.
    // We always claim the name (cheap), even with --no-wait, so KWin doesn't log
    // "name not activatable" noise.
    let inbox: Arc<Mutex<ResultInbox>> = Arc::new(Mutex::new(ResultInbox::default()));
    let want_callback = !cli.no_wait && cli.wait_ms > 0;
    // Interactive modes need much more headroom — user interaction can take 30s+.
    // Tile mode keeps the snappy default unless the user explicitly raises --wait-ms.
    let effective_wait_ms = match &mode {
        Mode::Overlay { .. } | Mode::Popup { .. } => cli.wait_ms.max(60_000),
        Mode::Tile { .. } => cli.wait_ms,
    };
    let service_conn: Option<Connection> = match connection::Builder::session()?
        .name("org.kridtile.Control")?
        .serve_at(
            "/org/kridtile/Control",
            ControlIface {
                inbox: inbox.clone(),
            },
        )?
        .build()
        .await
    {
        Ok(c) => Some(c),
        Err(e) => {
            // Most likely another kridtiler invocation is in flight. Not fatal —
            // the script still runs, we just can't observe its result.
            if want_callback {
                warn!("could not own org.kridtile.Control ({e}); proceeding without callback");
            }
            None
        }
    };

    let conn = Connection::session().await.context("connect session bus")?;
    let scripting = ScriptingProxy::new(&conn).await.context("Scripting proxy")?;

    // For interactive modes (Show / Popup): kick out any stale instance first
    // so we can reuse the fixed plugin name. unloadScript on a not-loaded plugin
    // is a no-op.
    if !matches!(mode, Mode::Tile { .. }) {
        let _ = scripting.unload_script(&plugin_name).await;
    }

    let waiter = if !want_callback {
        None
    } else {
        match &mode {
            Mode::Tile { .. } if service_conn.is_some() => {
                let inbox2 = inbox.clone();
                let req_id2 = req_id.clone();
                let dur = Duration::from_millis(effective_wait_ms);
                Some(tokio::spawn(
                    async move { await_result(inbox2, req_id2, dur).await },
                ))
            }
            // Show / Popup have no working callback channel today; rely on visual feedback.
            _ => None,
        }
    };

    let path_str = script_path
        .to_str()
        .ok_or_else(|| anyhow!("non-utf8 temp script path"))?;
    let id = if declarative {
        scripting
            .load_declarative_script(path_str, &plugin_name)
            .await
            .context("loadDeclarativeScript")?
    } else {
        scripting
            .load_script(path_str, &plugin_name)
            .await
            .context("loadScript")?
    };
    debug!(id, "loadScript ok");
    scripting.start().await.context("start")?;
    debug!(elapsed_ms = t0.elapsed().as_millis() as u64, "start ok");

    let outcome = if let Some(handle) = waiter {
        match handle.await {
            Ok(Ok(Some((status, payload)))) => Some((status, payload)),
            Ok(Ok(None)) => {
                warn!("no result signal within {}ms", effective_wait_ms);
                None
            }
            Ok(Err(e)) => {
                warn!("result listener error: {e}");
                None
            }
            Err(e) => {
                warn!("listener task join error: {e}");
                None
            }
        }
    } else {
        None
    };

    // Best-effort cleanup. Tile mode unloads after we got the result; Show mode
    // leaves the overlay loaded — the user is still interacting with it. The
    // next --show call (or an explicit kridtiler --cleanup) will remove it.
    if matches!(mode, Mode::Tile { .. }) {
        if let Err(e) = scripting.unload_script(&plugin_name).await {
            warn!("unloadScript({plugin_name}): {e}");
        }
        let _ = std::fs::remove_file(&script_path);
    }
    let _ = std::fs::remove_file(&result_path);
    drop(service_conn);

    let total_ms = t0.elapsed().as_millis() as u64;
    match outcome {
        Some((status, payload)) => {
            info!(
                "result: status={} elapsed_ms={} payload={}",
                status, total_ms, payload
            );
            if status == "ok" {
                Ok(())
            } else {
                bail!("script reported status={status}: {payload}");
            }
        }
        None => {
            info!("dispatched (no callback) elapsed_ms={total_ms}");
            Ok(())
        }
    }
}
