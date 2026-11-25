use super::*;

#[test]
fn a_failed_batch_restores_applied_files_in_reverse_order() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("SKILL.md"), "original manifest").unwrap();
    let edits = [
        read_edit(&root, "references/new/guide.md".into(), Some(Zeroizing::new(b"new file".to_vec())), None).unwrap(),
        read_edit(&root, "SKILL.md".into(), Some(Zeroizing::new(b"new manifest".to_vec())), None).unwrap(),
    ];
    let result = execute(&root, &[&edits[0], &edits[1]], |file| {
        if file.ends_with("SKILL.md") { Err(Error::WriteFailed) } else { Ok(()) }
    });
    assert_eq!(result, Err(Error::SkillEditRolledBack));
    assert_eq!(fs::read(root.join("SKILL.md")).unwrap(), b"original manifest");
    assert!(!root.join("references").exists());
}

#[test]
fn compensation_keeps_an_external_write_and_restores_the_remaining_batch() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let reference = root.join("references/notes.md");
    fs::create_dir(reference.parent().unwrap()).unwrap();
    fs::write(&reference, "original notes").unwrap();
    fs::write(root.join("SKILL.md"), "original manifest").unwrap();
    let edits = [
        read_edit(&root, "references/notes.md".into(), Some(Zeroizing::new(b"new notes".to_vec())), None).unwrap(),
        read_edit(&root, "SKILL.md".into(), Some(Zeroizing::new(b"new manifest".to_vec())), None).unwrap(),
    ];
    let result = execute(&root, &[&edits[0], &edits[1]], |file| {
        if file.ends_with("SKILL.md") {
            fs::write(&reference, "external notes").unwrap();
            return Err(Error::WriteFailed);
        }
        Ok(())
    });
    assert_eq!(result, Err(Error::SkillEditRecovery));
    assert_eq!(fs::read(&reference).unwrap(), b"external notes");
    assert_eq!(fs::read(root.join("SKILL.md")).unwrap(), b"original manifest");
}

#[test]
fn allows_only_the_importable_skill_paths_to_be_restored() {
    for path in ["SKILL.md", "references/one.md", "references/nested/deeper/a.txt"] { assert!(editable_path(path)); }
    for path in ["../SKILL.md", "references/../../x.md", "references/nul.txt", "references/.hidden.md",
        "references/one/two/three/a.md", "scripts/file.sh", "references/file.js", "references/d/"] {
        assert!(!editable_path(path));
    }
}
