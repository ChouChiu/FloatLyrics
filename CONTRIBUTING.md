# 为 FloatLyrics 贡献

感谢你帮助改进 FloatLyrics。本指南说明本地开发、质量要求、提交方式以及参与社区时
共同遵守的行为准则。

## 开发环境

FloatLyrics 是由三个 crate 组成的 Rust 2024 workspace，运行和界面测试需要 Linux Wayland、GTK4、
`gtk4-layer-shell` 和会话 D-Bus。单元测试不得依赖正在运行的 Spotify、D-Bus 或网络。

在 Arch Linux 上准备环境：

```bash
sudo pacman -S --needed base-devel git gtk4 gtk4-layer-shell openssl rust
git clone https://github.com/ChouChiu/FloatLyrics.git
cd FloatLyrics
cargo build --locked
cargo test --locked
```

使用其他发行版时，请安装等效的 C 工具链、`pkg-config`、GTK 4.12+、
`gtk4-layer-shell` 和 OpenSSL 开发包。稳定 Rust 工具链及 `rustfmt`、Clippy 组件由
`rust-toolchain.toml` 声明。

本地运行：

```bash
cargo run -- --debug
```

完整功能需要 Spotify、MPRIS 和支持 layer-shell 的 Wayland 合成器。

## 项目结构

- `floatlyrics-core`：路径、国际化、遥测、摘要和曲目指纹，不依赖 GTK 或 D-Bus。
- `floatlyrics-lyrics`：歌词模型、解析、搜索、时间轴以及 SQLite 缓存。
- `src/main.rs`、`src/lib.rs`：最小二进制入口、命令行解析和启动流程。
- `src/app.rs`、`src/app/`：Relm4 应用装配与消息流、控制器、展示状态和 GTK 界面。
- `src/mpris.rs`、`src/mpris/`：MPRIS 监听和播放位置同步。
- `src/config.rs`：应用配置及其原子写入逻辑。
- `data/`：桌面入口、图标和应用元数据等发布资源。

请把领域逻辑放入聚焦模块，并让 GTK、数据库、操作系统和网络边界保持清晰。

## 开发与代码风格

- 遵循 `rustfmt`，使用四空格缩进。
- 模块、函数和变量使用 `snake_case`，类型和 trait 使用 `PascalCase`，常量使用
  `SCREAMING_SNAKE_CASE`。
- 保持 `main.rs` 精简；可复用行为应放在领域模块中。
- 不要在业务逻辑或测试中依赖开发者本机路径、账户、网络或桌面会话。
- 改动界面文本时，同步维护 English、简体中文和繁體中文翻译；不要绕过本地化层
  直接加入新的用户可见字符串。翻译资源位于 `data/locale/*.json`，新增键时还需同步
  更新 `floatlyrics-core/src/i18n.rs` 中的 `define_text_keys!` 列表。
- 不提交 `target/`、本地 SQLite 文件、凭据或个人配置。

测试放在实现旁的 `#[cfg(test)]` 模块中，测试名应描述可观察行为，例如
`parses_enhanced_lrc`。文件系统或数据库测试使用临时目录隔离。

提交前运行：

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo build --locked --release
```

## 文档

生成三个 workspace crate 的文档，包括私有实现细节，但不生成第三方依赖文档：

```bash
cargo docs
```

使用 `cargo docs-open` 可生成相同文档并在默认浏览器中打开。两个别名均配置在
`.cargo/config.toml`，保证本地与 CI 使用相同参数；文档警告会作为错误处理。

只改动特定模块时可以先运行过滤测试（例如 `cargo test lyrics::`），但拉取请求提交前
仍需运行完整检查。

## 提交信息

使用 Conventional Commits：

```text
<type>(<scope>): <description>
```

描述使用简短、祈使、英文小写形式。常见类型为 `feat`、`fix`、`refactor`、`test`、
`docs` 和 `chore`；常见范围为 `app`、`lyrics`、`mpris`、`infra` 和 `ui`。例如：

```text
fix(mpris): handle missing player position
```

破坏性变更使用 `!`，或在正文加入 `BREAKING CHANGE:`。每个提交只处理一个清晰目的；
对非显然的设计决定在提交正文说明原因。

## 贡献流程

1. 搜索现有 issue 和 pull request，避免重复工作。较大功能或行为变化应先开 issue
   讨论范围和兼容性。
2. Fork 仓库并从最新 `main` 建立主题分支。
3. 实现最小、聚焦的改动，并为缺陷修复、解析、时间轴、缓存和 MPRIS 边界条件添加
   回归测试。
4. 运行格式化、Clippy、测试和 release 构建检查。
5. 提交 pull request，说明改动目的、用户可见行为和已运行的验证命令，并关联 issue。
6. 根据评审意见更新同一分支，保持讨论集中；合并前确保 CI 通过。

界面或窗口布局改动必须附真实运行截图或短录屏。新增配置键、数据库 schema、Linux
系统依赖或发布资源时，应在 pull request 中单独标明，并同步更新文档。

## 行为准则

参与 FloatLyrics 的 issue、评审、代码、聊天及其他项目空间时，请：

- 尊重不同背景、经验和观点，使用包容且专业的语言。
- 针对想法和代码提供具体、可执行的反馈，不进行人身攻击。
- 接受建设性意见，承认错误，并优先考虑社区和用户的长期利益。
- 尊重隐私，不公开他人的私人信息、通信或安全报告。

骚扰、歧视、威胁、侮辱、持续干扰讨论、未经许可公开私人信息，以及任何令人感到
不安全或不受欢迎的性化言行均不可接受。

如遇行为准则问题，请通过维护者 [ChouChiu](https://github.com/ChouChiu) 的 GitHub
个人资料所列私密联系方式报告，不要在公开 issue 中披露敏感细节。维护者会尽量保密、
避免利益冲突，并根据事件严重程度采取纠正、警告、临时限制或永久禁止参与等措施。
善意报告者不会因报告本身
受到报复。

## 许可证

你保留自己贡献的版权。提交贡献即表示你有权提交相关内容，并同意按项目的
[GPL-3.0-or-later](LICENSE) 许可证发布该贡献。
