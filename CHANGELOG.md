# Changelog

## 1.10.1

- Find a configuration snapshot by its exact number, filter pinned snapshots, and page through the current configuration’s history.
- Open the page containing a snapshot directly from local activity or a configuration plan result. Keyboard focus moves to the selected snapshot once, so a history refresh can no longer jump back to it.

## 1.10.0

- Pin configuration snapshots and inspect their encrypted payload size. Preview and explicitly select older snapshots to remove. Cleanup preserves the newest five at each target, pinned snapshots, and plan backups.
- Review local configuration activity by tool and export a metadata-only JSON report. Existing plan runs contribute per-tool results without exposing their names or settings.
- Add regression coverage for existing databases, stale cleanup previews, transaction rollback, and export privacy. The native release roundtrip exercises pinned and selected snapshots.

## 1.9.0

- Preview a local upgrade of an existing project skill. Compare both versions of each changed file and explicitly select which files to apply.
- Create an encrypted backup before each upgrade or restore. Preview recovery by project, keep files with newer edits, and undo an earlier restore from its own backup.
- Cover partial upgrades, stale previews, byte-for-byte recovery, and write failures with temporary project fixtures. Exercise upgrade and restore with the native keychain in the release retest.

## 1.8.0

- Connection diagnostics identify DNS, TLS and connection failures, HTTP status categories, interrupted or oversized bodies, invalid JSON, and model-list shape mismatches.
- The connection card shows the observed stage, status, elapsed time, and a suggested check. HTTP 429 and 503 can show a wait time from `Retry-After`.
- A copyable diagnostic summary contains fixed status fields and omits the endpoint, credentials, headers, raw response, and model identifiers.
- One explicit test sends a model-list request with a total request deadline, redirects and automatic retries disabled, and a clear WhaleSwitch user agent.

## 1.7.0

- Save a project-specific skill manifest baseline, rescan the selected project, and review additions, modifications, and removals across the three supported CLI locations.
- Filter the inventory and change list by path. Incomplete scans show confirmed changes and defer removal detection.
- Preview the path and source text of each file before importing a local skill. Imports copy `SKILL.md` and supported Markdown or text references.
- Recheck the source and project destination before writing. Same-name destinations stop the operation. Failed imports roll back verified files created by that run and preserve external edits.

## 1.6.0

- Combine up to three tool profiles into a named configuration plan. Import and export plans as JSON independently of their original profiles.
- Review each target path and select fields across tools. Keep field removals unchecked initially and reject stale or overlapping targets before writing.
- Verify all encrypted backups before the first file replacement. Attempt to restore files already written if a later step fails and show each file's outcome and recovery snapshot.
- Record interrupted plan runs locally and inspect the current file revisions on reopening.
- Lock npm and Rust dependency resolution and launch the Windows portable ZIP during CI.

## 1.5.0

- Restore selected fields from snapshots for Codex CLI, Claude Code, and OpenCode. Changes outside the selection stay in the current file.
- Keep removals unchecked initially. Skip unsupported field values during selective restore, and verify the snapshot values and live revision before writing.
- Ship a Windows x64 portable ZIP with application data alongside the executable and keys in Windows Credential Manager.
- Show the active data folder under Settings and verify the portable package alongside all installer checksums before publishing.

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
