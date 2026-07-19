# FloatLyrics

面向 Linux Wayland 的 Spotify 悬浮歌词：自动跟随当前曲目，在桌面上自由放置同步歌词浮窗，并呈现逐字卡拉 OK 效果。

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

## 为什么用 FloatLyrics

- 开箱即用：通过 MPRIS 自动跟踪 Spotify 的曲目、播放状态和进度。
- 专为桌面歌词设计：基于 GTK4、WebKitGTK 与 layer-shell，浮层始终置顶，同时不拦截其他窗口的鼠标操作。
- 逐字卡拉 OK 效果：支持 Apple Music 风格的逐字高亮动画，以及平滑的行间过渡切换。
- 完整的歌词生态：支持翻译显示、中文罗马音（拼音/粤拼）、韩语罗马音与日语音读；可自由调整文字颜色、字号和字体。
- 找不到也能自己选：自动搜索 QQ 音乐和网易云音乐，也可以为当前曲目手动选择结果。
- 越用越省心：已匹配歌词写入本地 SQLite 缓存，之后可离线使用。
- 适应你的桌面：浮窗可自由拖放，靠近屏幕边缘时自动吸附；透明度、字体、字号、偏移量和底部面板预留均可自定义。
- 三种界面语言：English、简体中文与繁體中文可在运行时切换。

## 运行要求

| 组件 | 要求 |
|---|---|
| 桌面会话 | Linux Wayland，合成器支持 layer-shell |
| 播放器 | Spotify 官方客户端，或能暴露 Spotify MPRIS 名称的 Flatpak/Snap 客户端 |
| 运行库 | GTK 4.12 或更高版本、gtk4-layer-shell、WebKitGTK 6.0 |

FloatLyrics 依赖 wlr-layer-shell 协议，目前不支持 X11。若不确定当前会话类型，可运行 `echo "$XDG_SESSION_TYPE"` 检查。

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

启动 Spotify 后运行：

```bash
floatlyrics
```

FloatLyrics 会自动等待并跟踪 Spotify。浮窗默认位于屏幕底部中央；将鼠标移到浮窗上可显示操作按钮，也可将它拖到桌面的其他位置。靠近屏幕边缘时，浮窗会自动吸附。

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

[spotify]
mpris_prefix = "org.mpris.MediaPlayer2.spotify"
```

配置保存仍采用严格校验，但启动读取会自动恢复不兼容的旧配置：能够识别且有效的字段会继续保留，未知字段会被忽略，类型错误、非法枚举和越界值只会让对应字段回退默认值；整份 TOML 无法解析时才会整体使用默认配置。恢复前的原始内容会保存为同目录下的 `config.toml.incompatible`（已有备份时追加数字后缀），随后写回当前规范格式。文件读取、备份或写入失败仍会阻止启动，避免静默丢失配置。

## 常见问题与限制

- 浮窗没有出现：确认正在使用 Wayland、合成器支持 layer-shell，并且 Spotify 已启动播放。
- Flatpak/Snap Spotify 无法识别：检查播放器暴露的 MPRIS 名称，并修改 `spotify.mpris_prefix`。
- 歌词进度偶尔偏移：可在设置中调整全局偏移量（`lyrics.offset_ms`）；切歌后若短暂失准，暂停再继续可重新校准。
- 歌词显示方框或乱码：在设置中调整字体优先级（`lyrics.font_order`），添加已安装的中文/日韩字体。
- 如何彻底重置配置：删除 `~/.config/floatlyrics/config.toml`，下次启动会自动生成默认配置；或使用 `--reset-window` 只恢复窗口位置。
- 自动跟踪目前仅针对 Spotify。
- QQ 音乐与网易云音乐接口可能因服务端变更暂时不可用；已经缓存的歌词不受影响。

## 参与贡献

欢迎提交 bug、功能建议、歌词解析改进、翻译和文档修正。开始编码前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)；较大的功能或行为变更建议先开 issue 讨论。

## 致谢

感谢 [LyricsX](https://github.com/MxIris-LyricsX-Project/LyricsX) 与 [Lyricify-App](https://github.com/WXRIW/Lyricify-App) 为 FloatLyrics 带来灵感，本项目的部分功能与交互设计参考了 LyricsX。

感谢 [OpenAI](https://openai.com/) 与 [DeepSeek](https://www.deepseek.com/) 带来如此出色的 AI 模型，本项目在开发过程中使用了这些模型生成代码与文档。

感谢 AUR 软件包维护者 [NihilDigit](https://github.com/NihilDigit) 与 [Integral-Tech](https://github.com/Integral-Tech) 为 FloatLyrics 提供并维护 Arch Linux 软件包。

## 许可证

FloatLyrics 以 [AGPL-3.0-only](LICENSE) 许可发布。
