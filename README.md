# FloatLyrics

Linux Wayland 桌面悬浮歌词，自动跟踪 Spotify 播放状态并实时显示同步歌词。

<table>
  <thead>
    <tr>
      <th>歌词浮窗</th>
      <th>设置页</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td><img src="docs/screenshots/lyrics.png" alt="歌词浮窗" width="1000"></td>
      <td><img src="docs/screenshots/setting.png" alt="设置页"  width="1000"></td>
    </tr>
  </tbody>
</table>

> 私心安利：  
> XLOV 是韩国 257 娱乐于 2025 年推出的四人跨国籍无性别概念团体。  
> 团名将代表未知与否定的 “X”，与代表未完成爱情的 “LOV” 组合而成，融入每位成员独特的个性，以「无性别概念」为核心主题。  
> 了解更多，前往 [维基百科](https://zh.wikipedia.org/zh-cn/XLOV)，[认人视频 1（B 站）](https://www.bilibili.com/video/BV1YPKS6LEDR/?vd_source=915fd0a7ea4424cb0a2d1c698d30a0b1)，[认人视频 2 （B 站）](https://www.bilibili.com/video/BV15ofmYjEKF/)，[官方油管账号](https://www.youtube.com/@XLOV_official)

## 功能

- **自动跟踪** — 实时获取 Spotify 播放状态，无需额外配置
- **悬浮显示** — 始终置顶的浮层，不遮挡其他窗口交互
- **同步歌词** — 支持逐字卡拉 OK 着色
- **多源搜索** — 自动从 QQ 音乐、网易云音乐获取歌词，也可手动搜索
- **本地缓存** — 歌词持久缓存，离线可用
- **多语言** — English / 简体中文 / 繁體中文，运行时切换
- **可拖拽** — 拖拽调整位置，自动吸附屏幕边缘

## 环境要求

| 组件 | 说明 |
|---|---|
| 系统 | Linux Wayland（合成器需支持 layer-shell） |
| Spotify | 官方或 Flatpak/Snap 客户端 |
| 依赖 | GTK4 ≥ 4.12、gtk4-layer-shell |

不支持 X11。

## 安装

### Arch Linux

从 [AUR](https://aur.archlinux.org/packages/floatlyrics-bin) 安装：

```bash
paru -S floatlyrics-bin
# OR
yay -S floatlyrics-bin
```

需要从源码构建时，安装 [floatlyrics](https://aur.archlinux.org/packages/floatlyrics)。

### Fedora / openSUSE

```bash
sudo dnf install ./floatlyrics-*.rpm
```

### Debian / Ubuntu（25.04+）

```bash
sudo apt install ./floatlyrics_*.deb
```

不支持 Ubuntu 24.04 及更早版本（缺少 `libgtk4-layer-shell0`）。

### 从源码构建

```bash
# Arch
sudo pacman -S --needed base-devel git gtk4 gtk4-layer-shell openssl rust

# Fedora
sudo dnf install gcc git gtk4-devel gtk4-layer-shell-devel openssl-devel rust cargo

# Debian/Ubuntu (25.04+)
sudo apt install build-essential git libgtk-4-dev libgtk4-layer-shell-dev libssl-dev rustc cargo

git clone https://github.com/ChouChiu/FloatLyrics.git
cd FloatLyrics
cargo build --locked --release
```

二进制文件位于 `target/release/floatlyrics`。

在 Arch Linux 上构建 AUR 安装包：

```bash
packaging/build-aur.sh --cleanbuild
```

安装包会生成在仓库根目录。该脚本使用独立的 makepkg 工作目录，避免与 Rust 的 `src/` 目录冲突。

## 维护 AUR 包

发布 AUR 包前，对应的 Git tag 和 GitHub Release RPM 必须已存在。发布脚本会更新版本号、校验和与 `.SRCINFO`，校验 PKGBUILD，并在推送前要求确认。

```bash
# 只发布源码版
packaging/release-aur.sh floatlyrics 1.0.0

# 只发布预编译版
packaging/release-aur.sh floatlyrics-bin 1.0.0

# 同时发布两个包
packaging/release-aur.sh all 1.0.0
```

只更新和校验本地包文件：

```bash
packaging/release-aur.sh --prepare-only all 1.0.0
```

## 使用

```bash
floatlyrics
```

| 参数 | 说明 |
|---|---|
| `--debug` | 启用详细日志 |
| `--config <path>` | 指定配置文件路径 |
| `--reset-window` | 重置窗口位置和大小 |
| `--settings` | 直接打开设置窗口 |
| `--select-lyrics` | 对当前曲目手动搜索歌词 |

启动后浮层自动吸附到屏幕边缘，拖拽可移动。

## 配置

配置文件 `~/.config/floatlyrics/config.toml`，首次运行自动生成：

```toml
[general]
language = "zh-CN"          # en | zh-CN | zh-TW

[window]
anchor = "bottom-center"
margin = 96
width = 350
opacity = 0.78
bottom_panel_height = 36

[lyrics]
offset_ms = 0
provider_order = ["qq-music", "netease"]
show_translation = true
show_romanization = false
font_order = ["Sans"]

[spotify]
mpris_prefix = "org.mpris.MediaPlayer2.spotify"
```

配置必须包含上述全部字段；未知字段或旧版、不完整的配置会在启动时报错。
Flatpak/Snap 版 Spotify 可按实际 D-Bus 名称修改 `mpris_prefix`。

## 已知限制

- 仅支持 Wayland（不支持 X11）
- 自动跟踪仅针对 Spotify
- QQ 音乐与网易云音乐接口可能因服务端变更暂时不可用
- 自动切歌后偶有进度偏差，暂停后恢复即可重新校准

## 许可证

[GPL-3.0-or-later](LICENSE)
