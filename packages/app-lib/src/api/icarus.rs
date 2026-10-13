//! Import Modrinth App instances and launch independent account copies.
use crate::state::{
    Credentials, DirectoryInfo, EditInstance, InstanceLink, ModLoader,
    ProcessMetadata, State,
};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::{Component, Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, serde::Serialize)]
pub struct MigrationInstance {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    #[serde(skip)]
    path: String,
    #[serde(skip)]
    loader_version: Option<String>,
    #[serde(skip)]
    icon_path: Option<String>,
}

fn input(message: &str) -> crate::Error {
    crate::ErrorKind::InputError(message.to_owned()).into()
}

async fn source() -> crate::Result<Option<(PathBuf, PathBuf, SqlitePool, bool)>>
{
    let Some(settings) =
        DirectoryInfo::initial_settings_dir_path("com.modrinth.ModrinthApp")
    else {
        return Ok(None);
    };
    let database = settings.join("app.db");
    if !database.is_file() {
        return Ok(None);
    }
    let settings = fs::canonicalize(settings).await?;
    let state = State::get().await?;
    if settings == fs::canonicalize(&state.directories.settings_dir).await? {
        return Ok(None);
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(database)
                .read_only(true),
        )
        .await?;
    let custom: Option<String> =
        sqlx::query_scalar("SELECT custom_dir FROM settings LIMIT 1")
            .fetch_optional(&pool)
            .await?
            .flatten();
    let root = fs::canonicalize(
        custom
            .map(PathBuf::from)
            .unwrap_or_else(|| settings.clone()),
    )
    .await?;
    if root == fs::canonicalize(&state.directories.config_dir).await? {
        return Err(input(
            "Modrinth App and Icarus use the same instance directory",
        ));
    }
    let modern: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='instances'").fetch_one(&pool).await?;
    Ok(Some((settings, root, pool, modern != 0)))
}

async fn candidates(
    pool: &SqlitePool,
    modern: bool,
) -> crate::Result<Vec<MigrationInstance>> {
    let query = if modern {
        "SELECT i.id,i.path,i.name,i.icon_path,c.game_version,c.loader,c.loader_version FROM instances i JOIN instance_content_sets c ON c.id=i.applied_content_set_id"
    } else {
        "SELECT path AS id,path,name,icon_path,game_version,mod_loader AS loader,mod_loader_version AS loader_version FROM profiles"
    };
    Ok(sqlx::query(query)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| MigrationInstance {
            id: row.get("id"),
            path: row.get("path"),
            name: row.get("name"),
            icon_path: row.get("icon_path"),
            game_version: row.get("game_version"),
            loader: ModLoader::from_string(row.get("loader")),
            loader_version: row.get("loader_version"),
        })
        .collect())
}

pub async fn migration_instances() -> crate::Result<Vec<MigrationInstance>> {
    let Some((settings, _, pool, modern)) = source().await? else {
        return Ok(vec![]);
    };
    let state = State::get().await?;
    let mut available = Vec::new();
    for candidate in candidates(&pool, modern).await? {
        let key =
            format!("icarus.import:{}:{}", settings.display(), candidate.id);
        let imported: i64 = sqlx::query_scalar("SELECT count(*) FROM app_metadata WHERE key=? AND value IN (SELECT id FROM instances)").bind(key).fetch_one(&state.pool).await?;
        if imported == 0 {
            available.push(candidate);
        }
    }
    Ok(available)
}

fn ensure_source_stopped(root: &Path) -> crate::Result<()> {
    let system = sysinfo::System::new_all();
    if system.processes().values().any(|process| {
        let name = process.name().to_string_lossy().to_lowercase();
        name.contains("modrinth")
            || process.cwd().is_some_and(|cwd| cwd.starts_with(root))
    }) {
        return Err(input(
            "Close Modrinth App and its running Minecraft instances before migrating",
        ));
    }
    Ok(())
}

fn confined(root: &Path, relative: &str) -> crate::Result<PathBuf> {
    if relative.is_empty()
        || relative.contains('\\')
        || Path::new(relative)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(input("Unsafe instance path"));
    }
    let mut path = root.to_path_buf();
    for part in Path::new(relative).components() {
        path.push(part);
        if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(input("Symbolic links cannot be copied"));
        }
    }
    let path = std::fs::canonicalize(path)?;
    if !path.starts_with(root) {
        return Err(input("Instance path escapes its directory"));
    }
    Ok(path)
}

async fn copy_files(
    source: &Path,
    destination: &Path,
    skip_worlds: bool,
) -> crate::Result<()> {
    let mut pending = vec![(source.to_path_buf(), destination.to_path_buf())];
    while let Some((from, to)) = pending.pop() {
        fs::create_dir_all(&to).await?;
        let mut entries = fs::read_dir(&from).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            if skip_worlds
                && from == source
                && matches!(
                    name.to_str(),
                    Some("saves" | "logs" | "crash-reports")
                )
            {
                continue;
            }
            let kind = entry.file_type().await?;
            let target = to.join(name);
            if kind.is_symlink() {
                return Err(input("Symbolic links cannot be copied"));
            }
            if kind.is_dir() {
                if fs::symlink_metadata(&target)
                    .await
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    return Err(input("Unsafe destination link"));
                }
                pending.push((entry.path(), target));
            } else if kind.is_file() {
                let before =
                    crate::util::fetch::sha1_file_async(&entry.path()).await?;
                let temporary =
                    to.join(format!(".icarus-copy-{}", Uuid::new_v4()));
                fs::copy(entry.path(), &temporary).await?;
                if before
                    != crate::util::fetch::sha1_file_async(&temporary).await?
                    || before
                        != crate::util::fetch::sha1_file_async(&entry.path())
                            .await?
                {
                    let _ = fs::remove_file(temporary).await;
                    return Err(input(
                        "Source files changed while copying; please retry after closing Minecraft",
                    ));
                }
                if fs::symlink_metadata(&target)
                    .await
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    let _ = fs::remove_file(temporary).await;
                    return Err(input("Unsafe destination link"));
                }
                if target.exists() {
                    fs::remove_file(&target).await?;
                }
                fs::rename(temporary, target).await?;
            } else {
                return Err(input(
                    "Only regular files and directories can be copied",
                ));
            }
        }
    }
    Ok(())
}

async fn create_copy(
    name: String,
    game_version: String,
    loader: ModLoader,
    loader_version: Option<String>,
    icon_path: Option<String>,
    icon_config: Option<crate::state::InstanceIconConfig>,
    source: &Path,
    skip_worlds: bool,
) -> crate::Result<String> {
    let job = crate::install::create_instance(
        name,
        game_version,
        loader,
        loader_version,
        icon_path,
        icon_config,
        InstanceLink::Unmanaged,
    )
    .await?;
    let job_id = Uuid::parse_str(&job.job_id)?;
    crate::install::runner::wait_for_job(job_id).await?;
    let completed = crate::install::get_job(job_id).await?;
    let id = completed
        .instance_id
        .ok_or_else(|| input("Installation completed without an instance"))?;
    let result = async {
        for option in crate::state::InstanceSyncedOption::ALL {
            if option.is_available() {
                crate::instance::set_synced_option(&id, option, false, None)
                    .await?;
            }
        }
        let destination = crate::instance::get_full_path(&id).await?;
        copy_files(source, &destination, skip_worlds).await?;
        crate::instance::sync_content_files(&id).await?;
        Ok(id.clone())
    }
    .await;
    if result.is_err() {
        let _ = crate::instance::remove(&id).await;
    }
    result
}

pub async fn import_modrinth(
    instance_ids: Vec<String>,
    remove_source: bool,
) -> crate::Result<Vec<String>> {
    let state = State::get().await?;
    let Some((settings, root, pool, modern)) = source().await? else {
        return Err(input("Modrinth App was not found"));
    };
    ensure_source_stopped(&root)?;
    let _settings_lock =
        crate::state::content_store::ContentStore::lock_process(&settings)
            .await?;
    let _root_lock = if root != settings {
        Some(
            crate::state::content_store::ContentStore::lock_process(&root)
                .await?,
        )
    } else {
        None
    };
    let profiles = confined(&root, "profiles")?;
    let all = candidates(&pool, modern).await?;
    if instance_ids
        .iter()
        .any(|id| !all.iter().any(|candidate| &candidate.id == id))
    {
        return Err(input("An instance selection is no longer available"));
    }
    let mut imported = Vec::new();
    for candidate in all
        .into_iter()
        .filter(|candidate| instance_ids.contains(&candidate.id))
    {
        let path = confined(&profiles, &candidate.path)?;
        let checkpoint =
            format!("icarus.import:{}:{}", settings.display(), candidate.id);
        let id = if let Some(id) = sqlx::query_scalar::<_,String>("SELECT value FROM app_metadata WHERE key=? AND value IN (SELECT id FROM instances)").bind(&checkpoint).fetch_optional(&state.pool).await? { id } else {
			let icon = candidate.icon_path.as_deref().and_then(|icon| {
				let path = PathBuf::from(icon);
				let path = if path.is_absolute() { path } else { root.join("caches").join(path) };
				std::fs::canonicalize(path).ok().filter(|path| path.starts_with(&root)).map(|path| path.to_string_lossy().into_owned())
			});
			let id = create_copy(candidate.name, candidate.game_version, candidate.loader, candidate.loader_version, icon, None, &path, false).await?;
			if modern {
				let overrides: Option<String> = sqlx::query_scalar("SELECT overrides FROM instance_launch_overrides WHERE instance_id=?").bind(&candidate.id).fetch_optional(&pool).await?;
				if let Some(overrides) = overrides {
					sqlx::query("UPDATE instance_launch_overrides SET overrides=? WHERE instance_id=?").bind(overrides).bind(&id).execute(&state.pool).await?;
				}
			}
			sqlx::query("INSERT INTO app_metadata(key,value,updated_at) VALUES(?,?,unixepoch()) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at").bind(&checkpoint).bind(&id).execute(&state.pool).await?;
			id
		};
        if remove_source {
            ensure_source_stopped(&root)?;
            let backup =
                profiles.join(format!(".icarus-migrated-{}", Uuid::new_v4()));
            fs::rename(&path, &backup).await?;
            let removal = async {
                let writable = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(
                        SqliteConnectOptions::new()
                            .filename(settings.join("app.db"))
                            .foreign_keys(true),
                    )
                    .await?;
                let query = if modern {
                    "DELETE FROM instances WHERE id=?"
                } else {
                    "DELETE FROM profiles WHERE path=?"
                };
                sqlx::query(query)
                    .bind(&candidate.id)
                    .execute(&writable)
                    .await?;
                writable.close().await;
                Ok::<_, crate::Error>(())
            }
            .await;
            if let Err(error) = removal {
                fs::rename(&backup, &path).await?;
                return Err(error);
            }
        }
        imported.push(id);
    }
    Ok(imported)
}

pub async fn run_copy(
    instance_id: &str,
    account_id: Uuid,
) -> crate::Result<ProcessMetadata> {
    let state = State::get().await?;
    let accounts = Credentials::get_all(&state.pool).await?;
    let credentials = accounts
        .remove(&account_id)
        .ok_or_else(|| {
            input("The selected Minecraft account is no longer available")
        })?
        .1;
    let source = crate::instance::get(instance_id)
        .await?
        .ok_or_else(|| input("Unknown instance"))?;
    if source.quarantined {
        return Err(input("This instance has been quarantined"));
    }
    let running = !crate::process::get_by_instance_id(instance_id)
        .await?
        .is_empty();
    let path = crate::instance::get_full_path(instance_id).await?;
    let id = create_copy(
        format!(
            "{} ({})",
            source.instance.name, credentials.offline_profile.name
        ),
        source.applied_content_set.game_version,
        source.applied_content_set.loader,
        source.applied_content_set.loader_version,
        source.instance.icon_path,
        source.icon_config,
        &path,
        running,
    )
    .await?;
    let overrides = source.launch_overrides;
    crate::instance::edit(
        &id,
        EditInstance {
            launch_overrides: Some(
                crate::state::InstanceLaunchOverridesPatch {
                    java_path: Some(overrides.java_path),
                    extra_launch_args: Some(overrides.extra_launch_args),
                    custom_env_vars: Some(overrides.custom_env_vars),
                    memory: Some(overrides.memory),
                    force_fullscreen: Some(overrides.force_fullscreen),
                    game_resolution: Some(overrides.game_resolution),
                    hooks: Some(overrides.hooks),
                    visible_tabs: Some(overrides.visible_tabs),
                },
            ),
            ..Default::default()
        },
    )
    .await?;
    crate::instance::run_as_account(&id, credentials).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_escaping_paths() {
        let directory = tempfile::tempdir().unwrap();
        for path in [
            "../outside",
            "/absolute",
            "a/../../outside",
            "a\\..\\outside",
            "",
        ] {
            assert!(confined(directory.path(), path).is_err());
        }
    }
    #[tokio::test]
    async fn copies_files_independently_and_skips_live_saves() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        fs::write(source.path().join("options.txt"), b"original")
            .await
            .unwrap();
        fs::create_dir(source.path().join("saves")).await.unwrap();
        fs::write(source.path().join("saves/world"), b"world")
            .await
            .unwrap();
        copy_files(source.path(), destination.path(), true)
            .await
            .unwrap();
        fs::write(destination.path().join("options.txt"), b"copy")
            .await
            .unwrap();
        assert_eq!(
            fs::read(source.path().join("options.txt")).await.unwrap(),
            b"original"
        );
        assert!(!destination.path().join("saves").exists());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_source_symlinks() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", source.path().join("escape"))
            .unwrap();
        assert!(confined(source.path(), "escape").is_err());
        assert!(
            copy_files(source.path(), destination.path(), false)
                .await
                .is_err()
        );
    }
}
