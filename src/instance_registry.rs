//! Instance registry: `instances.json` at cluster root.
//!
//! Used in cluster mode for multi-instance layout. Single-instance mode
//! has no `instances.json` and no `instances/` directory.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

const REGISTRY_VERSION: u32 = 1;
const INSTANCES_FILENAME: &str = "instances.json";

/// Role of an instance in the cluster.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceRole {
    Admin,
    Normal,
}

/// Runtime status of an instance.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceStatus {
    Created,
    Running,
    Stopped,
    Deleted,
}

/// Optional constraints for an instance (agent_max, call_quota, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstanceConstraints {
    /// Max number of entities (agents) in this instance.
    pub agent_max: Option<u32>,
    /// Call quota per period (optional).
    pub call_quota: Option<u64>,
    /// Cost limit per day in cents (optional).
    pub cost_per_day_cents: Option<u64>,
}

/// One entry in the instance registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceEntry {
    pub id: String,
    pub role: InstanceRole,
    pub status: InstanceStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<InstanceConstraints>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

/// Registry of instances (instances.json content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRegistry {
    pub version: u32,
    pub instances: Vec<InstanceEntry>,
}

impl Default for InstanceRegistry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            instances: Vec::new(),
        }
    }
}

impl InstanceRegistry {
    /// Path to instances.json under the given cluster root.
    pub fn path_for_cluster_root(cluster_root: &Path) -> PathBuf {
        cluster_root.join(INSTANCES_FILENAME)
    }

    /// Load registry from cluster root. If file is missing, returns default (empty) registry.
    pub async fn load(cluster_root: &Path) -> Result<Self> {
        let path = Self::path_for_cluster_root(cluster_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read instance registry: {}", path.display()))?;
        let registry: InstanceRegistry = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse instance registry: {}", path.display()))?;
        Ok(registry)
    }

    /// Save registry to cluster root.
    pub async fn save(&self, cluster_root: &Path) -> Result<()> {
        let path = Self::path_for_cluster_root(cluster_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        let contents = serde_json::to_string_pretty(self)
            .context("Failed to serialize instance registry")?;
        fs::write(&path, contents)
            .await
            .with_context(|| format!("Failed to write instance registry: {}", path.display()))?;
        Ok(())
    }

    /// Get entry by id.
    pub fn get(&self, id: &str) -> Option<&InstanceEntry> {
        self.instances.iter().find(|e| e.id == id)
    }

    /// Get mutable entry by id.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut InstanceEntry> {
        self.instances.iter_mut().find(|e| e.id == id)
    }

    /// Add an entry. Does not check for duplicate id.
    pub fn add(&mut self, entry: InstanceEntry) {
        self.instances.push(entry);
    }

    /// Update status of an instance by id. Returns true if found and updated.
    pub fn update_status(&mut self, id: &str, status: InstanceStatus) -> bool {
        if let Some(entry) = self.get_mut(id) {
            entry.status = status;
            return true;
        }
        false
    }

    /// Allocated gateway ports (for port pool).
    pub fn allocated_ports(&self) -> impl Iterator<Item = u16> + '_ {
        self.instances
            .iter()
            .filter_map(|e| e.gateway_port)
    }
}

#[cfg(test)]
mod tests {
    use super::{InstanceEntry, InstanceRegistry, InstanceRole, InstanceStatus};

    #[tokio::test]
    async fn instance_registry_load_returns_default_when_file_missing() {
        let dir = std::env::temp_dir().join("multiclaw-ir-missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reg = InstanceRegistry::load(&dir).await.unwrap();
        assert_eq!(reg.version, 1);
        assert!(reg.instances.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn instance_registry_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("multiclaw-ir-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut reg = InstanceRegistry::default();
        reg.add(InstanceEntry {
            id: "admin".to_string(),
            role: InstanceRole::Admin,
            status: InstanceStatus::Running,
            config_path: Some("/path/to/config.toml".to_string()),
            workspace_path: Some("/path/to/workspace".to_string()),
            gateway_port: Some(42617),
            created_at: Some("2026-01-01T00:00:00Z".to_string()),
            constraints: None,
            preset: None,
        });
        reg.save(&dir).await.unwrap();
        let loaded = InstanceRegistry::load(&dir).await.unwrap();
        assert_eq!(loaded.instances.len(), 1);
        assert_eq!(loaded.instances[0].id, "admin");
        assert_eq!(loaded.instances[0].gateway_port, Some(42617));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn instance_registry_get_returns_entry_by_id() {
        let mut reg = InstanceRegistry::default();
        reg.add(InstanceEntry {
            id: "alpha".to_string(),
            role: InstanceRole::Normal,
            status: InstanceStatus::Created,
            config_path: None,
            workspace_path: None,
            gateway_port: Some(42618),
            created_at: None,
            constraints: None,
            preset: None,
        });
        assert!(reg.get("alpha").is_some());
        assert!(reg.get("beta").is_none());
    }

    #[tokio::test]
    async fn instance_registry_add_and_update_status() {
        let mut reg = InstanceRegistry::default();
        reg.add(InstanceEntry {
            id: "x".to_string(),
            role: InstanceRole::Normal,
            status: InstanceStatus::Created,
            config_path: None,
            workspace_path: None,
            gateway_port: None,
            created_at: None,
            constraints: None,
            preset: None,
        });
        assert!(reg.update_status("x", InstanceStatus::Running));
        assert_eq!(reg.get("x").unwrap().status, InstanceStatus::Running);
        assert!(!reg.update_status("y", InstanceStatus::Running));
    }
}
