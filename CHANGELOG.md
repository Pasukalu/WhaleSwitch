# Changelog

## 1.4.0

- Inspect the user-level edit target, known path environment variables and read-only project configuration candidates for Codex CLI, Claude Code and OpenCode.
- Recheck the selected configuration on window focus and explicit refresh. Clean editors reload after external edits, file deletion, or target changes.
- Preserve unsaved drafts and show previous, current and draft values. Merge independent edits and choose which conflicting draft fields to keep.
- Bind diff previews and snapshot lists to the original configuration target. A changed revision or path requires a new review.

## 1.3.0

- Add a bilingual, read-only project skills inventory for documented Codex CLI, Claude Code, and OpenCode project folders.
- Display relative manifest paths, candidate CLIs, SHA-256 fingerprints, and a matching-folder-name hint across shared CLIs.
- Report partial scans and skip links, unreadable files, and files over the scan limits.
- Document trend research, acceptance criteria, and future priorities in the product roadmap.

## 1.2.0

- Select the individual fields to apply from a saved profile.
- Keep configuration removals unchecked by default. Select a removal explicitly to clear that setting.
- Check the chosen values against the saved profile and verify the live file revision before applying them.

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
