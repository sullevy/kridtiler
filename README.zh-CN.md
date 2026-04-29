# kridtiler

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

> English: [README.md](README.md)

KDE Plasma 6 / Wayland 窗口平铺 CLI。基于 KWin Scripting D-Bus 接口,无需常驻守护进程,单次调用端到端 14–22 ms。

## 演示

https://github.com/user-attachments/assets/c65fa23d-68b8-4adc-9a1b-7250b2fd85a6

## 工作原理

```
┌─────────────┐   loadScript    ┌──────────────────┐
│  kridtiler  │ ──D-Bus──────>  │ KWin (org.kde.   │
│  (Rust CLI) │                 │  KWin /Scripting)│
└─────────────┘                 └────────┬─────────┘
       ▲                                  │ workspace.activeWindow
       │  callDBus(TileResult)            │ .frameGeometry = {...}
       └──────────────────────────────────┘
```

- CLI 把 JS/QML 模板替换参数后写到 `/tmp/kridtiler/`
- 通过 `org.kde.kwin.Scripting.loadScript` / `loadDeclarativeScript` 让 KWin 加载执行
- 脚本读 `workspace.activeWindow`,改 `frameGeometry`,完成后调 `callDBus` 回传(仅 `--tile` 模式)
- 100% 走 KWin 官方 API,不 hack 内部状态

## 安装

依赖 Rust 1.75+、Plasma 6 / KWin Wayland。

```bash
cargo build --release
# 二进制位于 target/release/kridtiler — 复制到 PATH:
install -Dm755 target/release/kridtiler ~/.local/bin/kridtiler
```

可选示例配置:
```bash
mkdir -p ~/.config/kridtiler
cp config.example.toml ~/.config/kridtiler/config.toml
```

## CLI 用法

### 三类模式

| 模式 | 何时用 | 示例 |
|------|--------|------|
| **预设 / 直接坐标** | 知道目标位置,绑快捷键最适合 | `kridtiler half-left` |
| **`--show` 弹窗** | 临时挑位置,不想记网格坐标 | `kridtiler --show` |
| **`--overlay` 全屏覆盖** | 大网格精细划区,拖选直观 | `kridtiler --overlay 12 8` |

### 命令清单

```bash
# —— 直接定位 ——
kridtiler half-left                  # 内置预设(完整列表见下)
kridtiler work-left                  # 用户自定义预设(来自 config)
kridtiler --tile 0 0 1 0             # 默认 2×1 网格的左格
kridtiler --tile 0 0 5 7 --cols 12 --rows 8

# —— 交互式选择 ——
kridtiler --show                     # 小弹窗,默认居中(配置可改光标)
kridtiler --show 10 6                # 临时改成 10×6 网格
kridtiler --show --at cursor         # 临时改成光标跟随
kridtiler --overlay                  # 全屏 overlay
kridtiler --overlay 16 12            # 全屏 + 16×12 大网格

# —— 工具 ——
kridtiler --list-presets             # 列出所有预设(内置+用户)
kridtiler --list-monitors            # 列出所有显示器(name/几何/活跃标记)
kridtiler --help
```

### 完整参数

| 参数 | 含义 |
|------|------|
| `<PRESET>` | 位置参数:预设名 |
| `--tile X1 Y1 X2 Y2` | 0-indexed 闭区间网格坐标 |
| `--cols N` / `--rows M` | 临时改网格精度(覆盖配置/预设) |
| `--show [COLS [ROWS]]` | 弹窗模式,可选位置参数指定网格 |
| `--overlay [COLS [ROWS]]` | 全屏 overlay 模式 |
| `--at center\|cursor` | 弹窗位置,只对 `--show` 生效 |
| `--log LEVEL` | error\|warn\|info\|debug\|trace |
| `--wait-ms N` | 等回调超时(默认 800;交互模式自动放宽到 60s) |
| `--no-wait` | 不等回调,fire-and-forget |
| `--list-presets` | 列预设并退出 |
| `--list-monitors` | 列显示器并退出(`*` 标记当前活跃屏) |

### 内置预设

| 名字 | 网格 | 说明 |
|------|------|------|
| `half-left` / `half-right` | 2×1 | 左/右半屏 |
| `half-top` / `half-bottom` | 1×2 | 上/下半屏 |
| `maximize` | 1×1 | 满屏(=`MaximizeArea`,扣除 panel) |
| `center` | 4×4 → 内 2×2 | 中央 ~50% |
| `top-left` `top-right` `bottom-left` `bottom-right` | 2×2 | 四象限(简写 `tl/tr/bl/br`) |
| `thirds-left` `thirds-center` `thirds-right` | 3×1 | 三等分 |
| `two-thirds-left` `two-thirds-right` | 3×1 | 2/3 + 1/3 |

## 配置文件 `~/.config/kridtiler/config.toml`

所有 section 可选,缺失字段走内置默认。CLI 参数 > 配置 > 内置默认。

```toml
[general]
log_level = "info"           # 或 debug/warn/error
default_cols = 12            # --tile 没指定 --cols 时的默认
default_rows = 1

[overlay]
cols = 12
rows = 8

[popup]
cols = 8
rows = 6
anchor = "center"            # "center"(默认) | "cursor"

[appearance]
background_color   = "#1e1e2e"
background_opacity = 0.92
cell_color         = "#2a2a3a"
selection_color    = "#3a7ab0"   # 拖选过程中的填充
anchor_color       = "#4d9de0"   # anchor 格 / hover 边框
border_color       = "#4a4a6a"
popup_width        = 320          # px,popup_width_pct 未设时生效
popup_width_pct    = 0.30         # 屏幕宽度的 30%(优先于 popup_width)

# 用户自定义预设(同名时覆盖内置)
[presets.work-left]
cols = 12
rows = 1
rect = [0, 0, 7, 0]              # 左侧 8/12 ≈ 67%

[presets.work-right]
cols = 12
rows = 1
rect = [8, 0, 11, 0]              # 右侧 4/12 ≈ 33%
```

## 多显示器

- `workspace.activeWindow.output` 决定平铺哪块屏幕 → 始终在**当前焦点窗口所在的显示器**操作
- `clientArea(MaximizeArea, ...)` 自动扣除该屏的 panel/dock,网格按可用区铺开
- `--show` 居中模式落点 = 当前屏中心;cursor 模式跟光标(自然落在光标所在屏)

## 交互式弹窗用法

两种模式默认都**不抢焦点**,纯鼠标交互,保护输入法(fcitx5 / IBus)在 GTK/Qt 应用上的状态。

**`--show`(小弹窗 / gTile 风格)**
- 鼠标:**按住拖选**矩形 → 松开应用;或**点 anchor → 点对角**应用
- **右键** / 点弹窗外的空白区域 = 取消
- 默认无键盘(配置 `[popup] grab_focus = true` 可启用 Esc/Enter,但会破坏 IME)

**`--overlay`(全屏覆盖)**
- 鼠标:**左键拖选** → 松开应用
- **右键** = 取消
- 无键盘支持(全局快捷键代价比键盘缺失更大)

> **为什么没有 Esc/Enter?**  
> Wayland `text-input-v3` 协议在 KWin 6 + GTK/Qt 应用上有已知问题:overlay 抢焦点 → 关闭 → 焦点回到原窗口时,fcitx5/IBus 的 text-input 关联无法正确恢复,候选框还在但**无法上屏**。kridtiler 选择**完全不抢焦点**来规避这个 bug,代价是没有键盘交互。

## 已知限制

- `--show` / `--overlay` 没有 D-Bus 回调(KWin 的 declarative QML 脚本不暴露 `callDBus`,Qt 也默认禁 XHR 写本地文件)。**通过视觉 + journal 日志反馈**;CLI 立即返回。
- 第二次调用 `--show` / `--overlay` 会自动顶掉上一次未关闭的同类弹窗(用固定插件名 `kridtiler-popup` / `kridtiler-overlay`)。
- 跨屏闪烁:之前用 OnScreenDisplay 类型时存在,改成 PopupMenu + 不强制 `Workspace.activeWindow` 后已大幅缓解。仍有残留请反馈。
- 只跟踪 `activeWindow`,不能指定其他窗口(Wayland 无窗口选择 API)。

## 调试

```bash
kridtiler --show --log debug              # CLI 端日志
journalctl --user --since "1 minute ago" | grep kridtiler   # KWin 端 console.warn 输出

# 单跑 KWin 脚本(调试 QML 改动时):
qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.loadDeclarativeScript /path/to/foo.qml my-id
qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.start
qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.unloadScript my-id

# 临时文件位置
ls /tmp/kridtiler/
```

## 项目结构

```
kridtiler/
├── Cargo.toml                          # workspace
├── crates/kridtiler-cli/
│   └── src/
│       ├── main.rs                     # CLI 入口、模式分发、D-Bus 客户端
│       ├── preset.rs                   # 内置预设表
│       └── config.rs                   # ~/.config/kridtiler/config.toml 解析
├── kwin-script/
│   ├── tile.js.tmpl                    # --tile / 预设的 KWin JS 脚本模板
│   ├── overlay.qml.tmpl                # --overlay 全屏 QML
│   └── popup.qml.tmpl                  # --show / --popup gTile 风格 QML
├── config.example.toml
└── README.md
```

## 许可证

本项目以 **GNU General Public License v3.0 or later**（GPL-3.0-or-later）开源 — 完整条款见 [LICENSE](LICENSE)。

简而言之：你可以自由使用、研究、修改、再分发本软件,但若分发派生作品,派生作品必须同样以 GPL-3.0-or-later 开源。

