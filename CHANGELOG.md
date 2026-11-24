# Changelog

## 1.1.0

- Export and import a named profile as JSON, preserving explicit unset values.
- Review the profile name before saving an import and choose another name if it already exists.
- File transfer validates the tool and fields and uses the existing atomic export path.

## 1.0.0

- Named configuration profiles per tool. Capture the current file or a draft, review field-level changes and apply through the existing snapshot-and-save path.
- Rename and duplicate saved profiles independently of the live CLI configuration.
- Codex reasoning summary and output verbosity, Claude Code gateway URL, and OpenCode default agent controls.
- Keyless loopback Models probes for OpenAI-compatible local services with redirects disabled.
- Installer CI for Windows x64, macOS Apple Silicon and Intel, and Ubuntu x64. Each artifact includes its own SHA-256 file. Matching tagged builds can prepare a GitHub draft release.
- Aligned the Tauri runtime, JavaScript API, and CLI on the 2.12 release line.
- Tests for profile persistence, revisions, nested JSON edits, and loopback authentication policy.

## 0.1.0

- Initial Codex CLI, Claude Code, and OpenCode configuration workbench.
- Configuration diff, encrypted snapshots, restore, portable JSON archives, and credential-scoped connectivity tests.
