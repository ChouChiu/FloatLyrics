# FloatLyrics

FloatLyrics 是一款面向 Linux Wayland 的 MPRIS 悬浮歌词应用。它会自动跟随当前播放器，在不拦截鼠标操作的桌面浮层中显示同步歌词和逐字卡拉 OK 动画。

<table>
  <thead>
    <tr>
      <th>歌词浮窗</th>
      <th>设置页</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td><img src="docs/screenshots/lyrics.png" alt="FloatLyrics 歌词浮窗" width="1000"></td>
      <td><img src="docs/screenshots/setting.png" alt="FloatLyrics 设置页" width="1000"></td>
    </tr>
  </tbody>
</table>

## 核心特性

- **通用播放器支持**：自动发现 Spotify、VLC、mpv、Rhythmbox、浏览器等兼容 MPRIS 的播放器，并在多个实例之间选择当前活跃播放器。
- **流畅的同步歌词**：支持 Apple Music 风格逐字高亮、平滑换行动画以及普通同步歌词。
- **自动搜索，也可手动选择**：按配置顺序搜索 QQ 音乐和网易云音乐；匹配不理想时可手动选择结果。
- **翻译与罗马音**：支持歌词翻译、普通话拼音、粤拼、韩语罗马音和日语音读。
- **可定制桌面浮层**：支持自由拖放、边缘吸附、字体与颜色调整、透明度、底部面板预留，以及按歌曲记忆的时间偏移。
- **本地缓存与多语言界面**：歌词缓存在 SQLite 中，可离线复用；English、简体中文与繁體中文可在运行时切换。

## 运行要求

| 组件 | 要求 |
|---|---|
| 桌面会话 | Linux Wayland，合成器支持 layer-shell |
| 播放器 | 提供 `PlaybackStatus`、`Position` 和含标题 `Metadata` 的 MPRIS 播放器 |
| 运行库 | GTK 4.12 或更高版本、gtk4-layer-shell、WebKitGTK 6.0 |

FloatLyrics 依赖 layer-shell，目前不支持 X11。可运行 `echo "$XDG_SESSION_TYPE"` 检查当前会话类型。

已知兼容的合成器包括 GNOME（Mutter）、KDE（KWin）、Hyprland 和 Sway；其他支持 layer-shell 的合成器也可能正常工作。

## 安装

### Arch Linux

推荐安装预编译的 AUR 包：

```bash
paru -S floatlyrics-bin
# 或
yay -S floatlyrics-bin
```

也可以安装从源码构建的 [`floatlyrics`](https://aur.archlinux.org/packages/floatlyrics) 包。预编译包见 [`floatlyrics-bin`](https://aur.archlinux.org/packages/floatlyrics-bin)。

### Fedora / openSUSE

从 [GitHub Releases](https://github.com/ChouChiu/FloatLyrics/releases) 下载适合架构的 RPM，然后安装：

```bash
sudo dnf install ./floatlyrics-*.rpm
```

在 openSUSE 上也可使用 `sudo zypper install ./floatlyrics-*.rpm`。

### Debian / Ubuntu

从 [GitHub Releases](https://github.com/ChouChiu/FloatLyrics/releases) 下载 DEB，然后安装：

```bash
sudo apt install ./floatlyrics_*.deb
```

需要 Ubuntu 25.04 或更新版本；Ubuntu 24.04 及更早版本缺少 `libgtk4-layer-shell0`。

### 从源码构建

先安装 Rust 1.93+、Bun 1.3.14、C 工具链以及 GTK、layer-shell、WebKitGTK、OpenSSL 的开发包。Bun 请按[官方安装说明](https://bun.com/docs/installation)安装：

```bash
# Arch Linux
sudo pacman -S --needed base-devel git gtk4 gtk4-layer-shell webkitgtk-6.0 openssl rust

# Fedora
sudo dnf install gcc git gtk4-devel gtk4-layer-shell-devel webkitgtk6.0-devel openssl-devel rust cargo

# Debian / Ubuntu 25.04+
sudo apt install build-essential git libgtk-4-dev libgtk4-layer-shell-dev libwebkitgtk-6.0-dev libssl-dev rustc cargo
```

然后构建：

```bash
git clone https://github.com/ChouChiu/FloatLyrics.git
cd FloatLyrics
cargo build --locked --release
```

Cargo 会根据 `bun.lock` 自动安装前端依赖，并将 React 歌词页构建为内嵌的单文件 HTML；Bun 不属于最终二进制的运行时依赖。

生成的可执行文件位于 `target/release/floatlyrics`。如果你准备修改项目，请继续阅读 [贡献指南](CONTRIBUTING.md)。

## 使用

启动兼容 MPRIS 的播放器并开始播放，然后运行：

```bash
floatlyrics
```

FloatLyrics 按 `Playing`、`Paused`、`Stopped` 的顺序选择播放器。状态相同时优先使用 `player.preferred_players` 中的播放器，并尽量保持当前实例，避免来回切换。

浮窗的基本操作：

- 将鼠标移到浮窗上，显示歌词搜索、时间校准、设置和关闭按钮。
- 拖动浮窗可改变位置；靠近屏幕边缘或中央时会自动吸附。
- 点击 `−` 让本曲歌词延后 100 毫秒，点击 `+` 让歌词提前 100 毫秒。
- 点击中间的偏移值可重置本曲校准。该值按歌曲保存，并与全局偏移 `lyrics.offset_ms` 叠加。

### 播放器兼容性

满足上述 MPRIS 要求的播放器都可以通过标题与歌手搜索歌词，例如 VLC、Amberol、Lollypop、Telegram、listen1-desktop、Chrome 和 YouTube Music。部分播放器需要额外的 MPRIS 集成：

- Firefox：安装 Plasma Integration 等 MPRIS 扩展。
- mpv：安装 `mpv-mpris`。
- DeaDBeeF：启用 MPRIS2 插件。
- ncmpcpp：配合 `mpd-mpris` 使用。

部分播放器还会提供可直接定位歌词源曲目的 ID：

| 播放器 | 元数据 | 对应歌词源 |
|---|---|---|
| Electron-NCM、Qcm、go-musicfox 4.3.2+、netease-cloud-music-gtk 2.3.0+ | `mpris:trackid` | 网易云音乐 |
| FeelUOwn 3.9.12+ | `xesam:url` | 网易云音乐或 QQ 音乐 |
| YesPlayMusic | `xesam:url` | 网易云音乐 |

精确歌曲 ID 只会在轮到对应歌词源时使用，不会改变 `lyrics.provider_order`：

1. 手动选择过的歌词始终具有最高优先级。
2. 自动搜索仍严格遵循配置的歌词源顺序。
3. 对应来源被禁用时，歌曲 ID 会被忽略；ID 无效或接口暂时失败时，会回退到该来源的标题与歌手搜索。

> **已知限制：** Linux 官方 QQ 音乐客户端的 `Position` 可能始终为 0，因此无法可靠同步。QQ 音乐歌词源以及 FeelUOwn 中的 QQ 音乐曲目不受此限制。

常用启动参数：

| 参数 | 用途 |
|---|---|
| `--debug` | 输出详细诊断日志 |
| `--config <PATH>` | 使用指定的配置文件 |
| `--reset-window` | 恢复默认窗口位置和尺寸 |
| `--settings` | 启动时打开设置窗口 |
| `--select-lyrics` | 为当前曲目打开手动歌词搜索 |

完整参数以 `floatlyrics --help` 为准。

## 配置

大多数选项可直接在设置页修改。默认配置文件位于 `~/.config/floatlyrics/config.toml`，首次启动时自动创建：

```toml
[general]
language = "zh-CN"                               # en | zh-CN | zh-TW

[window]
anchor = "bottom-center"
remember_position = true
# position = { horizontal = 0.5, vertical = 0.85 }  # 拖动后自动写入，范围 0.0-1.0
margin = 96
width = 350
opacity = 0.78
bottom_panel_height = 36

[lyrics]
offset_ms = 0
apple_music_style = false
provider_order = ["qq-music", "netease"]
show_translation = true
show_romanization = false
chinese_romanization = "auto"                    # auto | mandarin-pinyin | cantonese-jyutping | cantonese-jyutping-no-tones
font_order = ["Sans"]
lyric_font_size = 24
translation_font_size = 13
romanization_font_size = 12
played_color = "#FFFFFFFF"
unplayed_color = "#9EA6B3FF"
translation_color = "#FFFFFFC7"
romanization_color = "#B8D8F0E6"

[player]
preferred_players = ["spotify"]
ignored_players = []                             # 例如 ["firefox"]
```

播放器选择器可以填写 MPRIS 名称后缀（如 `spotify`）、完整总线名或播放器的 `Identity`，匹配时不区分大小写。

配置文件采用严格校验。启动时遇到旧版或不兼容配置会自动修复：

- 保留能够识别且有效的字段。
- 忽略未知字段；类型错误、非法枚举和越界值仅回退对应字段。
- TOML 整体无法解析时使用默认配置。
- 修复前的原文件保存为 `config.toml.incompatible`；若文件已存在，则追加数字后缀。
- 读取、备份或写回失败仍会阻止启动，避免静默丢失配置。

## 常见问题与限制

- **浮窗没有出现：** 确认当前是 Wayland 会话、合成器支持 layer-shell，并且播放器正在通过 MPRIS 暴露播放状态。
- **播放器没有被识别：** 运行 `busctl --user list` 检查 `org.mpris.MediaPlayer2.*` 名称，必要时调整 `player.preferred_players` 或 `player.ignored_players`。
- **歌词时间不准：** 使用浮窗按钮校准当前歌曲，或在设置中调整全局偏移 `lyrics.offset_ms`。播放器未正确更新 `Position` 或 `Metadata` 时可能无法可靠同步。
- **歌词显示方框或乱码：** 在 `lyrics.font_order` 中加入已安装的中文、日文或韩文字体。
- **在线搜索失败：** QQ 音乐与网易云音乐接口可能因服务端变更暂时不可用；已缓存的歌词不受影响。
- **需要重置配置：** 删除 `~/.config/floatlyrics/config.toml` 后重启；只重置窗口设置可使用 `--reset-window`。

## 参与贡献

欢迎提交 bug、功能建议、歌词解析改进、翻译和文档修正。开始编码前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)；较大的功能或行为变更建议先开 issue 讨论。

## 致谢

感谢 [LyricsX](https://github.com/MxIris-LyricsX-Project/LyricsX) 与 [Lyricify-App](https://github.com/WXRIW/Lyricify-App) 为 FloatLyrics 带来灵感，本项目的部分功能与交互设计参考了 LyricsX。

感谢 [Waylyrics](https://github.com/waylyrics/waylyrics) 对 Linux 播放器兼容性的长期实践；FloatLyrics 的通用 MPRIS 选择策略参考了其活跃播放器优先、名称与 Identity 筛选等经验。

感谢 [OpenAI](https://openai.com/) 与 [DeepSeek](https://www.deepseek.com/) 带来如此出色的 AI 模型，本项目在开发过程中使用了这些模型生成代码与文档。

感谢 AUR 软件包维护者 [NihilDigit](https://github.com/NihilDigit) 与 [Integral-Tech](https://github.com/Integral-Tech) 为 FloatLyrics 提供并维护 Arch Linux 软件包。

## 许可证

FloatLyrics 以 [AGPL-3.0-only](LICENSE) 许可发布。
