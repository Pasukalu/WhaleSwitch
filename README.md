# WhaleSwitch

[![Desktop builds](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml)

**AI 编程工具的本地配置工作台 / Local configuration workbench for AI coding CLIs**

在同一个桌面应用里管理 Codex CLI、Claude Code 和 OpenCode。保存常用配置档案，预览每一项改动，并在覆盖配置时自动留下加密快照。

Manage Codex CLI, Claude Code, and OpenCode from one desktop app. Save reusable profiles, review field-level changes, and recover previous configs from encrypted snapshots.

[快速开始 / Quick start](#quick-start) · [支持字段 / Supported settings](#supported-settings) · [更新日志 / Changelog](CHANGELOG.md) · [架构与安全 / Architecture](docs/ARCHITECTURE.md)

## Quick start

1. 从源码运行，或在安装包发布后从仓库的 **Releases** 下载。构建流程覆盖 Windows x64（NSIS）、macOS Apple Silicon 和 Intel（DMG）、Ubuntu x64（DEB）。
2. 打开 WhaleSwitch 并扫描工具。侧栏显示 PATH 检测结果，配置页显示实际文件路径。已有配置可直接管理。
3. 修改字段后选择 **查看差异**。也可在 **配置档案** 保存当前配置或草稿，随后比较并应用。
4. 若需撤销修改，到 **快照** 比较并恢复完整历史文件。若当前文件无法解析，也可以从有效快照恢复。

Open the app, scan for tools, and review the resolved config path. Change fields or capture a named profile, then review the diff before applying. Rename or duplicate a profile from its row in **Profiles**. The Snapshots tab can restore a previous full file even if the current file is malformed.

## Supported settings

| Tool | User-level file | Editable settings |
| --- | --- | --- |
| Codex CLI | `$CODEX_HOME/config.toml` or `~/.codex/config.toml` | `model`, `model_provider`, `openai_base_url`, `model_reasoning_effort`, `model_reasoning_summary`, `model_verbosity` |
| Claude Code | `$CLAUDE_CONFIG_DIR/settings.json` or `~/.claude/settings.json` | `model`, `env.ANTHROPIC_BASE_URL` |
| OpenCode V1/V2 | `$XDG_CONFIG_HOME/opencode/opencode.json(c)` or `~/.config/opencode/opencode.json(c)` | `model` (`provider/model`), `default_agent` |

OpenCode also accepts an absolute JSON or JSONC path from `OPENCODE_CONFIG`. Ambiguous file candidates, inline configs and custom-directory overrides stop file selection. Project configuration, terminal overrides, and organization policies can take precedence over the edited user file.

The field list follows the [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference), [Claude Code settings](https://code.claude.com/docs/en/settings), [Claude Code environment variables](https://code.claude.com/docs/en/env-vars), and [OpenCode V1 migration guide](https://opencode.ai/docs/migrate-v1/). OpenCode's V1 and V2 provider blocks use different schemas. WhaleSwitch edits the shared fields listed here and preserves those provider blocks.

## Profile, snapshot and archive behavior

| Action | Effect |
| --- | --- |
| Save profile | Stores validated fields for the selected tool in SQLite, including defaults captured as unset. |
| Rename or duplicate | Rename keeps the same profile and captured values. Duplicate creates a separate profile from its saved fields. Both actions update the saved profile list. |
| Export or import a profile | Moves a saved profile as JSON for the same tool. Explicitly unset fields are preserved. Choose a name before saving the imported profile. |
| Apply profile | Starts with a read-only field diff, then uses the normal revision-checked write and backup flow. |
| Restore snapshot | Restores the full original bytes at the same tool and path. An existing file is backed up first. |
| Export or import active settings | Exports allowlisted configured values as JSON. Import stages an overlay for review. |

档案只包含支持字段，工具的原生登录仍由对应 CLI 管理。快照保存完整原文，因此使用系统密钥库中的专用密钥加密。密钥库不可用时，应用会拒绝覆盖已有配置。读取、保存与还原流程详见 [架构与安全](docs/ARCHITECTURE.md)。

在 **配置档案** 中，可以重命名档案，或将已保存的字段复制到新档案。操作只影响档案列表，明确应用并确认差异后才会写入工具配置。

### Move a profile between devices

1. Open **Profiles** and export the saved profile from its row.
2. On the other device, select the same CLI, then choose **Import profile** in **Profiles**.
3. Review the name and save the import. If the name is already taken, choose another. Use **Review changes** on the imported profile when you are ready to apply it.

档案 JSON 只包含工具类型、档案名称和支持的配置字段。显式清除的字段也会迁移。API 密钥、原始配置文件和加密快照由本机独立存储。

## Connection tests

- OpenAI-compatible and Anthropic APIs are tested with a Models request. Use an API key issued by the provider. The app stores it in Windows Credential Manager, macOS Keychain, or Linux Secret Service, scoped to the tool, API protocol and request URL.
- Keyless OpenAI-compatible probes are available for loopback services such as Ollama or LM Studio. The default is `http://127.0.0.1:11434/v1`; change the port or path as needed.
- Tests report response category, status, elapsed time and model count. They do not send a prompt. Automatic redirects and proxy inheritance are disabled.

## Develop and build

Install Node.js 22.13+, npm, stable Rust, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). On Linux, unlock the running Secret Service for native credential operations. On Windows, install C++ Build Tools and WebView2.

```sh
npm install
npm run tauri dev
npm test
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
npm run tauri build
```

`npm run dev` serves a browser preview at `http://localhost:1420`. It uses simulated configuration data in memory. Desktop filesystem writes and credential operations require Tauri.

GitHub Actions runs frontend and Rust tests and packages each target. Installers from successful runs are available in **Artifacts** on the [desktop builds page](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml). Each installer is paired with a SHA-256 file. A version-matching tag creates a draft release after all matrix builds succeed. The current packaging setup produces unsigned installers. Before publishing the draft, install and open the bundles, then verify the OS keyring and a save-and-restore cycle with disposable config files.

## Project references and license

[CC Switch](https://github.com/farion1231/cc-switch) informed the reusable profile and preview-before-write workflow. [CodexBar](https://github.com/steipete/CodexBar) informed the per-tool diagnostics presentation. WhaleSwitch's code and whale navigator artwork are independent. [MIT License](LICENSE).
