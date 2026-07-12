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
position = [0, 0]
size = [800, 200]

[lyrics]
font_size = 28
providers = ["qq", "netease"]

[spotify]
mpris_prefix = "org.mpris.MediaPlayer2.spotify"
```

若 Spotify 使用 Flatpak/Snap，可能需要修改 `mpris_prefix` 为实际 D-Bus 名称
（如 `org.mpris.MediaPlayer2.spotify.instanceXXXXXXX`）。

## 已知限制

- 仅支持 Wayland（不支持 X11）
- 自动跟踪仅针对 Spotify（其他 MPRIS 客户端需手动配置）
- QQ 音乐与网易云音乐接口可能因服务端变更暂时不可用
- 自动切歌后偶有进度偏差，暂停后恢复即可重新校准

## 许可证

[GPL-3.0-or-later](LICENSE)
