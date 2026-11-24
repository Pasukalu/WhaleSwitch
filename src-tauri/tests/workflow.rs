use std::{collections::HashMap, fs, io::{BufRead, BufReader, Write}, net::TcpListener, path::PathBuf, sync::Mutex, thread};
use tempfile::TempDir;
use whaleswitch_lib::{ConfigObservation, Engine, Error, Field, Fields, ProbeInput, ProbeOutcome, ProbeStage, Protocol, Result, SecretStore, Tool};
use zeroize::Zeroizing;

#[derive(Default)]
struct TestVault(Mutex<HashMap<String, Zeroizing<String>>>);

impl SecretStore for TestVault {
    fn get(&self, account: &str) -> Result<Option<Zeroizing<String>>> {
        Ok(self.0.lock().unwrap().get(account).cloned())
    }
    fn set(&self, account: &str, secret: &str) -> Result<()> {
        self.0.lock().unwrap().insert(account.into(), Zeroizing::new(secret.into()));
        Ok(())
    }
    fn remove(&self, account: &str) -> Result<()> {
        self.0.lock().unwrap().remove(account);
        Ok(())
    }
}

struct Fixture { dir: TempDir, files: [PathBuf; 3], engine: Engine }

fn fixture(vault: impl SecretStore + 'static) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    // Resolve OS temp aliases such as /var -> /private/var on macOS.
    let base = dir.path().canonicalize().unwrap();
    let files = [base.join("config.toml"), base.join("settings.json"), base.join("opencode.jsonc")];
    let engine = Engine::isolated(base.join("data"), files.clone(), Box::new(vault));
    Fixture { dir, files, engine }
}

fn model(value: Option<&str>) -> Fields {
    [(Field::Model, value.map(str::to_owned))].into_iter().collect()
}

#[test]
fn applies_verified_fields_without_losing_unknown_keys_or_comments() {
    let f = fixture(TestVault::default());
    let cases = [
        (Tool::Codex, "# global\nmodel  =  \"old\" # keep here\n[unrelated]\nitem = true\n", "new-model", "# keep here"),
        (Tool::Claude, "{\"model\": \"sonnet\", \"unknown\": {\"n\": [1, true, null]}}", "opus", "\"unknown\": {\"n\": [1, true, null]}"),
        (Tool::Opencode, "{\n // user config\n \"model\": \"openai/old\", // keep here\n \"unknown\": [\"x\",],\n}", "anthropic/new", "// keep here"),
    ];
    for (index, (tool, original, next, retained)) in cases.into_iter().enumerate() {
        fs::write(&f.files[index], original).unwrap();
        let view = f.engine.view(tool).unwrap();
        let changes = f.engine.preview(tool, &view.path, &view.revision, &model(Some(next))).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].after.as_deref(), Some(next));
        let after = f.engine.save(tool, &view.path, &view.revision, &model(Some(next))).unwrap();
        assert_eq!(after.fields.get(&Field::Model).and_then(|value| value.as_deref()), Some(next));
        let edited = fs::read_to_string(&f.files[index]).unwrap();
        assert!(edited.contains(retained));
        if tool == Tool::Codex { assert!(edited.contains("# global")); }
        let backup = f.engine.snapshots(tool).unwrap().remove(0);
        assert_eq!(backup.reason, "before_save");
        f.engine.restore(tool, &after.path, &after.revision, backup.id).unwrap();
        assert_eq!(fs::read(&f.files[index]).unwrap(), original.as_bytes());
        assert_eq!(f.engine.snapshots(tool).unwrap()[0].reason, "before_restore");
    }
}

#[test]
fn restores_selected_snapshot_values_for_each_tool() {
    let f = fixture(TestVault::default());
    let cases = [
        (Tool::Codex, "model = 'old'\nopenai_base_url = 'https://prior.invalid/v1'\n",
            "# current comment\nmodel = 'new'\nopenai_base_url = 'https://after.invalid/v1'\n[extra]\nkeep = true\n",
            "old", "https://after.invalid/v1", "# current comment"),
        (Tool::Claude, r#"{"model":"sonnet","env":{"ANTHROPIC_BASE_URL":"https://prior.invalid"}}"#,
            r#"{"model":"opus","env":{"ANTHROPIC_BASE_URL":"https://after.invalid"},"unknown": {"keep":true}}"#,
            "sonnet", "https://after.invalid", r#""unknown": {"keep":true}"#),
        (Tool::Opencode, r#"{"model":"openai/old"}"#,
            "{\n // current comment\n \"model\": \"openai/new\", \"default_agent\": \"build\", \"unknown\": true,\n}",
            "openai/old", r#""default_agent": "build""#, "// current comment"),
    ];
    for (index, (tool, old, current, expected, untouched, unknown)) in cases.into_iter().enumerate() {
        let path = &f.files[index];
        fs::write(path, old).unwrap();
        let view = f.engine.view(tool).unwrap();
        let snapshot = f.engine.snapshot(tool, &view.path, &view.revision).unwrap();
        fs::write(path, current).unwrap();
        let view = f.engine.view(tool).unwrap();
        let changes = f.engine.compare_snapshot_fields(tool, &view.path, &view.revision, snapshot.id).unwrap();
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|change| change.field == Field::Model && change.after.as_deref() == Some(expected)));
        let forged = if tool == Tool::Opencode { "openai/forged" } else { "forged" };
        assert!(matches!(
            f.engine.restore_snapshot_fields(tool, &view.path, &view.revision, snapshot.id, &model(Some(forged))),
            Err(Error::Conflict)
        ));
        assert_eq!(f.engine.snapshots(tool).unwrap().len(), 1);
        let restored = f.engine.restore_snapshot_fields(tool, &view.path, &view.revision, snapshot.id, &model(Some(expected))).unwrap();
        assert_eq!(restored.fields[&Field::Model].as_deref(), Some(expected));
        let updated = fs::read_to_string(path).unwrap();
        assert!(updated.contains(untouched));
        assert!(updated.contains(unknown));
        let backup = f.engine.snapshots(tool).unwrap().remove(0);
        assert_eq!(backup.reason, "before_restore_fields");
        f.engine.restore(tool, &restored.path, &restored.revision, backup.id).unwrap();
        assert_eq!(fs::read(path).unwrap(), current.as_bytes());
    }
}

#[test]
fn field_restore_can_clear_an_absent_key_and_rejects_stale_revisions() {
    let f = fixture(TestVault::default());
    let tool = Tool::Claude;
    let path = &f.files[1];
    fs::write(path, r#"{"model":"sonnet"}"#).unwrap();
    let view = f.engine.view(tool).unwrap();
    let snapshot = f.engine.snapshot(tool, &view.path, &view.revision).unwrap();
    let latest = r#"{"model":"opus","env":{"ANTHROPIC_BASE_URL":"https://example.invalid","KEEP":"unchanged"}}"#;
    fs::write(path, latest).unwrap();
    let view = f.engine.view(tool).unwrap();
    let selection: Fields = [(Field::AnthropicBaseUrl, None)].into_iter().collect();
    let changes = f.engine.compare_snapshot_fields(tool, &view.path, &view.revision, snapshot.id).unwrap();
    assert!(changes.iter().any(|change| change.field == Field::AnthropicBaseUrl && change.after.is_none()));
    fs::write(path, format!("{latest}\n")).unwrap();
    assert!(matches!(
        f.engine.restore_snapshot_fields(tool, &view.path, &view.revision, snapshot.id, &selection),
        Err(Error::Conflict)
    ));
    let fresh = f.engine.view(tool).unwrap();
    let restored = f.engine.restore_snapshot_fields(tool, &fresh.path, &fresh.revision, snapshot.id, &selection).unwrap();
    assert_eq!(restored.fields[&Field::AnthropicBaseUrl], None);
    assert_eq!(restored.fields[&Field::Model].as_deref(), Some("opus"));
    assert!(fs::read_to_string(path).unwrap().contains(r#""KEEP":"unchanged""#));
}

#[test]
fn field_restore_excludes_unsupported_values_from_both_sides() {
    let f = fixture(TestVault::default());
    let tool = Tool::Claude;
    let path = &f.files[1];
    let original = r#"{"model":false,"env":{"ANTHROPIC_BASE_URL":"https://example.invalid"}}"#;
    fs::write(path, original).unwrap();
    let view = f.engine.view(tool).unwrap();
    let snapshot = f.engine.snapshot(tool, &view.path, &view.revision).unwrap();
    let current = r#"{"model":"opus","env":false}"#;
    fs::write(path, current).unwrap();
    let view = f.engine.view(tool).unwrap();
    assert!(f.engine.compare_snapshot_fields(tool, &view.path, &view.revision, snapshot.id).unwrap().is_empty());
    assert!(matches!(
        f.engine.restore_snapshot_fields(tool, &view.path, &view.revision, snapshot.id, &model(None)),
        Err(Error::Conflict)
    ));
    assert_eq!(fs::read_to_string(path).unwrap(), current);
    assert_eq!(f.engine.snapshots(tool).unwrap().len(), 1);

    fs::write(path, "{broken").unwrap();
    let view = f.engine.view(tool).unwrap();
    assert!(matches!(
        f.engine.compare_snapshot_fields(tool, &view.path, &view.revision, snapshot.id),
        Err(Error::InvalidConfig)
    ));
    f.engine.restore(tool, &view.path, &view.revision, snapshot.id).unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}

#[test]
fn guards_the_revision_and_config_scope() {
    let f = fixture(TestVault::default());
    let empty = f.engine.view(Tool::Codex).unwrap();
    assert!(!empty.exists);
    let view = f.engine.save(Tool::Codex, &empty.path, &empty.revision, &model(Some("first"))).unwrap();
    fs::write(&f.files[0], "model = 'external'\n").unwrap();
    let save = f.engine.save(Tool::Codex, &view.path, &view.revision, &model(Some("overwrite")));
    assert!(matches!(save, Err(Error::Conflict)));
    assert!(f.engine.snapshots(Tool::Codex).unwrap().is_empty());
    assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), "model = 'external'\n");
    let current = f.engine.view(Tool::Codex).unwrap();
    let backup = f.engine.snapshot(Tool::Codex, &current.path, &current.revision).unwrap();
    let other = f.engine.view(Tool::Claude).unwrap();
    assert!(matches!(f.engine.restore(Tool::Claude, &other.path, &other.revision, backup.id), Err(Error::SnapshotMismatch)));
}

#[test]
fn inspects_external_changes_without_touching_the_configuration_or_vault() {
    let f = fixture(TestVault::default());
    let path = &f.files[0];
    fs::write(path, "model = 'initial'\n").unwrap();
    let ConfigObservation::View { view: old } = f.engine.inspect_config(Tool::Codex).config else { panic!("missing initial view") };
    let external = b"model = 'initial'\n# edited outside WhaleSwitch\n";
    fs::write(path, external).unwrap();
    let ConfigObservation::View { view: latest } = f.engine.inspect_config(Tool::Codex).config else { panic!("missing edited view") };
    assert_ne!(latest.revision, old.revision);
    assert_eq!(latest.fields, old.fields);
    assert_eq!(fs::read(path).unwrap(), external.as_slice());

    fs::remove_file(path).unwrap();
    let saved = f.engine.save(Tool::Codex, &latest.path, &latest.revision, &model(Some("draft")));
    assert!(matches!(saved, Err(Error::Conflict)));
    let ConfigObservation::View { view: deleted } = f.engine.inspect_config(Tool::Codex).config else { panic!("missing deletion state") };
    assert!(!deleted.exists);
    assert_eq!(deleted.revision, "missing");
    assert!(!path.exists());
    assert!(f.engine.snapshots_for(Tool::Codex, &deleted.path).unwrap().is_empty());

    fs::write(path, "model = [").unwrap();
    let ConfigObservation::View { view: malformed } = f.engine.inspect_config(Tool::Codex).config else { panic!("missing malformed state") };
    assert!(malformed.exists);
    assert_eq!(malformed.issue, Some(Error::InvalidConfig));
    assert!(malformed.fields.is_empty());
    assert!(matches!(f.engine.snapshots_for(Tool::Codex, "/different/config.toml"), Err(Error::Conflict)));
}

#[test]
fn saves_a_rebased_patch_and_backs_up_the_externally_edited_bytes() {
    let f = fixture(TestVault::default());
    let path = &f.files[0];
    fs::write(path, "model = 'initial'\n").unwrap();
    let original = f.engine.view(Tool::Codex).unwrap();
    let external = b"model = 'initial'\n# user comment\n[other]\nkeep = 'external edit'\n";
    fs::write(path, external).unwrap();
    assert!(matches!(
        f.engine.save(Tool::Codex, &original.path, &original.revision, &model(Some("draft"))),
        Err(Error::Conflict)
    ));
    let current = f.engine.view(Tool::Codex).unwrap();
    let result = f.engine.preview(Tool::Codex, &current.path, &current.revision, &model(Some("draft"))).unwrap();
    assert_eq!(result[0].before.as_deref(), Some("initial"));
    let saved = f.engine.save(Tool::Codex, &current.path, &current.revision, &model(Some("draft"))).unwrap();
    let updated = fs::read_to_string(path).unwrap();
    assert!(updated.contains("# user comment"));
    assert!(updated.contains("keep = 'external edit'"));
    let snapshot = f.engine.snapshots_for(Tool::Codex, &saved.path).unwrap().remove(0);
    f.engine.restore(Tool::Codex, &saved.path, &saved.revision, snapshot.id).unwrap();
    assert_eq!(fs::read(path).unwrap(), external.as_slice());
}

#[test]
fn keeps_snapshot_secret_out_of_sqlite_and_export_and_recovers_bad_config() {
    let f = fixture(TestVault::default());
    let bytes = b"{\"model\":\"sonnet\",\"env\":{\"ANTHROPIC_API_KEY\":\"test-sensitive-original\"}}";
    fs::write(&f.files[1], bytes).unwrap();
    let before = f.engine.view(Tool::Claude).unwrap();
    let backup = f.engine.snapshot(Tool::Claude, &before.path, &before.revision).unwrap();
    let exported = f.dir.path().canonicalize().unwrap().join("export.json");
    f.engine.export_archive(Tool::Claude, &before.path, &before.revision, &exported).unwrap();
    assert_eq!(f.engine.import_archive(Tool::Claude, &exported).unwrap(), model(Some("sonnet")));
    assert!(matches!(f.engine.import_archive(Tool::Opencode, &exported), Err(Error::WrongTool)));
    assert_eq!(f.engine.export_archive(Tool::Claude, &before.path, &before.revision, &exported), Err(Error::Conflict));
    let sqlite = fs::read(f.dir.path().join("data/snapshots.sqlite3")).unwrap();
    assert!(!String::from_utf8_lossy(&sqlite).contains("test-sensitive-original"));
    assert!(!fs::read_to_string(exported).unwrap().contains("test-sensitive-original"));
    fs::write(&f.files[1], "{broken").unwrap();
    let broken = f.engine.view(Tool::Claude).unwrap();
    assert_eq!(broken.issue, Some(Error::InvalidConfig));
    assert_eq!(f.engine.compare_snapshot(Tool::Claude, &broken.path, &broken.revision, backup.id).unwrap().len(), 1);
    f.engine.restore(Tool::Claude, &broken.path, &broken.revision, backup.id).unwrap();
    assert_eq!(fs::read(&f.files[1]).unwrap(), bytes);
}

#[test]
fn rejects_duplicate_archive_keys_and_wrong_format() {
    let f = fixture(TestVault::default());
    let archive = f.dir.path().canonicalize().unwrap().join("archive.json");
    fs::write(&archive, "{\"version\":1,\"tool\":\"claude\",\"fields\":{\"model\":\"x\",\"model\":\"y\"}}").unwrap();
    assert!(matches!(f.engine.import_archive(Tool::Claude, &archive), Err(Error::InvalidArchive)));
    let opencode = f.engine.view(Tool::Opencode).unwrap();
    assert!(matches!(f.engine.preview(Tool::Opencode, &opencode.path, &opencode.revision, &model(Some("no-provider"))), Err(Error::InvalidField)));
}

struct ClosedVault;
impl SecretStore for ClosedVault {
    fn get(&self, _: &str) -> Result<Option<Zeroizing<String>>> { Err(Error::VaultUnavailable) }
    fn set(&self, _: &str, _: &str) -> Result<()> { Err(Error::VaultUnavailable) }
    fn remove(&self, _: &str) -> Result<()> { Err(Error::VaultUnavailable) }
}

#[test]
fn unavailable_keyring_keeps_the_original_file() {
    let f = fixture(ClosedVault);
    fs::write(&f.files[0], "model = 'first'").unwrap();
    let view = f.engine.view(Tool::Codex).unwrap();
    assert!(matches!(f.engine.save(Tool::Codex, &view.path, &view.revision, &model(Some("new"))), Err(Error::VaultUnavailable)));
    assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), "model = 'first'");
}

#[test]
fn probes_a_local_models_endpoint_with_endpoint_scoped_keys() {
    let f = fixture(TestVault::default());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut request = String::new();
        let mut reader = BufReader::new(&mut socket);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" { break; }
            request.push_str(&line);
        }
        assert!(request.starts_with("GET /v1/models HTTP/1.1"));
        assert!(request.contains("authorization: Bearer fake-model-test-key\r\n"));
        let body = r#"{"data":[{"id":"test-model"}]}"#;
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    let input = ProbeInput { tool: Tool::Codex, protocol: Protocol::Openai, endpoint: format!("http://{address}/v1"), anonymous: false };
    f.engine.set_key(&input, Zeroizing::new("fake-model-test-key".to_owned())).unwrap();
    let other = ProbeInput { tool: Tool::Claude, protocol: Protocol::Openai, endpoint: input.endpoint.clone(), anonymous: false };
    assert!(!f.engine.has_key(&other).unwrap());
    let result = f.engine.probe(&input).unwrap();
    assert_eq!(result.outcome, ProbeOutcome::Connected);
    assert_eq!(result.stage, ProbeStage::Models);
    assert_eq!(result.models, Some(1));
    server.join().unwrap();
    f.engine.remove_key(&input).unwrap();
    assert!(!f.engine.has_key(&input).unwrap());
}

#[test]
fn named_profiles_persist_and_apply_through_checked_backups() {
    let f = fixture(TestVault::default());
    let tool = Tool::Codex;
    let original = "# Keep this\nmodel = 'original'\nmodel_verbosity = 'high'\n";
    fs::write(&f.files[0], original).unwrap();
    let view = f.engine.view(tool).unwrap();
    let patch: Fields = [(Field::Model, Some("profile-model".into())), (Field::ModelVerbosity, None)].into_iter().collect();
    let profile = f.engine.create_profile(tool, &view.path, &view.revision, " Work ", &patch).unwrap();
    assert_eq!(profile.name, "Work");
    assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), original);
    assert!(matches!(f.engine.create_profile(tool, &view.path, &view.revision, "work", &patch), Err(Error::ProfileNameExists)));
    let base = f.dir.path().canonicalize().unwrap();
    let reopened = Engine::isolated(base.join("data"), f.files.clone(), Box::new(ClosedVault));
    assert_eq!(reopened.profiles(tool).unwrap()[0].id, profile.id);
    assert!(reopened.profiles(Tool::Claude).unwrap().is_empty());
    let preview = f.engine.preview_profile(tool, &view.path, &view.revision, profile.id).unwrap();
    assert_eq!(preview.changes.len(), 2);
    assert!(matches!(f.engine.apply_profile(tool, &view.path, "stale", profile.id), Err(Error::Conflict)));
    let updated = f.engine.apply_profile(tool, &view.path, &view.revision, profile.id).unwrap();
    assert_eq!(updated.fields[&Field::Model].as_deref(), Some("profile-model"));
    assert_eq!(updated.fields[&Field::ModelVerbosity], None);
    let previous = f.engine.snapshots(tool).unwrap().remove(0);
    f.engine.restore(tool, &updated.path, &updated.revision, previous.id).unwrap();
    assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), original);
    f.engine.delete_profile(tool, profile.id).unwrap();
    assert!(matches!(f.engine.preview_profile(tool, &view.path, &view.revision, profile.id), Err(Error::ProfileMissing)));
}

#[test]
fn renaming_and_duplicating_profiles_does_not_write_live_configuration() {
    let f = fixture(ClosedVault);
    let tool = Tool::Codex;
    let original = "model = 'live'\n";
    fs::write(&f.files[0], original).unwrap();
    let view = f.engine.view(tool).unwrap();
    let patch: Fields = [(Field::Model, Some("captured".into()))].into_iter().collect();
    let profile = f.engine.create_profile(tool, &view.path, &view.revision, "Work", &patch).unwrap();
    let copy = f.engine.duplicate_profile(tool, profile.id, "Nightly").unwrap();
    assert_ne!(copy.id, profile.id);
    assert_eq!(copy.fields, profile.fields);
    assert!(matches!(f.engine.rename_profile(tool, profile.id, "nightly"), Err(Error::ProfileNameExists)));
    assert!(matches!(f.engine.duplicate_profile(Tool::Claude, profile.id, "Other"), Err(Error::ProfileMissing)));
    assert!(matches!(f.engine.rename_profile(tool, copy.id, "  "), Err(Error::InvalidProfileName)));
    let renamed = f.engine.rename_profile(tool, profile.id, " Work machine ").unwrap();
    assert_eq!(renamed.id, profile.id);
    assert_eq!(renamed.name, "Work machine");
    assert_eq!(renamed.timestamp, profile.timestamp);
    assert_eq!(renamed.fields, profile.fields);
    assert_eq!(f.engine.profiles(tool).unwrap().len(), 2);
    assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), original);
    assert!(f.engine.snapshots(tool).unwrap().is_empty());
}

#[test]
fn portable_profiles_keep_explicit_defaults_and_do_not_apply_on_import() {
    let f = fixture(ClosedVault);
    let tool = Tool::Codex;
    let original = "model = 'live'\nmodel_verbosity = 'high'\n# private note\n";
    fs::write(&f.files[0], original).unwrap();
    let view = f.engine.view(tool).unwrap();
    let patch: Fields = [(Field::Model, Some("portable".into())), (Field::ModelVerbosity, None)].into_iter().collect();
    let profile = f.engine.create_profile(tool, &view.path, &view.revision, "Work", &patch).unwrap();
    let destination = f.dir.path().canonicalize().unwrap().join("work.profile.json");
    f.engine.export_profile_archive(tool, profile.id, &destination).unwrap();
    let bytes = fs::read(&destination).unwrap();
    let raw = String::from_utf8_lossy(&bytes);
    assert!(!raw.contains("private note"));
    assert!(!raw.contains("timestamp"));
    assert!(!raw.contains("revision"));
    assert!(matches!(f.engine.export_profile_archive(tool, profile.id, &destination), Err(Error::Conflict)));
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    assert!(matches!(f.engine.read_profile_archive(Tool::Claude, &destination), Err(Error::WrongTool)));
    assert!(matches!(f.engine.import_archive(tool, &destination), Err(Error::InvalidArchive)));

    let archive = f.engine.read_profile_archive(tool, &destination).unwrap();
    assert_eq!(archive.name, profile.name);
    assert_eq!(archive.fields.get(&Field::ModelVerbosity), Some(&None));
    assert!(matches!(f.engine.import_profile(tool, archive, "work"), Err(Error::ProfileNameExists)));
    let archive = f.engine.read_profile_archive(tool, &destination).unwrap();
    let imported = f.engine.import_profile(tool, archive, "Travel").unwrap();
    assert_ne!(imported.id, profile.id);
    assert_eq!(imported.fields, profile.fields);
    let review = f.engine.preview_profile(tool, &view.path, &view.revision, imported.id).unwrap();
    assert!(review.changes.iter().any(|change| change.field == Field::ModelVerbosity
        && change.before.as_deref() == Some("high") && change.after.is_none()));
    assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), original);
    assert!(f.engine.snapshots(tool).unwrap().is_empty());
}

#[test]
fn selected_profile_changes_keep_the_live_endpoint_and_back_up_each_confirmed_write() {
    let f = fixture(TestVault::default());
    let tool = Tool::Codex;
    let original = "model = 'live'\nopenai_base_url = 'https://current.example/v1'\n# user comment\n";
    fs::write(&f.files[0], original).unwrap();
    let view = f.engine.view(tool).unwrap();
    let patch: Fields = [(Field::Model, Some("portable".into())), (Field::OpenaiBaseUrl, None)].into_iter().collect();
    let profile = f.engine.create_profile(tool, &view.path, &view.revision, "Travel", &patch).unwrap();
    let tampered: Fields = [(Field::Model, Some("unreviewed".into()))].into_iter().collect();
    let apply = |fields: &Fields| f.engine.apply_profile_selection(tool, &view.path, &view.revision, profile.id, fields);
    assert!(matches!(apply(&Fields::new()), Err(Error::InvalidField)));
    assert!(matches!(apply(&tampered), Err(Error::Conflict)));
    assert!(f.engine.snapshots(tool).unwrap().is_empty());

    let selected: Fields = [(Field::Model, Some("portable".into()))].into_iter().collect();
    let updated = apply(&selected).unwrap();
    assert_eq!(updated.fields[&Field::Model].as_deref(), Some("portable"));
    assert_eq!(updated.fields[&Field::OpenaiBaseUrl].as_deref(), Some("https://current.example/v1"));
    assert!(fs::read_to_string(&f.files[0]).unwrap().contains("# user comment"));
    assert_eq!(f.engine.snapshots(tool).unwrap().len(), 1);

    let clear: Fields = [(Field::OpenaiBaseUrl, None)].into_iter().collect();
    assert!(matches!(apply(&clear), Err(Error::Conflict)));
    let cleared = f.engine.apply_profile_selection(tool, &updated.path, &updated.revision, profile.id, &clear).unwrap();
    assert_eq!(cleared.fields[&Field::OpenaiBaseUrl], None);
    assert_eq!(cleared.fields[&Field::Model].as_deref(), Some("portable"));
    assert_eq!(f.engine.snapshots(tool).unwrap().len(), 2);
}

#[test]
fn profile_import_rejects_invalid_or_unsupported_archive_content() {
    let f = fixture(ClosedVault);
    let destination = f.dir.path().canonicalize().unwrap().join("untrusted.profile.json");
    for contents in [
        r#"{"version":2,"tool":"codex","name":"Work","fields":{}}"#,
        r#"{"version":1,"tool":"codex","name":"Work","fields":{"default_agent":"build"}}"#,
        r#"{"version":1,"tool":"codex","name":"Work","fields":{},"secret":"opaque"}"#,
        r#"{"version":1,"tool":"codex","name":"Work","fields":{"model":"first","model":"second"}}"#,
        r#"{"version":1,"tool":"codex","name":"sk-secret","fields":{}}"#,
    ] {
        fs::write(&destination, contents).unwrap();
        assert!(matches!(f.engine.read_profile_archive(Tool::Codex, &destination), Err(Error::InvalidArchive)));
    }
    assert!(f.engine.profiles(Tool::Codex).unwrap().is_empty());
}

#[test]
fn edits_the_verified_endpoint_inside_claude_env_without_exposing_other_keys() {
    let f = fixture(TestVault::default());
    let tool = Tool::Claude;
    let original = r#"{"env":{"ANTHROPIC_API_KEY":"test-private-credential","CUSTOM":"keep"},"model":"sonnet"}"#;
    fs::write(&f.files[1], original).unwrap();
    let view = f.engine.view(tool).unwrap();
    let value = "https://gateway.example/team";
    let fields: Fields = [(Field::AnthropicBaseUrl, Some(value.to_owned()))].into_iter().collect();
    let saved = f.engine.save(tool, &view.path, &view.revision, &fields).unwrap();
    assert_eq!(saved.fields[&Field::AnthropicBaseUrl].as_deref(), Some(value));
    let raw = fs::read_to_string(&f.files[1]).unwrap();
    assert!(raw.contains(r#""ANTHROPIC_API_KEY":"test-private-credential","CUSTOM":"keep""#));
    let mut removed = fields;
    removed.insert(Field::AnthropicBaseUrl, None);
    f.engine.save(tool, &saved.path, &saved.revision, &removed).unwrap();
    let edited = fs::read_to_string(&f.files[1]).unwrap();
    assert!(!edited.contains("ANTHROPIC_BASE_URL"));
    assert!(edited.contains("test-private-credential"));
    fs::write(&f.files[1], r#"{"env":["unknown shape"]}"#).unwrap();
    let malformed = f.engine.view(tool).unwrap();
    removed.insert(Field::AnthropicBaseUrl, Some(value.to_owned()));
    assert!(matches!(f.engine.save(tool, &malformed.path, &malformed.revision, &removed), Err(Error::InvalidField)));
}

#[test]
fn keyless_loopback_probe_skips_the_os_vault_and_handles_http_redirect() {
    let f = fixture(ClosedVault);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut request = String::new();
        for line in BufReader::new(&mut socket).lines() {
            let line = line.unwrap();
            if line.is_empty() { break; }
            request.push_str(&line);
            request.push('\n');
        }
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: https://example.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let input = ProbeInput { tool: Tool::Opencode, protocol: Protocol::Openai, endpoint: format!("http://{address}/v1"), anonymous: true };
    let result = f.engine.probe(&input).unwrap();
    assert_eq!(result.outcome, ProbeOutcome::Redirect);
    assert_eq!(result.stage, ProbeStage::Http);
    assert_eq!(result.status, Some(302));
    server.join().unwrap();
}

#[test]
fn opencode_supports_default_agent_without_migrating_provider_blocks() {
    let f = fixture(TestVault::default());
    let tool = Tool::Opencode;
    let provider_blocks = [
        r#""provider":{"local":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"http://localhost:11434/v1"}}}"#,
        r#""providers":{"local":{"package":"@opencode/ai/providers/openai-compatible","settings":{"baseURL":"http://localhost:11434/v1"}}}"#,
    ];
    for block in provider_blocks {
        fs::write(&f.files[2], format!("{{\n // retain this\n {block}, \"model\":\"local/coder\"\n}}")).unwrap();
        let view = f.engine.view(tool).unwrap();
        let patch: Fields = [(Field::DefaultAgent, Some("plan".to_owned()))].into_iter().collect();
        let updated = f.engine.save(tool, &view.path, &view.revision, &patch).unwrap();
        assert_eq!(updated.fields[&Field::DefaultAgent].as_deref(), Some("plan"));
        let edited = fs::read_to_string(&f.files[2]).unwrap();
        assert!(edited.contains(block));
        assert!(edited.contains("// retain this"));
    }
}
