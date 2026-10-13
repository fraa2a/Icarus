use crate::api::Result;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("icarus")
        .invoke_handler(tauri::generate_handler![
            migration_instances,
            import_modrinth,
            run_copy
        ])
        .build()
}

#[tauri::command]
pub async fn migration_instances()
-> Result<Vec<theseus::icarus::MigrationInstance>> {
    Ok(theseus::icarus::migration_instances().await?)
}

#[tauri::command]
pub async fn import_modrinth(
    instance_ids: Vec<String>,
    remove_source: bool,
) -> Result<Vec<String>> {
    Ok(theseus::icarus::import_modrinth(instance_ids, remove_source).await?)
}

#[tauri::command]
pub async fn run_copy(
    instance_id: &str,
    account_id: uuid::Uuid,
) -> Result<theseus::data::ProcessMetadata> {
    Ok(theseus::icarus::run_copy(instance_id, account_id).await?)
}
