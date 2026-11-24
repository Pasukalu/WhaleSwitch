# WhaleSwitch 产品需求与路线图

调研日期 2026-11-24 · 当前实现 1.6.0 · 后续版本号用于表达优先级

## 用户与目标

| 用户 | 常见任务 | 可验证的结果 |
| --- | --- | --- |
| 同时使用两三种 AI 编程 CLI 的开发者 | 切换模型和环境，在机器间搬运配置 | 能在同一个界面核对目标文件、预览字段差异、选择要应用的值并撤销写入 |
| 维护共享仓库或 monorepo 的团队成员 | 排查队友为何看到不同技能或模型设置 | 能指出每个发现项的文件来源和适用工具，区分观察结果和待验证的运行时行为 |
| 经常切换本地模型与云端网关的开发者 | 排查地址、授权和模型列表问题 | 能区分连通性、授权和接口格式故障，并知道下一个该核对的配置项 |

WhaleSwitch 将本地配置管理、项目来源排查和恢复放在同一个桌面工作台。产品继续以 Codex CLI、Claude Code、OpenCode 的已验证能力为边界。1.2 已具备选字段应用档案、修订号检查和加密快照。项目技能盘点将同样的来源可见性扩展到仓库中的技能文件。

## GitHub Trending 与源码观察

样本来自 2026-11-24 的 [GitHub Trending 日榜](https://github.com/trending?since=daily)。榜单随时间变化。以下实现借鉴目录、数据结构和工作流设计，WhaleSwitch 的实现代码独立编写。

| 趋势项目 | 查阅到的实现或功能 | 对 WhaleSwitch 的产品决策 |
| --- | --- | --- |
| Superpowers | 每套流程有独立 `SKILL.md`，如 brainstorming 的文件头。插件清单单独维护发布元数据。 [技能文件](https://github.com/obra/superpowers/blob/main/skills/brainstorming/SKILL.md) · [插件清单](https://github.com/obra/superpowers/blob/main/.claude-plugin/plugin.json) | 以 `SKILL.md` 为盘点单位。路径和文件指纹分别表示来源与内容版本。 |
| Stitch Skills | 同一个技能目录还包含脚本、资源和参考文件，仓库文档公开了结构。 [目录结构](https://github.com/google-labs-code/stitch-skills#file-structure) · [Agent Skills 规范](https://agentskills.io/specification) | 盘点读取入口文件的有界字节。后续若加入导入，附件和命令要单独预览。 |
| Cursor Plugins | `agent-compatibility` 清单把兼容性扫描定位为独立工作流，并记录版本等元数据。 [兼容性插件清单](https://github.com/cursor/plugins/blob/main/agent-compatibility/.cursor-plugin/plugin.json) | 将路径候选、版本和实际运行结果分开呈现。后续兼容性说明跟随 CLI 版本维护。 |
| CLI-Anything | 日榜项目将工具能力通过 CLI 接口暴露给代理。 [项目](https://github.com/HKUDS/CLI-Anything) | 将只读、无密钥的 JSON 检查列入后续 CLI 方案，复用桌面核心及同一份适配器规则。 |

同类项目进一步说明了产品差异。CC Switch 的 live writer 委托工具各自的配置更新路径，WhaleSwitch 继续将写入限制在适配器规定的字段，同时在应用前提供字段级勾选和冲突提示。CCS 的档案注册表面向会话与账号管理，WhaleSwitch 的档案围绕可迁移的字段值和可恢复的本地修改。 [CC Switch live writer](https://github.com/farion1231/cc-switch/blob/main/src-tauri/src/services/provider/live.rs) · [CCS 档案注册表](https://github.com/kaitranntt/ccs/blob/main/src/auth/profile-registry.ts)

## 1.3 项目技能盘点（已实现）

**用户故事**　打开一个刚接手的仓库，在同一页查到它为三种 CLI 准备了哪些技能文件，并发现可能发生重名的目录。

**流程**　打开「项目技能」→ 用系统对话框选定目录 → 查看文件路径、候选 CLI、同名提示和 SHA-256 指纹。再次选择目录即可重新扫描。取消对话框保留上次结果。结果为空、路径不可访问和扫描不完整各有独立反馈。

| 根目录下的来源 | 候选 CLI | 文档证据 |
| --- | --- | --- |
| `.agents/skills/<folder>/SKILL.md` | Codex CLI、OpenCode | [Codex Skills](https://learn.chatgpt.com/docs/build-skills) · [OpenCode Skills](https://opencode.ai/docs/skills/#discovery) |
| `.claude/skills/<folder>/SKILL.md` | Claude Code、OpenCode | [Claude Code Skills](https://code.claude.com/docs/en/skills) · [OpenCode Skills](https://opencode.ai/docs/skills/#discovery) |
| `.opencode/skills/<folder>/SKILL.md` | OpenCode | [OpenCode Skills](https://opencode.ai/docs/skills/#discovery) |

| 验收项 | 通过条件 |
| --- | --- |
| 三种工具映射 | 在三处各放一份 `SKILL.md`，返回三条路径和表中的候选工具。浏览器演示版提示使用桌面应用。 |
| 同名与指纹 | 两个目录名仅 ASCII 大小写有别，且至少共享一个候选 CLI 时，两行都有同名提示。相同的文件字节产生相同指纹。修改正文后指纹变化。 |
| 授权与数据流 | WebView 只调用无路径参数的命令。Rust 从系统目录选择器获得根目录，仅返回路径、名称、候选工具、指纹与状态。应用发起零网络请求、零命令执行、零文件写入。 |
| 异常与边界 | 跳过符号链接、Windows 重解析点、不可读及非 UTF-8 文件。每个来源最多处理 256 个目录项，每份清单最多读取 128 KiB。不可读的来源或条目和触及扫描上限时有相应提示。 |
| 范围表达 | 界面将工具标签标为候选。同名提示基于目录名。只扫描所选根目录下表中来源的直接子目录。用户目录、父子工作区、OpenCode 平铺技能以及 YAML 语义和运行时优先级留待后续。 |
| 交付验证 | CI 在 Windows、macOS Intel、macOS Apple Silicon 和 Linux 跑核心测试并编译安装包。发版前在桌面以临时项目文件核对选择、取消、空状态和扫描结果。 |

## 1.4 配置来源与外部变更检测（已实现）

**用户故事**　同时用 Claude Code、Codex CLI 或编辑器改设置时，回来可以核对最新文件，合并手头的草稿，并明确知道下一次保存会影响哪个路径。

| 验收项 | 通过条件 |
| --- | --- |
| 目标与来源 | 显示三种 CLI 的用户级编辑路径和相关路径环境变量的设置状态。选择项目目录后，展示经过文档验证的固定候选路径、文件状态和识别到的字段名。项目文件只读。 |
| 同路径外部修改 | CLI 写入同一文件后，窗口获得焦点或手动重新扫描时，未编辑的界面会更新。存在草稿时显示旧值、当前值和草稿。同一字段同时变化时选择保留值，其他草稿修改继续保留。 |
| 删除与路径变化 | 删除文件时保留草稿。重新创建需要用户再次预览并保存。目标切到不同路径时，不合并旧草稿。OpenCode 同时出现两份全局配置时报告目标不明确。 |
| 错误与写入 | 配置语法损坏、路径不安全或读取失败时暂停写入并保留草稿。旧预览失效。合并草稿后保存仍需按最新修订预览，并先加密备份已有文件。 |
| 桌面验收 | 原生套件打开配置来源，选择和取消项目目录。在临时配置中分别验证空白草稿重载、三栏对比、路径迁移、文件删除和已有配置的快照恢复。 |

文档依据见 [Codex 配置基础](https://learn.chatgpt.com/docs/config-file/config-basic)、[Claude Code 设置来源](https://code.claude.com/docs/en/settings)、[OpenCode 配置](https://opencode.ai/docs/config/) 和 [OpenCode V1 路径覆盖定义](https://github.com/anomalyco/opencode/blob/v1.1.8/packages/opencode/src/flag/flag.ts)。

## 1.5 字段级恢复与 Windows 便携包（已实现）

Git 的选择性恢复提供了交互预览的参考。WhaleSwitch 以适配器已识别的配置字段为单位。在含未知键与注释的临时文件上，只恢复所选模型并保存一份加密的恢复前快照。已有非支持值跳过，清除操作需显式勾选，同一字段的快照值和当前修订需在写入前重新验证。 [Git restore](https://git-scm.com/docs/git-restore)

Windows x64 可从发布页下载免安装 ZIP，解压后在原目录启动。构建分开交付安装程序和便携包。应用数据目录和系统凭据管理器的边界见 [便携说明](PORTABLE.md)。竞争项目的发布形式见 [CC Switch Releases](https://github.com/farion1231/cc-switch/releases)。

## 1.6 多工具配置组合（已实现）

把日常模型和工作环境配置保存为方案。每种工具从一个档案取值，也可从 JSON 导入到另一台机器。预览同时列出本机路径、字段差异和工具级错误，清除操作需要主动勾选。

| 验收项 | 通过条件 |
| --- | --- |
| 选择性提交 | 只替换选中的字段，保留其他字段和注释。预览之后文件变化时停止提交。 |
| 全量预检 | 重复路径或过期目标停止写入。全部待写的已有文件各有一份验证成功的加密快照，之后才替换第一个文件。 |
| 失败补偿 | 保留逐文件状态和快照入口。遇到外部改写时保护新内容，并在方案页标记需要恢复的文件。 |
| 中断检查 | 再次打开方案页时比较未完成记录与实际修订，在最近执行中显示结果。 |
| 构建验证 | 提交 npm 与 Cargo 锁文件，CI 使用锁定安装及构建，发布任务校验四套安装包与 Windows 便携包。CI 解压并启动 Windows 便携包。 |

## 后续版本与优先级

| 优先级 | 功能或优化 | 用户收益与验收条件 |
| --- | --- | --- |
| P1 · 后续 | 本地操作记录与快照管理 | 保留时间、工具、动作、结果和快照编号。可选导出排除字段值、密钥、原始文件和绝对路径。可固定关键快照并预览清理范围。 |
| P0 · 并行工程优化 | 系统签名、公证和目录选择 | 发布包启动、真实系统密钥库及写入恢复已纳入复测工作流，复测结果通过 Sigstore 签名。待完成 Windows Authenticode、Apple Developer ID 签名与公证和原生目录选择交互复测。 [发布验证](RELEASE-VERIFICATION.md) |
| P1 · 1.7 | 技能变化与受控导入 | 比较两次选定项目的技能指纹，区分新增、删除和修改。导入先做本地清单预览并标记附件与覆盖风险，显式确认后才写允许的文本及路径。验收覆盖越界路径、同名文件、回滚与被禁止的脚本附件。 |
| P1 · 1.8 | 提供方连接诊断 | 针对经验证的 Models 接口给出 DNS、TLS、超时、401、429 和响应格式问题的下一步检查项。用本地测试服务覆盖状态，连接测试仅在明确点击后发起。 |
| P1 · 持续 | 可访问性与大列表效率 | 键盘可完成选目录、切换工具、筛选和差异确认。中英文、深浅主题与窄窗口都能读到路径和状态。用 300 个分散在三处的条目测试典型扫描，参考目标为指定 SSD 测试机首屏 p95 ≤ 1 秒，实际数字在测量后发布。 |
| P2 · 试验 | 只读 CLI 与 CI 校验 | 支持显式传入仓库目录和 `--json`。输出沿用桌面核心规则，不含密钥、完整正文及绝对路径。CLI 只有用户主动启用时才参与 CI，命名和退出码保持稳定。 |

版本排序依据是恢复风险、跨工具痛点和对现有适配器的复用程度。每项变更先在临时目录验证，再覆盖真实平台的安装和文件操作。基准数据来自公开测试夹具、CI 和自愿反馈，应用保持本地优先。

## 安全约束与模块边界

- 配置适配器按官方文档验证路径、字段和格式。未知配置保留原样。所有写入走修订校验和加密备份，系统密钥库不可用时停止覆盖已有文件。
- 技能盘点独立于配置写入事务和 SQLite。`skills` 核心模块复用文件访问校验，Tauri 负责取得用户选择的目录，React 仅渲染结果。运行时加载、生成代码和安装流程由相应 CLI 处理。
- 导入和分享以数据最小化为原则。API 密钥只能使用系统密钥库和服务商签发的凭据。网页登录凭据、命令输出和密钥均禁止进入日志或分享文件。
- 多文件组合采用逐文件原子替换与失败补偿，用户始终能看到每个文件的结果和可恢复快照。项目级写入须有完整的覆盖确认流程。
