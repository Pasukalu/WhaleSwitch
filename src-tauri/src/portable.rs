use crate::engine::{directory_exists, metadata, unsafe_meta, validate_ancestors};
use crate::{Error, Result};
use std::{fs, io, path::{Path, PathBuf}};

const MARKER: &str = "WhaleSwitch.portable";
const DATA_FOLDER: &str = "WhaleSwitchData";

pub(crate) struct PortableLayout {
    pub data: PathBuf,
    pub webview: PathBuf,
}

pub(crate) fn prepare(executable: &Path) -> Result<Option<PortableLayout>> {
    let parent = executable.parent().ok_or(Error::UnsafePath)?;
    let marker = parent.join(MARKER);
    let info = match marker.symlink_metadata() {
        Ok(info) => info,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Io),
    };
    validate_ancestors(&marker)?;
    if unsafe_meta(&info) || !info.is_file() || info.len() != 0 {
        return Err(Error::UnsafePath);
    }

    let data = parent.join(DATA_FOLDER);
    prepare_directory(&data)?;
    for name in [
        "write.lock", "snapshots.sqlite3", "snapshots.sqlite3-journal",
        "snapshots.sqlite3-wal", "snapshots.sqlite3-shm",
    ] {
        metadata(&data.join(name))?;
    }
    let webview = data.join("webview");
    prepare_directory(&webview)?;
    Ok(Some(PortableLayout { data, webview }))
}

fn prepare_directory(path: &Path) -> Result<()> {
    if !directory_exists(path)? {
        match fs::create_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(Error::Io),
        }
        if !directory_exists(path)? { return Err(Error::Io); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_binds_state_and_webview_to_executable_folder() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let executable = root.join("WhaleSwitch.exe");
        assert!(prepare(&executable).unwrap().is_none());
        assert!(!root.join(DATA_FOLDER).exists());
        fs::write(root.join(MARKER), []).unwrap();
        let layout = prepare(&executable).unwrap().unwrap();
        assert_eq!(layout.data, root.join(DATA_FOLDER));
        assert_eq!(layout.webview, root.join(DATA_FOLDER).join("webview"));
        assert!(layout.webview.is_dir());
        assert_eq!(prepare(&executable).unwrap().unwrap().data, layout.data);
    }

    #[test]
    fn rejects_a_non_directory_data_folder_and_non_empty_marker() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let executable = root.join("WhaleSwitch.exe");
        fs::write(root.join(MARKER), "unknown format").unwrap();
        assert!(matches!(prepare(&executable), Err(Error::UnsafePath)));
        fs::write(root.join(MARKER), []).unwrap();
        fs::write(root.join(DATA_FOLDER), "keep this file").unwrap();
        assert!(matches!(prepare(&executable), Err(Error::UnsafePath)));
        assert_eq!(fs::read_to_string(root.join(DATA_FOLDER)).unwrap(), "keep this file");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_link_to_an_existing_database() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let executable = root.join("WhaleSwitch.exe");
        fs::write(root.join(MARKER), []).unwrap();
        fs::create_dir(root.join(DATA_FOLDER)).unwrap();
        fs::write(root.join("other.sqlite3"), b"fixture").unwrap();
        symlink(root.join("other.sqlite3"), root.join(DATA_FOLDER).join("snapshots.sqlite3")).unwrap();
        assert!(matches!(prepare(&executable), Err(Error::UnsafePath)));
        assert_eq!(fs::read(root.join("other.sqlite3")).unwrap(), b"fixture");
    }
}
