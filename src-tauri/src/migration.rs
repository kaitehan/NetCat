use std::{fs, path::Path, time::Duration};

use rusqlite::{backup::Backup, Connection, OpenFlags};

use crate::error::AppError;

const LEGACY_DATABASE_NAME: &str = "netcapture.sqlite3";
const CURRENT_DATABASE_NAME: &str = "netcat.sqlite3";
const MIGRATION_DATABASE_NAME: &str = ".netcat.sqlite3.migrating";

pub fn cleanup_legacy_injection_artifacts(data_directory: &Path) -> Result<(), AppError> {
    let directory = data_directory.join("frida");
    match fs::remove_file(directory.join("frida-session.js")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(artifact_cleanup_error(error)),
    }
    let mut entries = fs::read_dir(&directory).map_err(artifact_cleanup_error)?;
    if entries
        .next()
        .transpose()
        .map_err(artifact_cleanup_error)?
        .is_none()
    {
        fs::remove_dir(&directory).map_err(artifact_cleanup_error)?;
    }
    Ok(())
}

fn artifact_cleanup_error(error: std::io::Error) -> AppError {
    AppError::new("ARTIFACT_CLEANUP_FAILED", "无法清理旧版注入会话文件")
        .with_detail(error.to_string())
        .with_action("请检查应用数据目录权限后重试")
}

pub fn migrate_legacy_database(
    legacy_data_directory: &Path,
    current_data_directory: &Path,
) -> Result<bool, AppError> {
    let source = legacy_data_directory.join(LEGACY_DATABASE_NAME);
    let target = current_data_directory.join(CURRENT_DATABASE_NAME);
    if target.exists() || !source.exists() {
        return Ok(false);
    }

    fs::create_dir_all(current_data_directory).map_err(migration_file_error)?;
    let temporary_target = current_data_directory.join(MIGRATION_DATABASE_NAME);
    if temporary_target.exists() {
        fs::remove_file(&temporary_target).map_err(migration_file_error)?;
    }

    let migration_result = backup_database(&source, &temporary_target);
    if let Err(error) = migration_result {
        let _ = fs::remove_file(&temporary_target);
        return Err(error);
    }
    fs::rename(&temporary_target, &target).map_err(|error| {
        let _ = fs::remove_file(&temporary_target);
        migration_file_error(error)
    })?;
    Ok(true)
}

#[cfg(target_os = "macos")]
pub fn migrate_legacy_macos_database(current_data_directory: &Path) -> Result<bool, AppError> {
    let Some(application_support_directory) = current_data_directory.parent() else {
        return Ok(false);
    };
    migrate_legacy_database(
        &application_support_directory.join("com.netcapture.desktop"),
        current_data_directory,
    )
}

#[cfg(not(target_os = "macos"))]
pub fn migrate_legacy_macos_database(_current_data_directory: &Path) -> Result<bool, AppError> {
    Ok(false)
}

fn backup_database(source: &Path, target: &Path) -> Result<(), AppError> {
    let source_connection = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(migration_database_error)?;
    let mut target_connection = Connection::open(target).map_err(migration_database_error)?;
    let backup = Backup::new(&source_connection, &mut target_connection)
        .map_err(migration_database_error)?;
    backup
        .run_to_completion(128, Duration::from_millis(10), None)
        .map_err(migration_database_error)
}

fn migration_database_error(error: rusqlite::Error) -> AppError {
    AppError::new("BRAND_MIGRATION_FAILED", "无法迁移 NetCapture 本地数据")
        .with_detail(error.to_string())
        .with_action("请关闭旧版应用并检查应用数据目录权限后重试")
}

fn migration_file_error(error: std::io::Error) -> AppError {
    AppError::new("BRAND_MIGRATION_FAILED", "无法迁移 NetCapture 本地数据")
        .with_detail(error.to_string())
        .with_action("请检查应用数据目录权限后重试")
}
