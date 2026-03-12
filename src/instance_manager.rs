//! Instance manager: CRUD and port allocation for cluster instances.
//!
//! Used by admin to create/list/delete instances. Port pool starts at 42618
//! (admin uses 42617).

use crate::instance_registry::{
    InstanceEntry, InstanceRegistry, InstanceRole, InstanceStatus,
};
use anyhow::{bail, Context, Result};
use std::path::Path;
use tokio::fs;

const PORT_POOL_START: u16 = 42618;
const INSTANCES_DIR: &str = "instances";

/// Minimal config.toml for a new normal instance (port is injected).
/// When preset is startup or enterprise, appends [instance] and [instance.ceo] so CEO is enabled.
fn minimal_instance_config(port: u16, preset: Option<&str>) -> String {
    let base = format!(
        r#"default_temperature = 0.7

[gateway]
port = {port}
host = "127.0.0.1"
require_pairing = false

[secrets]
encrypt = false

[channels_config]
cli = true
"#
    );
    let scaffold_ceo = preset
        .map(|p| p.eq_ignore_ascii_case("startup") || p.eq_ignore_ascii_case("enterprise"))
        .unwrap_or(false);
    if scaffold_ceo {
        let preset_val = preset.unwrap_or("startup");
        format!(
            r#"{base}[instance]
preset = "{preset_val}"

[instance.ceo]
"#
        )
    } else {
        base
    }
}

/// Allocate the next available port >= PORT_POOL_START from the registry.
fn allocate_port(reg: &InstanceRegistry) -> u16 {
    let used: std::collections::HashSet<u16> = reg.allocated_ports().collect();
    (PORT_POOL_START..=65535).find(|p| !used.contains(p)).unwrap_or(42618)
}

/// Create a new instance: directories, minimal config, registry entry.
/// Fails if id exists or id is "admin".
pub async fn instance_create(
    cluster_root: &Path,
    id: &str,
    role: InstanceRole,
    preset: Option<&str>,
) -> Result<InstanceEntry> {
    if id == "admin" {
        bail!("Instance id 'admin' is reserved");
    }
    let id_clean = id.trim();
    if id_clean.is_empty() {
        bail!("Instance id must be non-empty");
    }
    if id_clean.contains(std::path::MAIN_SEPARATOR) || id_clean.contains('/') {
        bail!("Instance id must not contain path separators");
    }

    let mut reg = InstanceRegistry::load(cluster_root).await?;
    if reg.get(id_clean).is_some() {
        bail!("Instance '{}' already exists", id_clean);
    }

    let port = allocate_port(&reg);
    let instance_dir = cluster_root.join(INSTANCES_DIR).join(id_clean);
    let workspace_dir = instance_dir.join("workspace");
    let config_path = instance_dir.join("config.toml");

    fs::create_dir_all(&workspace_dir)
        .await
        .with_context(|| format!("Failed to create instance dir: {}", instance_dir.display()))?;

    fs::write(&config_path, minimal_instance_config(port, preset))
        .await
        .with_context(|| format!("Failed to write instance config: {}", config_path.display()))?;

    crate::entity::scaffold_instance_workspace(&workspace_dir, id_clean)
        .await
        .with_context(|| format!("Failed to scaffold instance workspace: {}", workspace_dir.display()))?;

    // When preset implies CEO (startup/enterprise), scaffold workspace/entities/ceo/ with detailed IDENTITY.md and AGENTS.md.
    let scaffold_ceo = preset
        .map(|p| p.eq_ignore_ascii_case("startup") || p.eq_ignore_ascii_case("enterprise"))
        .unwrap_or(false);
    if scaffold_ceo {
        crate::entity::scaffold_entity_workspace(&workspace_dir, "ceo", Some("CEO"))
            .await
            .with_context(|| "Failed to scaffold CEO entity workspace")?;
    }

    let created_at = chrono::Utc::now().to_rfc3339();
    let entry = InstanceEntry {
        id: id_clean.to_string(),
        role,
        status: InstanceStatus::Created,
        config_path: Some(config_path.to_string_lossy().into_owned()),
        workspace_path: Some(workspace_dir.to_string_lossy().into_owned()),
        gateway_port: Some(port),
        created_at: Some(created_at),
        constraints: None,
        preset: preset.map(String::from),
    };
    reg.add(entry.clone());
    reg.save(cluster_root).await?;

    Ok(entry)
}

/// Soft-delete an instance: set status to Deleted. Does not remove directories.
/// Fails if id is "admin" or instance does not exist.
pub async fn instance_delete(cluster_root: &Path, id: &str) -> Result<()> {
    if id == "admin" {
        bail!("Cannot delete reserved instance 'admin'");
    }
    let mut reg = InstanceRegistry::load(cluster_root).await?;
    if !reg.update_status(id, InstanceStatus::Deleted) {
        bail!("Instance '{}' not found", id);
    }
    reg.save(cluster_root).await?;
    Ok(())
}

/// List all instances from the registry (including deleted).
pub async fn instance_list(cluster_root: &Path) -> Result<Vec<InstanceEntry>> {
    let reg = InstanceRegistry::load(cluster_root).await?;
    Ok(reg.instances.clone())
}

/// Return the entry for an instance if it exists and is not deleted.
pub async fn instance_status(cluster_root: &Path, id: &str) -> Result<Option<InstanceEntry>> {
    let reg = InstanceRegistry::load(cluster_root).await?;
    Ok(reg.get(id).filter(|e| e.status != InstanceStatus::Deleted).cloned())
}

/// Returns true if the current config path is the admin instance (for permission check).
pub fn is_admin_instance(config_path: &Path) -> bool {
    let path_str = config_path.to_string_lossy();
    path_str.contains("instances") && path_str.contains("admin")
        && path_str.ends_with("config.toml")
}

#[cfg(test)]
mod tests {
    use super::{instance_create, instance_delete, instance_list, InstanceRole};
    use crate::instance_registry::InstanceStatus;
    use tokio::fs;

    #[tokio::test]
    async fn instance_manager_create_allocates_port_and_writes_registry() {
        let dir = std::env::temp_dir().join("multiclaw-im-create");
        let _ = fs::remove_dir_all(&dir).await;
        fs::create_dir_all(&dir).await.unwrap();

        let entry = instance_create(&dir, "worker1", InstanceRole::Normal, None)
            .await
            .unwrap();
        assert_eq!(entry.id, "worker1");
        assert!(entry.gateway_port.unwrap() >= 42618);
        assert!(dir.join("instances").join("worker1").join("config.toml").exists());
        assert!(dir.join("instances").join("worker1").join("workspace").exists());

        let list = instance_list(&dir).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "worker1");

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn instance_manager_delete_frees_port() {
        let dir = std::env::temp_dir().join("multiclaw-im-del");
        let _ = fs::remove_dir_all(&dir).await;
        fs::create_dir_all(&dir).await.unwrap();

        instance_create(&dir, "x", InstanceRole::Normal, None).await.unwrap();
        instance_delete(&dir, "x").await.unwrap();
        let list = instance_list(&dir).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, InstanceStatus::Deleted);

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn instance_manager_list_returns_registry_entries() {
        let dir = std::env::temp_dir().join("multiclaw-im-list");
        let _ = fs::remove_dir_all(&dir).await;
        fs::create_dir_all(&dir).await.unwrap();

        assert!(instance_list(&dir).await.unwrap().is_empty());
        instance_create(&dir, "a", InstanceRole::Normal, None).await.unwrap();
        instance_create(&dir, "b", InstanceRole::Normal, Some("default")).await.unwrap();
        let list = instance_list(&dir).await.unwrap();
        assert_eq!(list.len(), 2);
        let ids: Vec<&str> = list.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));

        let _ = fs::remove_dir_all(&dir).await;
    }
}
