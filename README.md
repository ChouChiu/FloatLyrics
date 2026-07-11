# FloatLyrics

FloatLyrics 是一款使用 Rust 编写的 Linux Wayland 桌面歌词应用，当前主要支持通过
Spotify MPRIS 获取播放状态并显示悬浮歌词。

## 环境要求

- Linux Wayland 会话
- GTK 4.12 或更新版本
- gtk4-layer-shell
- 支持 MPRIS 的 Spotify 客户端
- Rust 1.92 或更新版本（从源码构建时）

## 运行

```bash
cargo run -- --debug
```

支持以下命令行参数：

```bash
floatlyrics
floatlyrics --debug
floatlyrics --config <path>
floatlyrics --reset-window
floatlyrics --settings
floatlyrics --select-lyrics
```

默认数据路径：

- 配置文件：`~/.config/floatlyrics/config.toml`
- 数据库：`~/.local/share/floatlyrics/floatlyrics.sqlite3`

配置文件会在首次启动时生成。若 Spotify 客户端使用非标准的 MPRIS 总线名前缀，
可以覆盖对应配置并重启 FloatLyrics：

```toml
[spotify]
mpris_prefix = "org.mpris.MediaPlayer2.spotify"
```

## 开发验证

```bash
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --locked --release
make validate-data
```

## 项目架构

项目由单个 Cargo 包组成，内部模块按职责划分：

- `src/lib.rs`：命令行解析、启动流程与公开模块入口。
- `src/app.rs`：应用组合根与依赖装配。
- `src/app/`：播放控制器、独立于 GTK 的展示模型、设置页与 GTK 视图。
- `src/main.rs`：精简的二进制入口，调用 `floatlyrics::run()`。
- `src/lyrics/`：歌词模型、解析、歌词源搜索与时间轴计算。
- `src/mpris/`：D-Bus 监听、播放器模型与播放位置同步。
- `src/cache.rs`、`src/config.rs`、`src/paths.rs`、`src/telemetry.rs`：缓存、配置、
  本地路径与遥测等基础设施。

## 当前功能范围

- 通过 MPRIS 监听 Spotify 播放状态。
- 按配置顺序从 QQ 音乐和网易云音乐获取歌词。
- 搜索、预览并为当前歌曲持久绑定手动选择的歌词。
- 使用 GTK4 与 Wayland layer-shell 显示悬浮歌词窗口。
- 使用 SQLite 缓存歌曲、歌词、手动匹配、歌词源结果与设置。

## 已知限制

- 仅支持 Wayland，且桌面环境或合成器必须支持 layer-shell。
- 当前只自动跟踪 Spotify 及使用所配置 MPRIS 前缀的兼容客户端。
- 在线歌词依赖 QQ 音乐和网易云音乐的非稳定公共接口，可能因服务端变化暂时不可用。
