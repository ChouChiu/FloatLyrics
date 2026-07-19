# 为 FloatLyrics 做贡献

感谢你愿意改进 FloatLyrics。代码、测试、文档、翻译、问题报告与设计建议都很有价值。本指南说明如何搭建环境、确定改动归属、验证结果并提交便于审阅的 Pull Request。

## 开始之前

请先搜索现有 [Issues](https://github.com/ChouChiu/FloatLyrics/issues) 和 [Pull Requests](https://github.com/ChouChiu/FloatLyrics/pulls)，确认没有重复工作。

- 小型修复、测试和文档改进可以直接提交 PR。
- 新功能、行为变化、架构调整或新增系统依赖，请先开 issue 对齐方向。
- 安全问题不要发布在公开 issue；请通过仓库所有者提供的私密联系方式报告。

## 准备开发环境

项目使用 Rust 2024 edition，最低支持 Rust 1.93；React 歌词前端使用 Bun 1.3.14、TypeScript 和 Biome。`rust-toolchain.toml` 会选择 stable，并安装 `rustfmt`、Clippy、Rust 源码与 rust-analyzer。Bun 请按[官方安装说明](https://bun.com/docs/installation)安装。

运行完整应用需要 Linux Wayland、支持 layer-shell 的合成器、会话 D-Bus、GTK 4.12+、gtk4-layer-shell、WebKitGTK 6.0 和 OpenSSL。运行单元测试则不需要桌面会话。

Arch Linux 可直接安装所需依赖：

```bash
sudo pacman -S --needed base-devel git gtk4 gtk4-layer-shell webkitgtk-6.0 openssl rust
```

Fedora：

```bash
sudo dnf install gcc git gtk4-devel gtk4-layer-shell-devel webkitgtk6.0-devel openssl-devel rust cargo
```

Debian / Ubuntu 25.04+：

```bash
sudo apt install build-essential git libgtk-4-dev libgtk4-layer-shell-dev libwebkitgtk-6.0-dev libssl-dev rustc cargo
```

获取代码并验证环境：

```bash
git clone https://github.com/ChouChiu/FloatLyrics.git
cd FloatLyrics
bun install --frozen-lockfile
bun run check
bun run typecheck
bun test
cargo build --locked
cargo test --locked --all-targets --all-features
```

在支持 layer-shell 的 Wayland 会话中启动开发版本：

```bash
cargo run --locked -- --debug
```

`--debug` 会启用详细 tracing 日志，并非只表示使用 debug profile。GTK 由 Relm4 初始化，请勿额外调用 `gtk::init()`。

修改 React 歌词前端后，可运行 `bun run format`，通过 `biome check --write .` 自动格式化并应用安全修复。

### 构建流水线

`build.rs` 在 Cargo 编译前自动执行以下步骤：

1. 安装前端依赖：`bun install --frozen-lockfile`，根据 `bun.lock` 还原精确版本。
2. 构建 React 歌词页：`bun run build:lyrics`，入口为 `src/frontend/view/lyrics/lyrics.html`，输出为内嵌所有 JS/CSS 的单个 HTML 文件到 Cargo 的 `OUT_DIR`。同时生成 `frontend-dependencies.json` 用于开源许可页面。
3. 编译时嵌入：Rust 代码通过 `include_str!(concat!(env!("OUT_DIR"), "/lyrics.html"))` 将构建产物嵌入二进制。

因此只需运行 `cargo build --locked`，无需手动执行 Bun 命令——`build.rs` 自动处理一切。但如果只修改了 React 前端，在 Wayland 会话外也可单独执行 `bun run build:lyrics` 来验证前端构建是否成功。

## 理解工作区

```text
floatlyrics (src/)              CLI 与应用层
  ├─ frontend/                  Relm4/GTK4/WebKit 界面与 UI 适配器
  ├─ backend/                   播放状态、歌词协调、缓存与 MPRIS
  ├─ shared/                    配置与前后端共享数据协议
  └─ floatlyrics-lyrics/        歌词模型、LRC/QRC 解析、搜索、SQLite 缓存
       └─ floatlyrics-core/     路径、i18n、遥测、曲目指纹
```

依赖只能沿图中方向自上而下。根 crate 内部遵循 `frontend → backend → shared`，`frontend` 也可直接读取 `shared`；后端不得依赖 GTK、Relm4、WebKit 或前端消息。与 GTK、D-Bus、网络或数据库无关的领域逻辑，应放入能够承载它的最底层 crate，避免让可复用逻辑依赖应用边界。

| 路径 | 职责 |
|---|---|
| `src/lib.rs` | CLI 参数与应用启动流程 |
| `src/frontend.rs`、`src/frontend/` | Relm4 应用、GTK/WebKit 视图、设置页和 UI 适配器 |
| `src/backend.rs`、`src/backend/` | 播放控制、歌词与搜索服务、缓存协调和 MPRIS |
| `src/shared.rs`、`src/shared/` | 配置模型与跨层展示协议 |
| `floatlyrics-lyrics/src/` | 歌词解析、时间轴、搜索提供方与缓存 |
| `floatlyrics-core/src/` | 跨 crate 的基础能力与稳定领域类型 |
| `data/locale/` | 三种语言的 JSON 文案目录 |
| `packaging/` | 打包安装脚本、AUR 元数据与发布自动化 |

### React 歌词前端架构

前端位于 `src/frontend/view/lyrics/`，渲染引擎基于 `@applemusic-like-lyrics/react`（AMLL）和 PixiJS：

- `bridge.ts`：在 `window.floatLyrics` 上挂载 JS bridge，接收 Rust 侧通过 WebKit `evaluate_script` 推送的 `LyricsCommand`。
- `store.ts`：`LyricsViewState` 维护双槽位（slot 0/1）的 ping-pong 状态机。切歌时切换 slot 实现交叉淡入淡出。
- `app.tsx`：React 组件，通过 `useSyncExternalStore` 订阅 store，将 `LyricsDocument` 转换为 AMLL lines 格式驱动 `LyricPlayer`。
- `types.ts`：定义 Rust ↔ JS 共享的数据类型（`TimedSyllable`、`LyricsFrame`、`LyricsCommand` 等）。

修改前端后务必运行 `bun run typecheck` 和 `bun test` 验证 TypeScript 类型和组件逻辑。

## 实现约定

- 使用 `rustfmt` 默认格式和标准 Rust 命名规范。
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

开发时可按模块筛选，常用命令：

```bash
cargo test --locked -p floatlyrics-core i18n            # 本地化
cargo test --locked -p floatlyrics-lyrics parsing::     # 歌词解析
cargo test --locked -p floatlyrics-lyrics timeline::    # 时间轴
cargo test --locked -p floatlyrics-lyrics cache::       # 缓存与 schema
cargo test --locked -p floatlyrics controller::         # 播放控制
cargo test --locked -p floatlyrics manual_search::      # 手动搜索
cargo test --locked -p floatlyrics mpris::              # MPRIS
cargo test --locked -p floatlyrics config::             # 配置
```

## 提交前检查

每条 Cargo 命令都应使用 `--locked`。提交 PR 前依次运行：

```bash
bun install --frozen-lockfile
bun run check
bun run typecheck
bun test
bun run build:lyrics
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

JavaScript 依赖必须通过 `bun add` 或 `bun add --dev` 修改，并将 `package.json` 与 `bun.lock` 一起提交。React 歌词构建会根据实际 bundle 自动生成运行时 npm 许可证数据到 `target/lyrics-web/frontend-dependencies.json`；该文件用于检查，不应提交。

## 调试技巧

运行带 tracing 日志的开发版本：

```bash
cargo run --locked -- --debug
```

`--debug` 启用详细日志。可通过 `RUST_LOG` 环境变量进一步控制日志级别：

```bash
RUST_LOG=floatlyrics=debug cargo run --locked -- --debug
RUST_LOG=floatlyrics_lyrics=trace cargo run --locked -- --debug
```

i18n 测试可使用 `FLOATLYRICS_LOCALE_DIR` 指向自定义目录，避免污染系统数据：

```bash
FLOATLYRICS_LOCALE_DIR=./data/locale cargo test --locked -p floatlyrics-core i18n
```

使用 `--config` 参数可指定独立配置文件进行功能测试，不影响默认配置。

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

源码版和预编译版 AUR 元数据分别位于 `packaging/aur/floatlyrics/` 与
`packaging/aur/floatlyrics-bin/`。构建产物会写入仓库根目录；脚本使用独立的
makepkg 工作目录，避免与 Rust 的 `src/` 冲突。

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

贡献者保留自己提交内容的版权。提交贡献即表示你有权提供相关内容，并同意其按照项目的 [AGPL-3.0-only](LICENSE) 许可证分发。
