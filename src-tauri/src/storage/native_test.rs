use super::{SecretStore, SystemVault};
use crate::{
    Engine, Field, PlanArchive, PlanEntry, PlanFileStatus, PlanSelection, ProbeInput, Protocol,
    Result, SkillLocation, Tool,
};
use std::{collections::BTreeSet, fs, sync::Mutex};
use zeroize::Zeroizing;

struct ScopedVault {
    prefix: String,
    accounts: Mutex<BTreeSet<String>>,
}

impl ScopedVault {
    fn account(&self, account: &str) -> String {
        format!("{}-{account}", self.prefix)
    }
}

impl SecretStore for ScopedVault {
    fn get(&self, account: &str) -> Result<Option<Zeroizing<String>>> {
        SystemVault.get(&self.account(account))
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        let full_account = self.account(account);
        self.accounts.lock().unwrap().insert(full_account.clone());
        SystemVault.set(&full_account, secret)
    }

    fn remove(&self, account: &str) -> Result<()> {
        SystemVault.remove(&self.account(account))
    }
}

impl Drop for ScopedVault {
    fn drop(&mut self) {
        for account in self.accounts.get_mut().unwrap().iter() {
            let _ = SystemVault.remove(account);
        }
    }
}

#[test]
#[ignore = "requires an unlocked native credential store on a test runner"]
fn native_release_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let files = [
        root.join("config.toml"),
        root.join("settings.json"),
        root.join("opencode.jsonc"),
    ];
    let cases = [
        ("model = 'old'\n# preserve-whaleswitch-fixture\n", "new-model"),
        (r#"{"model":"sonnet","marker":"preserve-whaleswitch-fixture"}"#, "opus"),
        ("{\n  // preserve-whaleswitch-fixture\n  \"model\": \"openai/old\",\n}\n", "openai/new"),
    ];
    for (file, (original, _)) in files.iter().zip(cases) {
        fs::write(file, original).unwrap();
    }
    let data = root.join("data");
    let prefix = format!("release-smoke-{:032x}", rand::random::<u128>());
    let vault = ScopedVault { prefix: prefix.clone(), accounts: Mutex::default() };
    let engine = Engine::isolated(data.clone(), files.clone(), Box::new(vault));
    let probe = ProbeInput {
        tool: Tool::Codex,
        protocol: Protocol::Openai,
        endpoint: "https://whaleswitch.invalid/v1".into(),
        anonymous: false,
    };
    assert!(!engine.has_key(&probe).unwrap());
    engine.set_key(&probe, Zeroizing::new("whaleswitch-synthetic-key".into())).unwrap();
    assert!(engine.has_key(&probe).unwrap());
    engine.remove_key(&probe).unwrap();
    assert!(!engine.has_key(&probe).unwrap());

    let entries = Tool::ALL.into_iter().zip(cases).map(|(tool, (_, next))| PlanEntry {
        tool, fields: [(Field::Model, Some(next.to_owned()))].into_iter().collect(),
    }).collect();
    let name = "Native release test";
    let plan = engine.import_plan(PlanArchive { version: 1, name: name.into(), entries }, name).unwrap();
    let selected = engine.preview_plan(plan.id).unwrap().into_iter().map(|row| PlanSelection {
        tool: row.tool,
        path: row.path.unwrap(),
        revision: row.revision.unwrap(),
        fields: row.changes.into_iter().map(|change| (change.field, change.after)).collect(),
    }).collect::<Vec<_>>();
    let run = engine.apply_plan(plan.id, &selected).unwrap();
    assert!(run.finished);
    assert_eq!(run.files.len(), Tool::ALL.len());
    assert!(SystemVault.get(&format!("{prefix}-snapshot-encryption-v1")).unwrap().is_some());
    let db = fs::read(data.join("snapshots.sqlite3")).unwrap();
    let marker = b"preserve-whaleswitch-fixture";
    assert!(!db.windows(marker.len()).any(|window| window == marker));

    for ((file, (original, next)), result) in files.iter().zip(cases).zip(run.files) {
        assert_eq!(result.status, PlanFileStatus::Applied);
        let current = engine.view(result.tool).unwrap();
        assert_eq!(current.fields.get(&Field::Model).and_then(|v| v.as_deref()), Some(next));
        assert!(fs::read_to_string(file).unwrap().contains("preserve-whaleswitch-fixture"));
        engine.restore(result.tool, &current.path, &current.revision, result.snapshot_id.unwrap()).unwrap();
        assert_eq!(fs::read(file).unwrap(), original.as_bytes());
    }
    let project = root.join("skill-workspace");
    let source = root.join("review-native");
    let target = project.join(".agents/skills/review-native");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&target).unwrap();
    let original = b"---\r\nname: review-native\r\ndescription: Original native skill fixture\r\n---\r\n";
    fs::write(target.join("SKILL.md"), original).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: review-native\ndescription: Updated native skill fixture\n---\n").unwrap();
    let selected = ["SKILL.md".into()];
    let preview = engine.preview_skill_upgrade(&project, &source, SkillLocation::Agents).unwrap();
    let saved = engine.apply_skill_upgrade(&project, &source, SkillLocation::Agents, &preview.revision, &selected).unwrap();
    let restore = engine.preview_skill_restore(&project, saved.id).unwrap();
    engine.apply_skill_restore(&project, saved.id, &restore.revision, &selected).unwrap();
    assert_eq!(fs::read(target.join("SKILL.md")).unwrap(), original);

    let tool = Tool::Codex;
    let current = engine.view(tool).unwrap();
    let bytes = fs::read(&files[0]).unwrap();
    let older = engine.snapshot(tool, &current.path, &current.revision).unwrap();
    engine.pin_snapshot(tool, &current.path, older.id, true).unwrap();
    let removable = engine.snapshot(tool, &current.path, &current.revision).unwrap();
    for _ in 0..5 {
        engine.snapshot(tool, &current.path, &current.revision).unwrap();
    }
    let preview = engine.preview_snapshot_cleanup(tool, &current.path).unwrap();
    assert!(!preview.candidates.iter().any(|item| item.id == older.id));
    let cleanup = engine.apply_snapshot_cleanup(tool, &current.path, &preview.revision, &[removable.id]).unwrap();
    assert_eq!(cleanup.removed, 1);
    assert_eq!(fs::read(&files[0]).unwrap(), bytes);
    let snapshots = engine.snapshots(tool).unwrap();
    assert!(snapshots.iter().any(|item| item.id == older.id && item.pinned));
    assert!(!snapshots.iter().any(|item| item.id == removable.id));
    let activity = engine.config_activity(tool).unwrap();
    assert!(activity.iter().any(|event| matches!(&event.action, crate::ActivityAction::Cleanup)
        && matches!(&event.status, crate::ActivityStatus::Succeeded)));
    drop(engine);
    assert!(SystemVault.get(&format!("{prefix}-snapshot-encryption-v1")).unwrap().is_none());
}
