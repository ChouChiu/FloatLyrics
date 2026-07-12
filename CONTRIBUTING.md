# 贡献指南

感谢你为 FloatLyrics 贡献代码、文档或反馈。本指南涵盖开发环境、代码规范、提交约定与协作流程。

## 开发环境

FloatLyrics 是 Rust 2024 workspace，由三个 crate 组成。功能运行与 UI 测试需要 Linux
Wayland、GTK4 (≥ 4.12)、gtk4-layer-shell 和会话 D-Bus。单元测试不得依赖 Spotify、D-Bus
或网络。

准备环境（Arch Linux）：

```bash
sudo pacman -S --needed base-devel git gtk4 gtk4-layer-shell openssl rust
git clone https://github.com/ChouChiu/FloatLyrics.git
cd FloatLyrics
cargo build --locked
cargo test --locked
```

其他发行版请安装等效的 C 工具链、pkg-config、GTK 4.12+、gtk4-layer-shell 和 OpenSSL
开发包。Rust 稳定工具链要求由 `rust-toolchain.toml` 自动处理。

本地运行：

```bash
cargo run -- --debug
```

## 项目结构

```
floatlyrics (src/)              应用入口 + CLI + Relm4/GTK4 + MPRIS
  ├─ floatlyrics-lyrics/        歌词模型、解析、搜索、时间轴、SQLite 缓存
  └─ floatlyrics-core/          路径、i18n、遥测、曲目指纹
```

依赖方向自上而下。域逻辑放在上层，GTK、D-Bus、数据库和网络边界放在相应模块中。

关键目录：

| 路径 | 说明 |
|---|---|
| `src/lib.rs` | CLI 解析与启动流程 |
| `src/app.rs` | Relm4 应用组装 |
| `src/app/` | 控制器、展示模型、视图、设置、手动搜索、关于页面 |
| `src/mpris/` | MPRIS D-Bus 监听与位置同步 |
| `src/config.rs` | 配置读写（原子 temp + rename） |
| `floatlyrics-lyrics/src/` | LRC/QRC 解析、QQ/网易搜索、时间轴 |
| `floatlyrics-core/src/` | i18n、app 路径、telemetry |
| `data/locale/` | 中英文翻译 JSON |

## 代码规范

- `rustfmt`，四空格缩进
- `snake_case` 用于模块、函数、变量；`PascalCase` 用于类型与 trait；`SCREAMING_SNAKE_CASE` 用于常量
- 保持 `main.rs` 最小化，可复用逻辑归入域模块
- **勿在业务逻辑或测试中硬编码本机路径、账户、网络或桌面会话**
- 界面文本必须维护三语翻译（English / 简体中文 / 繁體中文），详见下方 [i18n](#i18n) 章节

## i18n

每个面向用户的字符串必须存在于以下三个文件中：

```
data/locale/en.json
data/locale/zh-CN.json
data/locale/zh-TW.json
```

新增键时，还需同步更新 `floatlyrics-core/src/i18n.rs` 中的 `define_text_keys!` 宏。

启动时 `i18n::validate_catalogues()` 会自动验证所有 locale 的键是否齐全。i18n 相关
测试位于 `floatlyrics-core/src/test/i18n_test.rs`。

## 测试

测试位于 `#[cfg(test)]` 模块中，通过 `#[path = "test/xxx_test.rs"]` 指向 `src/test/`
下的文件。

**原则：**

- 测试名描述可观察行为，如 `parses_enhanced_lrc`
- 不得依赖 Spotify、D-Bus、网络或工作区外的路径
- 文件系统 / 数据库测试使用 `tempfile` 隔离
- 每个 bug 修复应有对应的回归测试

CI 执行命令：

```bash
cargo test --locked --all-targets --all-features
```

更新依赖后，还需要重新生成应用内的开源许可证清单：

```bash
cargo install --locked --features cli --version 0.9.1 cargo-about
cargo about generate --locked --all-features data/licenses/about.hbs \
  --output-file data/licenses/dependencies.json
```

生成结果必须和 `Cargo.lock` 一起提交。CI 会检查许可证清单是否为最新状态。

筛选测试：

```bash
cargo test lyrics::
cargo test mpris::
```

## 提交流程

提交前务必通过以下检查：

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
```

### 提交信息

采用 [Conventional Commits](https://www.conventionalcommits.org/)，格式：

```
<type>(<scope>): <description>
```

- 描述用小写英文，祈使语气，简洁明了
- 破坏性变更使用 `!`，或正文中写 `BREAKING CHANGE:`

**常用类型：** `feat` `fix` `refactor` `test` `docs` `chore`

**常用范围：** `app` `lyrics` `mpris` `infra` `ui`

示例：

```
fix(mpris): handle missing player position
feat(app): add play-pause control
```

每个提交仅包含一个明确目的。必要时在正文中解释设计决策。

### 协作流程

1. 搜索已有 issue/PR，避免重复。**重大功能或行为变更请先开 issue 讨论**
2. Fork 仓库，从最新 `main` 创建分支
3. 实现最小、聚焦的改动，补全测试
4. 本地跑完格式化、Clippy、测试和 release 构建
5. 提交 PR，说明改动目的、用户可见行为和验证方式
6. 根据 review 在同一分支更新，确保 CI 通过后合并

UI / 窗口布局变更请附真实截图或录屏。新增配置键、数据库 schema、系统依赖或发布资源时
请在 PR 中标明。

## 许可证

贡献者保留自身代码的版权。提交即表示你拥有提交内容的权利，并同意按项目
[GPL-3.0-or-later](LICENSE) 分发。
