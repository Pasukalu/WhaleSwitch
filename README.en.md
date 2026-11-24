# WhaleSwitch

![WhaleSwitch, a local configuration workbench, with its original whale navigator mascot](docs/assets/whaleswitch-banner.jpg)

[![Desktop builds](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml)

**Local configuration for Codex CLI, Claude Code, and OpenCode.**

[简体中文](README.md) · [Download](https://github.com/Pasukalu/WhaleSwitch/releases) · [Code review](docs/CODE-REVIEW.md) · [Changelog](CHANGELOG.md)

Inspect each CLI's user-level edit target, review changes, save reusable profiles, and restore previous files from encrypted snapshots. The desktop app supports English and Simplified Chinese, plus dark, light, and system themes.

## Install

Choose a platform in [GitHub Releases](https://github.com/Pasukalu/WhaleSwitch/releases) and download the matching `.sha256` file. Build artifacts are also available from [GitHub Actions](https://github.com/Pasukalu/WhaleSwitch/actions/workflows/checks.yml).

| Platform | Installer | How to install |
| --- | --- | --- |
| Windows x64 | `*-windows-x64.exe` | Run the NSIS installer and open WhaleSwitch from Start. |
| macOS Apple Silicon | `*-macos-arm64.dmg` | Mount the disk image and drag WhaleSwitch to Applications. |
| macOS Intel | `*-macos-x64.dmg` | Mount the disk image and drag WhaleSwitch to Applications. |
| Ubuntu x64 | `*-linux-x64.deb` | Open it with the installer or use `sudo apt install` with the selected local `.deb`. |

Public prerelease installers are unsigned. Release notes list the verification completed for each build. Linux credential operations require an available and unlocked Secret Service. See the [checksum examples](README.md#安装).

## First run

1. Select a CLI and open **Config sources** to check the user-level edit target. The tool list reports PATH discovery.
2. Edit a supported field and select **Review changes**. Confirm to write through the revision check and encrypted backup flow.
3. Save a reusable profile, export it as JSON, or restore an earlier full file from **Snapshots**.
4. Returning to the window or scanning again checks for external edits. When you have a draft, compare the previous, current, and draft values. For the same readable file, choose which conflicting fields to carry into a new draft, then review again before saving.

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
| Project skills | Lists candidate tools, matching folder names and fingerprints for `SKILL.md` under documented project folders. |
| Profiles | Saves, renames, duplicates and transfers portable profiles. Applying a profile lets you select individual fields, with removals starting unchecked. |
| Snapshots | Captures, compares and restores full files at the original tool and path. |
| Connection test | Requests a model list from an explicitly selected OpenAI-compatible or Anthropic endpoint. Keyless OpenAI-compatible probes are limited to loopback targets. |

## Local storage

Snapshot contents are encrypted in SQLite with a key held in the operating system credential store. Provider-issued connection keys use the same OS store, scoped to a tool, protocol and request URL. Profile exports contain the validated settings, including explicit unset values. Project reports list recognized setting names and file status. The diagnostic request returns a status, elapsed time and model count. [Architecture and security](docs/ARCHITECTURE.md)

## Develop

Install Node.js 22.13+, npm, stable Rust and the platform dependencies. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

```sh
npm install
npm run tauri dev
npm run typecheck
npm test
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
npm run tauri build
```

`npm run dev` serves a browser demo with simulated data. The desktop app runs native file, snapshot, and credential operations.

CI checks the frontend and Rust core, builds four desktop installers, and generates their SHA-256 files. The release job verifies the complete asset set against the matching app version. [Workflow](.github/workflows/checks.yml)

[Architecture](docs/ARCHITECTURE.md) · [Code review](docs/CODE-REVIEW.md) · [Product roadmap](docs/PRODUCT-ROADMAP.md) · [MIT License](LICENSE)

WhaleSwitch is an independent open-source project with an original whale navigator mascot.
