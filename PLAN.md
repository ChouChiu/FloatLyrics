# FloatLyrics MVP 计划

  ## Summary

  创建 Rust 桌面歌词应用 FloatLyrics，面向 Linux Wayland，优先适配 CachyOS/Arch + Niri。产品体验参考
  LyricsX 的核心能力：自动搜索下载歌词、桌面悬浮显示、样式和位置调整、歌词偏移、歌词跳转、导入导出和
  多源歌词。

  播放器集成第一版只支持 Spotify：应用只跟随 Spotify 的 MPRIS 实例。歌词核心复用 ChouChiu/Lyrics-
  Helper，在线歌词源使用 QQ 音乐、网易云音乐，默认搜索优先级为 QQ 音乐 -> 网易云音乐。

  ## Key Changes

  - 技术栈：Rust 2024、GTK4、libadwaita、gtk4-layer-shell、zbus、tokio、lyrics-helper、rusqlite、
    serde、tracing、clap。

  - 歌词核心：使用 lyrics_helper::parse_auto 解析本地歌词，generate_string 导出歌词，LyricsData /
    LineInfo 作为内部模型。

  - 播放器：只跟随 org.mpris.MediaPlayer2.spotify 或兼容 Spotify MPRIS 名称，不提供多播放器选择。
  - 歌词源：只启用 QQ 音乐、网易云音乐；默认顺序为 QQ -> 网易云。
  - UI：GTK 主线程负责悬浮歌词窗口和设置窗口，Tokio 后台任务负责 Spotify MPRIS、歌词搜索、缓存和网络
    请求。

  - 悬浮窗：使用 layer-shell，默认屏幕下方居中，支持普通歌词、逐字高亮、翻译、拼音、背景和声显示已有
    数据。

  - 数据：SQLite 缓存 tracks、lyrics、manual_matches、provider_results、settings；手动匹配优先级最
    高。

  ## Public Interfaces

  - CLI：
      - floatlyrics
      - floatlyrics --debug
      - floatlyrics --config <path>
      - floatlyrics --reset-window

  - 默认路径：
      - 配置：~/.config/floatlyrics/config.toml
      - 数据库：~/.local/share/floatlyrics/floatlyrics.sqlite3

  ## Test Plan

  - 单元测试：Spotify metadata 转换、歌词行定位、track 指纹、手动匹配优先级、SQLite 缓存。
  - 集成测试：mock Spotify MPRIS、验证非 Spotify 不跟随、mock QQ/网易云搜索顺序。
  - 手动验收：Niri Wayland 下显示悬浮窗，Spotify 播放时自动刷新歌词，设置窗口可搜索/绑定/管理缓存，样
    式和偏移即时生效。

  ## Assumptions

  - “只需支持 Spotify”指播放器集成只支持 Spotify；歌词源仍保留 QQ 音乐、网易云音乐。
  - Lyrics-Helper 作为歌词核心依赖，FloatLyrics 不重复实现歌词格式解析、生成、解密和搜索。
  - 拼音和背景和声优先显示已有歌词数据中的内容；自动生成拼音不纳入 MVP。
  - Linux 版本以悬浮窗口 + 设置窗口为主，不复刻 LyricsX 的 macOS 菜单栏体验。
