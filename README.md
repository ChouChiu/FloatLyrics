# FloatLyrics

FloatLyrics 是一款使用 Rust 编写的 Linux Wayland 桌面歌词应用，当前主要支持通过
Spotify MPRIS 获取播放状态并显示悬浮歌词。

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
```

`--settings` 会在已有的 FloatLyrics 进程中打开设置窗口，因此可以直接作为桌面栏按钮的
点击命令，不会重复启动 Spotify 监听器。

## 在 Niri 的 Noctalia Shell 中使用

Noctalia Shell v4 已内置 `CustomButton` 桌面栏组件，不需要安装额外插件：

1. 打开 **Noctalia 设置 → 状态栏 → 小组间**，将 **CustomButton** 添加到顶栏的左侧、中央或
   右侧区域。
2. 打开该组件的设置，将图标设为 `music`，提示设为 `打开 FloatLyrics 设置`，
   并将**左键点击命令**设为：

   ```bash
   floatlyrics --settings
   ```

   `floatlyrics` 必须已经安装到 `PATH`。开发期间可以改用
   `target/debug/floatlyrics` 的绝对路径。

Niri 会将设置页识别为普通窗口。若要让它以居中的浮动窗口打开，请将以下规则添加到
`~/.config/niri/config.kdl`：

```kdl
window-rule {
    match app-id=r#"^io\.github\.chouchiu\.FloatLyrics$"#
    open-floating true
    default-column-width { fixed 680; }
    default-window-height { fixed 500; }
}
```

设置分类位于窗口顶部工具栏，整体结构参考了
[LyricsX 偏好设置](https://github.com/MxIris-LyricsX-Project/LyricsX)的紧凑工具栏布局。

默认数据路径：

- 配置文件：`~/.config/floatlyrics/config.toml`
- 数据库：`~/.local/share/floatlyrics/floatlyrics.sqlite3`

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
- 使用 GTK4 与 Wayland layer-shell 显示悬浮歌词窗口。
- 使用 SQLite 缓存歌曲、歌词、手动匹配、歌词源结果与设置。
