//! Strict, local-only startup and conservative migration of existing user data.

use crate::api::model::{Config, HeaderIdentity, Session};
use serde::de::DeserializeOwned;
use std::fmt;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

const FILES: &[&str] = &[
    "identity.json",
    "config.json",
    "session.json",
    "points_cache.json",
    "ai_sports.json",
    "semester_result.json",
    "rank_result.json",
];
const DIRECTORIES: &[&str] = &["school_roads", "submission_receipts"];
static INITIALIZATION: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub(crate) struct InitialState {
    pub identity: HeaderIdentity,
    pub session: Session,
    pub config: Config,
}

#[derive(Debug)]
pub(crate) struct StartupError(String);

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn error(path: &Path, operation: &str, detail: impl fmt::Display) -> StartupError {
    StartupError(format!("{operation} {}：{detail}", path.display()))
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, StartupError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(value) if value.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(value) => return Err(error(path, "无法读取", value)),
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|value| {
        error(
            path,
            "JSON 格式或字段无效",
            format!(
                "第 {} 行，第 {} 列；请备份并修复原文件后重试",
                value.line(),
                value.column()
            ),
        )
    })
}

fn has_state(root: &Path) -> bool {
    FILES
        .iter()
        .chain(DIRECTORIES)
        .any(|name| root.join(name).exists())
}

fn copy_json_tree(source: &Path, destination: &Path) -> Result<(), StartupError> {
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|value| error(source, "无法检查迁移源", value))?;
    if metadata.file_type().is_symlink() {
        return Err(error(source, "拒绝迁移符号链接", "请手动检查数据目录"));
    }
    if metadata.is_dir() {
        std::fs::create_dir(destination)
            .map_err(|value| error(destination, "无法创建迁移目录", value))?;
        for item in
            std::fs::read_dir(source).map_err(|value| error(source, "无法读取迁移目录", value))?
        {
            let item = item.map_err(|value| error(source, "无法读取迁移条目", value))?;
            copy_json_tree(&item.path(), &destination.join(item.file_name()))?;
        }
    } else if metadata.is_file()
        && source
            .extension()
            .is_some_and(|extension| extension == "json")
    {
        read_json::<serde_json::Value>(source)?;
        std::fs::copy(source, destination)
            .map_err(|value| error(destination, "无法复制迁移数据", value))?;
    } else {
        return Err(error(
            source,
            "迁移目录包含非 JSON 文件",
            "请手动检查，原件保持不变",
        ));
    }
    Ok(())
}

fn migrate(root: &Path, legacy: &Path) -> Result<(), StartupError> {
    if root == legacy || has_state(root) || !has_state(legacy) {
        return Ok(());
    }
    // Validate the complete old state before copying any of it to the new root.
    read_json::<HeaderIdentity>(&legacy.join("identity.json"))?;
    read_json::<Config>(&legacy.join("config.json"))?;
    read_json::<Session>(&legacy.join("session.json"))?;
    if root.exists()
        && std::fs::read_dir(root)
            .map_err(|value| error(root, "无法读取目标目录", value))?
            .next()
            .is_some()
    {
        return Err(error(
            root,
            "无法自动迁移到非空目录",
            "请选择空目录，或手动迁移；不会混合两套数据",
        ));
    }
    let parent = root
        .parent()
        .ok_or_else(|| error(root, "无效数据目录", "缺少父目录"))?;
    std::fs::create_dir_all(parent).map_err(|value| error(parent, "无法创建父目录", value))?;
    let staging = parent.join(format!(".nswt-migration-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&staging)
        .map_err(|value| error(&staging, "无法创建迁移暂存目录", value))?;
    let outcome = (|| {
        for name in FILES.iter().chain(DIRECTORIES) {
            let source = legacy.join(name);
            if source.exists() {
                copy_json_tree(&source, &staging.join(name))?;
            }
        }
        if root.exists() {
            // Only remove an empty destination. Existing files are never removed.
            std::fs::remove_dir(root)
                .map_err(|value| error(root, "目标目录已变化，迁移已停止", value))?;
        }
        std::fs::rename(&staging, root).map_err(|value| error(root, "无法完成迁移", value))
    })();
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    outcome
}

pub(crate) fn initialize() -> Result<InitialState, StartupError> {
    let _lock = INITIALIZATION
        .lock()
        .map_err(|_| StartupError("本地初始化锁异常，请退出后重试".into()))?;
    let root = crate::platform::try_data_dir().map_err(StartupError)?;
    let legacy = if !crate::runtime::offline() && std::env::var_os("NSWT_DATA_DIR").is_none() {
        std::env::current_exe()
            .ok()
            .and_then(|value| value.parent().map(Path::to_path_buf))
    } else {
        None
    };
    initialize_at(&root, legacy.as_deref())
}

fn initialize_at(root: &Path, legacy: Option<&Path>) -> Result<InitialState, StartupError> {
    if let Some(legacy) = legacy {
        migrate(root, legacy)?;
    }
    std::fs::create_dir_all(root).map_err(|value| error(root, "无法创建数据目录", value))?;
    let probe = root.join(format!(".write-check-{}", uuid::Uuid::new_v4()));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|value| error(root, "数据目录不可写", value))?;
    std::fs::remove_file(&probe).map_err(|value| error(&probe, "无法清理写入检查文件", value))?;
    let config = read_json(&root.join("config.json"))?.unwrap_or_default();
    let session = read_json(&root.join("session.json"))?.unwrap_or_default();
    let identity_path = root.join("identity.json");
    let identity = match read_json::<HeaderIdentity>(&identity_path)? {
        Some(identity) => identity,
        None => {
            let mut identity = HeaderIdentity::default();
            identity.device_id = uuid::Uuid::new_v4().to_string().to_uppercase();
            identity.app_install_time = HeaderIdentity::fresh_install_time(&identity.platform);
            identity.mac_address = crate::crypto::header::random_mac();
            let bytes = serde_json::to_vec_pretty(&identity)
                .map_err(|_| StartupError("无法创建本地设备身份".into()))?;
            publish_identity(&identity_path, &bytes).map_err(StartupError)?;
            // Another process may have won first creation. Always use the identity
            // actually published to disk, never a losing in-memory candidate.
            read_json::<HeaderIdentity>(&identity_path)?.ok_or_else(|| {
                error(
                    &identity_path,
                    "设备身份创建后不可读",
                    "请检查数据目录后重试",
                )
            })?
        }
    };
    if identity.device_id.is_empty()
        || identity.app_install_time <= 0
        || identity.mac_address.is_empty()
    {
        return Err(error(
            &identity_path,
            "设备身份字段不完整",
            "请保留并修复原文件，不会生成替代身份",
        ));
    }
    Ok(InitialState {
        identity,
        session,
        config,
    })
}

/// Publish complete first-install identity bytes without replacing another
/// process's identity. A same-directory hard link is an atomic no-clobber step
/// on APFS/NTFS. Unsupported filesystems fail visibly rather than overwrite.
fn publish_identity(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("设备身份缺少父目录")?;
    let temporary = parent.join(format!(".nswt-identity-{}", uuid::Uuid::new_v4()));
    let outcome = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        match std::fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(value) if value.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(value) => Err(value),
        }
    })();
    let _ = std::fs::remove_file(&temporary);
    outcome.map_err(|value: std::io::Error| {
        format!(
            "无法安全创建设备身份 {}：{value}；不会覆盖已有身份",
            path.display()
        )
    })
}

/// Replace a JSON file only after the complete new contents have been synced.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("数据文件缺少父目录")?;
    std::fs::create_dir_all(parent)
        .map_err(|value| format!("无法创建 {}：{value}", parent.display()))?;
    let temporary = parent.join(format!(".nswt-write-{}", uuid::Uuid::new_v4()));
    let outcome = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if temporary.exists() {
        let _ = std::fs::remove_file(&temporary);
    }
    outcome.map_err(|value: std::io::Error| format!("无法保存 {}：{value}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("nswt-startup-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn new_install_persists_and_reuses_the_same_identity() {
        let directory = Directory::new();
        let first = initialize_at(&directory.0, None).unwrap();
        let bytes = std::fs::read(directory.0.join("identity.json")).unwrap();
        let again = initialize_at(&directory.0, None).unwrap();
        assert_eq!(first.identity.device_id, again.identity.device_id);
        assert_eq!(
            bytes,
            std::fs::read(directory.0.join("identity.json")).unwrap()
        );
    }

    #[test]
    fn concurrent_first_install_never_replaces_the_winning_identity() {
        let directory = Directory::new();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let root = directory.0.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    initialize_at(&root, None).unwrap().identity.device_id
                })
            })
            .collect();
        let stored = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert!(stored.iter().all(|identity| identity == &stored[0]));
        let on_disk: HeaderIdentity = read_json(&directory.0.join("identity.json"))
            .unwrap()
            .unwrap();
        assert_eq!(on_disk.device_id, stored[0]);
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn corrupt_private_files_are_not_replaced_or_exposed() {
        for name in ["identity.json", "config.json", "session.json"] {
            let directory = Directory::new();
            let bytes = b"{private-test-secret:broken";
            std::fs::write(directory.0.join(name), bytes).unwrap();
            let message = initialize_at(&directory.0, None).err().unwrap().to_string();
            assert!(message.contains(name));
            assert!(!message.contains("private-test-secret"));
            assert_eq!(
                bytes.as_slice(),
                std::fs::read(directory.0.join(name)).unwrap()
            );
            if name != "identity.json" {
                assert!(!directory.0.join("identity.json").exists());
            }
        }
    }

    #[test]
    fn migration_preserves_original_bytes_unknown_fields_and_receipts() {
        let base = Directory::new();
        let old = base.0.join("旧版 路径");
        let new = base.0.join("新 数据");
        std::fs::create_dir(&old).unwrap();
        initialize_at(&old, None).unwrap();
        let mut identity: serde_json::Value =
            read_json(&old.join("identity.json")).unwrap().unwrap();
        identity["future_field"] = serde_json::json!("keep-me");
        let bytes = serde_json::to_vec(&identity).unwrap();
        std::fs::write(old.join("identity.json"), &bytes).unwrap();
        std::fs::create_dir(old.join("submission_receipts")).unwrap();
        std::fs::write(
            old.join("submission_receipts/test.json"),
            b"{\"status\":\"unknown\"}",
        )
        .unwrap();
        initialize_at(&new, Some(&old)).unwrap();
        assert_eq!(std::fs::read(new.join("identity.json")).unwrap(), bytes);
        assert_eq!(std::fs::read(old.join("identity.json")).unwrap(), bytes);
        assert!(new.join("submission_receipts/test.json").is_file());
    }

    #[test]
    fn existing_target_is_not_mixed_with_another_identity() {
        let base = Directory::new();
        let old = base.0.join("old");
        let new = base.0.join("new");
        let first = initialize_at(&old, None).unwrap();
        let second = initialize_at(&new, None).unwrap();
        assert_ne!(first.identity.device_id, second.identity.device_id);
        let loaded = initialize_at(&new, Some(&old)).unwrap();
        assert_eq!(second.identity.device_id, loaded.identity.device_id);
    }

    #[test]
    fn corrupt_migration_never_creates_a_partial_target() {
        let base = Directory::new();
        let old = base.0.join("old");
        let new = base.0.join("new");
        initialize_at(&old, None).unwrap();
        std::fs::write(old.join("config.json"), b"{broken").unwrap();
        assert!(initialize_at(&new, Some(&old)).is_err());
        assert!(!new.exists());
        std::fs::write(old.join("config.json"), b"{}").unwrap();
        assert!(initialize_at(&new, Some(&old)).is_ok());
    }

    #[test]
    fn a_file_cannot_be_used_as_a_writable_data_directory() {
        let base = Directory::new();
        let file = base.0.join("not-a-directory");
        std::fs::write(&file, b"unchanged").unwrap();
        assert!(initialize_at(&file, None).is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"unchanged");
    }

    #[test]
    fn atomic_update_replaces_the_file_without_leaving_temporary_files() {
        let base = Directory::new();
        let path = base.0.join("config.json");
        atomic_write(&path, b"{\"one\":1}").unwrap();
        atomic_write(&path, b"{\"two\":2}").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"two\":2}");
        assert_eq!(std::fs::read_dir(&base.0).unwrap().count(), 1);
    }
}
