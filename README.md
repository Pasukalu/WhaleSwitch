# WhaleSwitch

![WhaleSwitch 和小鲸鱼领航员。AI 编程工具的本地配置工作台](docs/assets/whaleswitch-banner.jpg)

[![桌面构建](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml)

**管理 Codex CLI、Claude Code 和 OpenCode 的本地配置。**

[English](README.en.md) · [下载安装包](https://github.com/Pasukalu/WhaleSwitch/releases) · [源码审查](docs/CODE-REVIEW.md) · [更新日志](CHANGELOG.md)

WhaleSwitch 用于查清配置从哪里读入、比较改动、切换多工具配置方案以及恢复历史版本。支持简体中文和 English，外观可选择深色、浅色或跟随系统。

## 安装

在 [GitHub Releases](https://github.com/Pasukalu/WhaleSwitch/releases) 选择版本和平台，同时下载同名的 `.sha256` 文件。持续构建的包也可在 [GitHub Actions](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml) 的 Artifacts 中下载。

| 系统 | 选择的安装包 | 使用方式 |
| --- | --- | --- |
| Windows x64 | `*-windows-x64.exe` | 运行 NSIS 安装程序，再从开始菜单启动。 |
| Windows x64 便携 | `*-windows-x64-portable.zip` | 完整解压后运行 `WhaleSwitch.exe`。 [便携说明](docs/PORTABLE.md) |
| macOS Apple Silicon | `*-macos-arm64.dmg` | 挂载 DMG，将 WhaleSwitch 拖到 Applications。 |
| macOS Intel | `*-macos-x64.dmg` | 挂载 DMG，将 WhaleSwitch 拖到 Applications。 |
| Ubuntu x64 | `*-linux-x64.deb` | 用软件安装器打开，或在下载目录用 `sudo apt install` 安装选定的 `.deb`。 |

发布页记录各版本的系统签名状态和复测范围。v1.6.0 使用 Sigstore 对复测记录和五个下载包的摘要签名。系统签名及验证命令见 [发布验证](docs/RELEASE-VERIFICATION.md)。Linux 的密钥库操作需要桌面会话中的 Secret Service 可用并解锁。

<details>
<summary>校验安装包 SHA-256</summary>

在下载目录执行相应命令。Windows 需要将命令输出与校验文件中的哈希比对。其他命令直接使用校验文件验证。

```powershell
Get-FileHash .\WhaleSwitch-1.7.0-windows-x64.exe -Algorithm SHA256
Get-FileHash .\WhaleSwitch-1.7.0-windows-x64-portable.zip -Algorithm SHA256
```

```sh
# macOS Apple Silicon
shasum -a 256 -c WhaleSwitch-1.7.0-macos-arm64.dmg.sha256

# Ubuntu
sha256sum -c WhaleSwitch-1.7.0-linux-x64.deb.sha256
```

</details>

## 第一次使用

1. **确认目标。** 打开应用，选择一个 CLI。`扫描工具` 检查 PATH，**配置来源** 显示实际的用户级编辑路径。PATH 中找不到工具时，仍可查看已存在并受支持的配置文件。
2. **编辑和预览。** 在 **配置** 修改已支持的字段，选择 **查看差异** 并确认。已有文件会先创建加密快照，再执行修订校验和原子替换。
3. **复用或恢复。** 在 **配置档案** 保存和导入日常设置。需要撤销某项改动时，打开 **快照 → 按字段恢复**，预览并勾选要恢复的设置。
4. **处理外部改动。** 切回窗口或手动扫描后，有草稿时会列出旧值、文件当前值和草稿。同一路径下可逐项合并，随后重新预览提交。

### 切换多工具配置

1. 在 **配置档案** 保存各工具的常用设置，再到 **配置方案** 选择最多三个工具的档案并保存方案。也可以导入方案 JSON。
2. 打开 **预览方案**，核对每种工具的本地目标路径并勾选字段。清除字段需主动勾选。有外部变更时重新预览。
3. 选择 **应用所选字段**。替换第一个文件之前，应用会验证所有待写的已有文件的加密快照。出现失败或进程中断时，可在 **最近执行** 核对逐工具状态和可用快照。

### 跟踪项目技能

1. 打开 **项目技能**，用系统目录选择器指定项目。完整扫描后点 **保存基线**。
2. 修改项目技能后刷新扫描，在 **基线变更** 中核对新增、修改和移除。基线仅比较 `SKILL.md` 的内容指纹。
3. 在 **从本地导入** 选择目标技能位置，再选择含 `SKILL.md` 的来源目录。逐项展开原文并核对目标路径后，点击 **导入到项目**。遇到同名目标时选另一个位置。

导入包含 `SKILL.md` 和 `references` 中受支持的 `.md`、`.txt`，其余附件计入跳过数量。导入前重验来源指纹，先写参考文档，最后写入口文件。 [技能规范](https://agentskills.io/specification)

## 工具与配置范围

| 工具 | 用户级文件 | 可编辑字段 |
| --- | --- | --- |
| Codex CLI | `$CODEX_HOME/config.toml` 或 `~/.codex/config.toml` | `model`、`model_provider`、`openai_base_url`、`model_reasoning_effort`、`model_reasoning_summary`、`model_verbosity` |
| Claude Code | `$CLAUDE_CONFIG_DIR/settings.json` 或 `~/.claude/settings.json` | `model`、`env.ANTHROPIC_BASE_URL` |
| OpenCode V1/V2 | `$XDG_CONFIG_HOME/opencode/opencode.json(c)` 或 `~/.config/opencode/opencode.json(c)` | `model`（`provider/model`）和 `default_agent` |

OpenCode V1 可通过 `OPENCODE_CONFIG` 指定绝对 JSON 或 JSONC 路径。存在内联配置、自定义目录或多个全局候选文件时，应用暂停定位可写目标。工作目录、启动参数、信任设置和组织策略可能影响 CLI 的实际生效值。 [Codex 配置](https://learn.chatgpt.com/docs/config-file/config-reference) · [Claude Code 设置](https://code.claude.com/docs/en/settings) · [OpenCode 配置](https://opencode.ai/docs/config/)

| 页面 | 可执行的操作 |
| --- | --- |
| 配置来源 | 查看编辑目标、相关环境变量设置状态和项目配置候选文件。项目读取为只读，仅返回状态与支持字段名。 |
| 项目技能 | 检查 `.agents/skills`、`.claude/skills` 和 `.opencode/skills`。保存基线、查看变更、筛选路径，并在预览后导入本地技能。 |
| 配置档案 | 保存、重命名、复制、导出和导入档案。应用前预览差异，带清除动作的字段初始不勾选。 |
| 配置方案 | 保存或导入跨工具的字段组合。一次预览所有目标，按需提交并查看每次执行的恢复入口。 |
| 快照 | 手动创建、比较、按字段恢复和恢复整份文件。按字段恢复会保留当前文件中的其他字段和未知配置。语法损坏时可恢复整份文件。 |
| 连接测试 | 对 OpenAI 兼容或 Anthropic 端点发起模型列表请求，显示状态、耗时和模型数。OpenAI 兼容的回环服务可使用无密钥模式。 |

## 数据与密钥

- 快照以加密形式存入本地 SQLite，快照密钥和连接测试 API 密钥保存在操作系统安全存储中。已有配置需要通过加密备份校验后才会写入。
- 配置档案的 JSON 仅包含工具、名称与支持字段，保留显式清除值。换设备时可导入档案，再勾选需要迁移的字段。
- 方案 JSON 包含版本、名称和每个工具的支持字段。密钥、本机配置路径及历史快照保存在本地。
- 连接测试使用服务商签发的 API 密钥，仅在手动触发时发送请求。密钥按工具、协议及具体请求 URL 隔离，重定向与系统代理继承均已禁用。
- 项目盘点读取固定候选文件。技能基线只保存项目路径的指纹、相对清单路径和内容指纹。技能原文在用户选择的导入目录中审阅。 [架构与安全](docs/ARCHITECTURE.md)

## 开发与贡献

需要 Node.js 22.13+、npm、stable Rust 和 Tauri 2 平台依赖。 [Tauri 前置条件](https://v2.tauri.app/start/prerequisites/)

```sh
npm ci
npm run tauri dev
npm run typecheck
npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
npm run tauri build -- -- --locked
```

`npm run dev` 启动使用模拟数据的浏览器预览。实际目录扫描、加密快照和密钥库操作在桌面版内验证。

CI 按锁文件安装依赖，在 Windows x64、macOS Apple Silicon、macOS Intel 和 Ubuntu x64 上运行类型检查及前后端测试，随后生成安装包。Windows 任务还会解压并启动便携 ZIP。维护者可用匹配版本的标签生成发布草稿，或从 `main` 上的提交推送 `release/vX.Y.Z` 分支发布预发行包。发布任务校验版本、五套下载包及其 SHA-256 文件。 [构建工作流](.github/workflows/checks.yml)

- 代码结构见 [架构](docs/ARCHITECTURE.md)，维护发现见 [1.4 源码审查](docs/CODE-REVIEW.md)。
- 功能目标与开源参考见 [产品需求与路线图](docs/PRODUCT-ROADMAP.md)。
- 反馈时可提供系统、架构、应用版本及错误类型。请在提交前去除密钥和机器路径。 [提交问题](https://github.com/Pasukalu/WhaleSwitch/issues/new)

WhaleSwitch 是独立的开源项目，横幅使用项目的原创鲸鱼娘吉祥物。 [MIT License](LICENSE)
