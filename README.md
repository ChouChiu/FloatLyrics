# FloatLyrics

Linux Wayland 桌面悬浮歌词应用，基于 Rust、Relm4 与 GTK4 layer-shell 构建，
通过 MPRIS 跟踪 Spotify 播放状态并显示同步歌词。

## 环境要求

- Linux Wayland 合成器，需支持 **layer-shell** 协议
- GTK 4.12+
- gtk4-layer-shell
- Spotify 客户端（或使用匹配 MPRIS 总线名前缀的兼容客户端）
- 从源码构建需 Rust 1.93+

## 安装

每个 [release](https://github.com/ChouChiu/FloatLyrics/releases) 提供预构建的
`.deb` 和 `.rpm` 包。

### 从源码构建

```bash
git clone https://github.com/ChouChiu/FloatLyrics.git
cd FloatLyrics
cargo build --locked --release
```

二进制文件位于 `target/release/floatlyrics`。

## 使用

```bash
floatlyrics
```

| 参数 | 说明 |
|---|---|
| `--debug` | 启用详细日志 |
| `--config <path>` | 使用指定的配置文件 |
| `--reset-window` | 将窗口位置和大小重置为默认值 |
| `--settings` | 直接打开设置窗口 |
| `--select-lyrics` | 打开当前曲目的手动歌词搜索 |

## 配置

配置文件位于 `~/.config/floatlyrics/config.toml`（首次启动自动生成）。若 Spotify
客户端使用非标准 MPRIS 总线名前缀，可覆盖配置项：

```toml
[spotify]
mpris_prefix = "org.mpris.MediaPlayer2.spotify"
```

数据库与缓存：`~/.local/share/floatlyrics/floatlyrics.sqlite3`。

## 架构

Cargo 工作空间，包含三个 crate，自上而下分层：

| Crate | 职责 |
|---|---|
| `floatlyrics` | CLI、Relm4/GTK4 layer-shell 界面、MPRIS 监听、应用组合根 |
| `floatlyrics-lyrics` | 歌词模型、解析、搜索、时间轴、SQLite 缓存 |
| `floatlyrics-core` | 应用路径、i18n、遥测、曲目指纹 |

依赖方向：`floatlyrics` → `floatlyrics-lyrics` → `floatlyrics-core`。

主要源模块：

- `src/lib.rs` — 命令行解析、GTK 渲染器初始化、启动流程
- `src/app.rs` — Relm4 应用组件、消息路由与窗口生命周期
- `src/app/controller.rs` — 播放控制器与状态机
- `src/app/model.rs` — 独立于 GTK 的展示模型
- `src/app/view.rs`、`src/app/view/` — GTK 组件与 layer-shell 窗口
- `src/app/settings.rs`、`src/app/manual_search.rs`、`src/app/about.rs` — 设置、搜索、关于页面
- `src/mpris/` — D-Bus MPRIS 监听与播放位置同步
- `src/config.rs` — 原子化配置读写（临时文件 + 重命名）
- `floatlyrics-core/src/i18n.rs`、`data/locale/*.json` — 运行时 i18n（English / 简体中文 / 繁體中文）
- `floatlyrics-lyrics/src/lyrics/` — LRC 解析、QQ 音乐与网易云音乐搜索、时间轴

## 功能

- 通过 MPRIS 跟踪 Spotify 播放状态
- GTK4 layer-shell 悬浮歌词显示
- 按可配置顺序从 QQ 音乐和网易云音乐自动抓取歌词
- 手动搜索歌词并持久绑定到当前曲目
- SQLite 缓存曲目、歌词、手动匹配及设置
- 支持英文、简体中文、繁體中文
- 设置窗口、开源依赖列表的关于页面

## 已知限制

- 仅支持 Wayland，且合成器须支持 layer-shell 协议
- 仅自动跟踪 Spotify（或使用配置的 MPRIS 前缀的兼容客户端）
- QQ 音乐与网易云音乐的歌词接口为非官方接口，可能因服务端变更而暂时不可用
- 自动切歌后歌词进度可能超前，可手动暂停后恢复以重新校准

## 开发

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
```

筛选测试：`cargo test lyrics::`、`cargo test mpris::` 等。

提交规范与工作流详见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

[GPL-3.0-or-later](LICENSE)
