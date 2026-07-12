# 为 FloatLyrics 做贡献

感谢你愿意改进 FloatLyrics。代码、测试、文档、翻译、问题报告与设计建议都很有价值。本指南说明如何搭建环境、确定改动归属、验证结果并提交便于审阅的 Pull Request。

## 开始之前

请先搜索现有 [Issues](https://github.com/ChouChiu/FloatLyrics/issues) 和 [Pull Requests](https://github.com/ChouChiu/FloatLyrics/pulls)，确认没有重复工作。

- 小型修复、测试和文档改进可以直接提交 PR。
- 新功能、行为变化、架构调整或新增系统依赖，请先开 issue 对齐方向。
- 安全问题不要发布在公开 issue；请通过仓库所有者提供的私密联系方式报告。

## 准备开发环境

项目使用 Rust 2024 edition，最低支持 Rust 1.93。`rust-toolchain.toml` 会选择 stable，并安装 `rustfmt`、Clippy、Rust 源码与 rust-analyzer。

运行完整应用需要 Linux Wayland、支持 layer-shell 的合成器、会话 D-Bus、GTK 4.12+、gtk4-layer-shell 和 OpenSSL。普通单元测试不应依赖桌面会话。

Arch Linux 可直接安装所需依赖：

```bash
sudo pacman -S --needed base-devel git gtk4 gtk4-layer-shell openssl rust
```

Fedora：

```bash
sudo dnf install gcc git gtk4-devel gtk4-layer-shell-devel openssl-devel rust cargo
```

Debian / Ubuntu 25.04+：

```bash
sudo apt install build-essential git libgtk-4-dev libgtk4-layer-shell-dev libssl-dev rustc cargo
```

获取代码并验证环境：

```bash
git clone https://github.com/ChouChiu/FloatLyrics.git
cd FloatLyrics
cargo build --locked
cargo test --locked --all-targets --all-features
```

在支持 layer-shell 的 Wayland 会话中启动开发版本：

```bash
cargo run --locked -- --debug
```

`--debug` 会启用详细 tracing 日志，并非只表示使用 debug profile。GTK 由 Relm4 初始化，请勿额外调用 `gtk::init()`。

## 理解工作区

```text
floatlyrics (src/)              CLI、应用组装、Relm4/GTK4 UI、MPRIS
  └─ floatlyrics-lyrics/        歌词模型、LRC/QRC 解析、搜索、SQLite 缓存
       └─ floatlyrics-core/     路径、i18n、遥测、曲目指纹
```

依赖只能沿图中方向自上而下。与 GTK、D-Bus、网络或数据库无关的领域逻辑，应放入能够承载它的最底层 crate，避免让可复用逻辑依赖应用边界。

| 路径 | 职责 |
|---|---|
| `src/lib.rs` | CLI 参数与应用启动流程 |
| `src/app.rs`、`src/app/` | Relm4 应用、控制器、展示模型、视图和设置页 |
| `src/mpris/` | Spotify MPRIS 发现、事件监听和进度同步 |
| `src/config.rs` | 配置模型与原子写入（临时文件 + rename） |
| `floatlyrics-lyrics/src/` | 歌词解析、时间轴、搜索提供方与缓存 |
| `floatlyrics-core/src/` | 跨 crate 的基础能力与稳定领域类型 |
| `data/locale/` | 三种语言的 JSON 文案目录 |
| `packaging/` | AUR 构建/发布脚本与发行资源 |

## 实现约定

- 使用 `rustfmt` 默认格式；模块、函数和变量使用 `snake_case`，类型与 trait 使用 `PascalCase`，常量使用 `SCREAMING_SNAKE_CASE`。
- 保持 `main.rs` 最小化，将可测试、可复用逻辑放入库或领域模块。
- 错误应携带足够上下文，但不要在底层库中直接决定 UI 呈现。
- 不要在业务逻辑或测试中硬编码开发者本机路径、账户、网络服务或桌面会话状态。
- 修改配置格式时要考虑现有用户；配置写入必须继续采用原子替换。
- 新增公共 API 时补充 rustdoc。项目将 rustdoc warning 视为错误。

### 用户界面文案与 i18n

每个用户可见字符串都必须通过本地化层提供，并同时存在于：

```text
data/locale/en.json
data/locale/zh-CN.json
data/locale/zh-TW.json
```

新增 key 时，还必须加入 `floatlyrics-core/src/i18n.rs` 的 `define_text_keys!` 宏。不要在 GTK 视图或业务逻辑中绕过本地化层硬编码文案。

应用启动时会调用 `i18n::validate_catalogues()` 校验三份 catalogue。对应测试位于 `floatlyrics-core/src/test/i18n_test.rs`。

## 编写测试

测试模块使用 `#[cfg(test)]`，并通过 `#[path = "test/foo_test.rs"]` 将实现放在各 crate 的 `src/test/` 下。

- 测试名称描述可观察行为，例如 `parses_enhanced_lrc`。
- bug 修复应附带能够先复现问题的回归测试。
- 单元测试不得要求 Spotify、D-Bus、网络、Wayland 合成器或开发者本地路径。
- 文件系统和数据库测试使用 `tempfile` 隔离，且不得依赖执行顺序。
- 尽量在最接近领域逻辑的 crate 中测试，UI 边界只保留必要的组装测试。

运行全部测试：

```bash
cargo test --locked --all-targets --all-features
```

开发时可按模块筛选，例如：

```bash
cargo test --locked lyrics::
cargo test --locked mpris::
```

## 提交前检查

每条 Cargo 命令都应使用 `--locked`。提交 PR 前依次运行：

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
cargo docs
```

`cargo docs` 是仓库定义的 alias，会为整个 workspace 构建文档并将 warning 视为错误；需要在浏览器中查看时使用 `cargo docs-open`。

### 更新依赖与许可证清单

依赖变化会影响应用“开源许可证”页面。修改依赖和 `Cargo.lock` 后，安装 CI 使用的 cargo-about 版本并重新生成清单：

```bash
cargo install --locked --features cli --version 0.9.1 cargo-about
cargo about generate --locked --all-features data/licenses/about.hbs \
  --output-file data/licenses/dependencies.json
```

将更新后的 `data/licenses/dependencies.json` 与 `Cargo.lock` 一起提交。CI 会检查生成结果是否最新。

## Git 与 Pull Request

从最新的 `main` 创建主题分支，保持每个提交和 PR 目标单一。提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)：

```text
<type>(<scope>): <description>
```

描述使用小写、祈使语气的英文。常用 type 为 `feat`、`fix`、`refactor`、`test`、`docs`、`chore`；常用 scope 为 `app`、`lyrics`、`mpris`、`infra`、`ui`。

```text
fix(mpris): handle missing player position
feat(lyrics): parse enhanced lrc timestamps
docs(infra): clarify release checks
```

破坏性变更可在 type/scope 后加 `!`，并在正文写明 `BREAKING CHANGE:`。

PR 描述应包含：

- 问题背景与改动目标；
- 用户可见行为和重要设计取舍；
- 实际执行过的验证命令；
- 仍存在的限制或后续工作。

UI、交互或窗口布局变化请附真实截图或录屏。新增配置键、数据库 schema、系统依赖、网络接口或发布资源时，请在 PR 中明确标注。收到 review 后继续在同一分支更新，直至检查通过。

## 维护者：AUR 工作流

普通贡献无需执行本节。发布 AUR 包前，对应 Git tag 与 GitHub Release 资源必须已经存在，同时需要 `makepkg`、`namcap` 和 AUR SSH 权限。

在 Arch Linux 上构建当前源码包：

```bash
packaging/build-aur.sh --cleanbuild
```

产物会写入仓库根目录；脚本使用独立的 makepkg 工作目录，避免与 Rust 的 `src/` 冲突。

只准备并校验两个 AUR 包的本地文件：

```bash
packaging/release-aur.sh --prepare-only all 1.0.0
```

确认 diff 和版本后发布单个或全部包：

```bash
packaging/release-aur.sh floatlyrics 1.0.0
packaging/release-aur.sh floatlyrics-bin 1.0.0
packaging/release-aur.sh all 1.0.0
```

脚本会更新版本、校验和与 `.SRCINFO`，运行 PKGBUILD 检查，展示 diff，并在推送 AUR 前要求交互确认。

## 许可证

贡献者保留自己提交内容的版权。提交贡献即表示你有权提供相关内容，并同意其按照项目的 [GPL-3.0-or-later](LICENSE) 许可证分发。
