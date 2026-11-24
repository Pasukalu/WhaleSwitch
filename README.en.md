# WhaleSwitch

![WhaleSwitch, a local configuration workbench, with its original whale navigator mascot](docs/assets/whaleswitch-banner.jpg)

[![Desktop builds](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml)

**Local configuration for Codex CLI, Claude Code, and OpenCode.**

[简体中文](README.md) · [Download](https://github.com/Pasukalu/WhaleSwitch/releases) · [Code review](docs/CODE-REVIEW.md) · [Changelog](CHANGELOG.md)

Inspect each CLI's user-level edit target, review changes across tools, and restore previous files from encrypted snapshots. The desktop app supports English and Simplified Chinese, plus dark, light, and system themes.

## Install

Choose a platform in [GitHub Releases](https://github.com/Pasukalu/WhaleSwitch/releases) and download the matching `.sha256` file. Build artifacts are also available from [GitHub Actions](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml).

| Platform | Installer | How to install |
| --- | --- | --- |
| Windows x64 | `*-windows-x64.exe` | Run the NSIS installer and open WhaleSwitch from Start. |
| Windows x64 portable | `*-windows-x64-portable.zip` | Extract all files and run `WhaleSwitch.exe`. [Portable guide](docs/PORTABLE.md#english) |
| macOS Apple Silicon | `*-macos-arm64.dmg` | Mount the disk image and drag WhaleSwitch to Applications. |
| macOS Intel | `*-macos-x64.dmg` | Mount the disk image and drag WhaleSwitch to Applications. |
| Ubuntu x64 | `*-linux-x64.deb` | Open it with the installer or use `sudo apt install` with the selected local `.deb`. |

Release notes identify each version's platform signing status and retest coverage. Sigstore statements cover the build provenance and retest results of the five downloadable packages. Linux credential operations require an available and unlocked Secret Service. See [release verification](docs/RELEASE-VERIFICATION.md) and the [checksum examples](README.md#安装).

## First run

1. Select a CLI and open **Config sources** to check the user-level edit target. The tool list reports PATH discovery.
2. Edit a supported field and select **Review changes**. Confirm to write through the revision check and encrypted backup flow.
3. Save a reusable profile and export it as JSON. To undo specific edits, choose **Snapshots → Restore fields**, review the diff, and select the fields to restore.
4. Returning to the window or scanning again checks for external edits. When you have a draft, compare the previous, current, and draft values. For the same readable file, choose which conflicting fields to carry into a new draft, then review again before saving.

### Switch settings across tools

1. Save the settings you use for each tool in **Profiles**. In **Plans**, select up to three tool profiles and save the plan, or import a plan JSON.
2. Choose **Preview plan** and check each local target path. Select the fields to apply and opt in to any removals. Preview again if a file changes.
3. Choose **Apply selected fields**. WhaleSwitch verifies an encrypted backup of every selected existing file before replacing the first file. Use **Recent runs** to check per-tool results and available recovery snapshots after a failure or interruption.

### Track project skills

1. Open **Project skills** and choose a project directory. Select **Save baseline** after a complete scan.
2. Rescan after changing project skills. **Changes from baseline** lists added, modified, and removed `SKILL.md` files by content fingerprint.
3. In **Import from local folder**, select a destination and choose a folder containing `SKILL.md`. Expand each listed file and check the project path before selecting **Import into project**. An existing destination stops the import.

Imports include `SKILL.md` and supported `.md` and `.txt` files in `references`. The preview reports skipped attachments. WhaleSwitch rechecks the reviewed fingerprints and places the manifest after its references. [Agent Skills specification](https://agentskills.io/specification)

### Diagnose a connection

In **Connection test**, choose the API protocol and enter the base or Models URL. Store a provider-issued API key, or enable keyless mode for an OpenAI-compatible loopback service. Select **Test connection** to send a single model-list request.

The result shows the stage, elapsed time, any HTTP status, and a relevant check. For example, 401 points to the credential or protocol and 403 points to permissions or gateway policy. For 429 and 503, WhaleSwitch displays a valid `Retry-After` delay when available. Open **Redacted diagnostic summary** to copy the status fields for a support report. [OpenAI Models](https://developers.openai.com/api/reference/resources/models/methods/list) · [Anthropic Models](https://platform.claude.com/docs/en/api/models/list)

## Supported configuration

| CLI | User-level file | Editable settings |
| --- | --- | --- |
| Codex CLI | `$CODEX_HOME/config.toml` or `~/.codex/config.toml` | `model`, `model_provider`, `openai_base_url`, `model_reasoning_effort`, `model_reasoning_summary`, `model_verbosity` |
| Claude Code | `$CLAUDE_CONFIG_DIR/settings.json` or `~/.claude/settings.json` | `model`, `env.ANTHROPIC_BASE_URL` |
| OpenCode V1/V2 | `$XDG_CONFIG_HOME/opencode/opencode.json(c)` or `~/.config/opencode/opencode.json(c)` | `model` in `provider/model` form, `default_agent` |

For OpenCode V1, WhaleSwitch also supports an absolute JSON or JSONC path in `OPENCODE_CONFIG`. Competing files, inline configuration, and custom directories can pause target selection. Launch flags, project trust and managed settings can affect what a particular CLI session loads. [Codex config](https://learn.chatgpt.com/docs/config-file/config-reference) · [Claude Code settings](https://code.claude.com/docs/en/settings) · [OpenCode config](https://opencode.ai/docs/config/)

| Workspace page | What it does |
| --- | --- |
| Config sources | Displays the edit target, path environment flags and read-only candidate files in a selected project. |
| Project skills | Scans the three documented tool locations, compares against a local baseline, filters paths, and previews local imports. |
| Profiles | Saves, renames, duplicates and transfers portable profiles. Applying a profile lets you select individual fields, with removals starting unchecked. |
| Plans | Saves or imports a combination of tool settings, reviews and applies selected fields, and links each run to available backups. |
| Snapshots | Captures and compares snapshots at the original tool and path. Restore selected fields, or restore a full file to recover from invalid syntax. |
| Connection test | Diagnoses DNS, TLS, HTTP, and response failures for an explicitly selected Models endpoint and provides a shareable summary. |

## Local storage

Snapshot contents are encrypted in SQLite with a key held in the operating system credential store. Provider-issued connection keys use the same OS store, scoped to a tool, protocol and request URL. Profile and plan exports contain validated settings, including explicit unset values. Local target paths and snapshot history stay on the device. Project reports list recognized setting names and file status. The diagnostic request returns a status, elapsed time and model count. [Architecture and security](docs/ARCHITECTURE.md)

## Develop

Install Node.js 22.13+, npm, stable Rust and the platform dependencies. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

```sh
npm ci
npm run tauri dev
npm run typecheck
npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
npm run tauri build -- -- --locked
```

`npm run dev` serves a browser demo with simulated data. The desktop app runs native file, snapshot, and credential operations.

CI uses the npm and Cargo lockfiles to check the frontend and Rust core, builds four desktop installers and a Windows portable ZIP, and generates their SHA-256 files. The Windows job extracts and launches the portable package. The release job verifies the complete asset set against the matching app version. [Workflow](.github/workflows/checks.yml)

[Architecture](docs/ARCHITECTURE.md) · [Code review](docs/CODE-REVIEW.md) · [Product roadmap](docs/PRODUCT-ROADMAP.md) · [MIT License](LICENSE)

WhaleSwitch is an independent open-source project with an original whale navigator mascot.
