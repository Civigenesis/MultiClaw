//! CEO-only tools for multi-entity instances: create_team, create_entity, assign_task, instance_status.
//! Exposed only when the current run target is the CEO entity (see phase 2d wiring).
//! create_team and create_entity persist to config.toml and create entity/team workspace dirs.

use super::traits::{Tool, ToolResult};
use crate::config::{Config, EntityConfig, TeamConfig};
use crate::entity::{entity_workspace_dir, team_workspace_dir, EntityPool, CEO_ENTITY_ID};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

/// Lists entities and instance status (CEO only).
pub struct InstanceStatusTool {
    entity_pool: Option<Arc<EntityPool>>,
}

impl InstanceStatusTool {
    pub fn new(entity_pool: Option<Arc<EntityPool>>) -> Self {
        Self { entity_pool }
    }
}

#[async_trait]
impl Tool for InstanceStatusTool {
    fn name(&self) -> &str {
        "instance_status"
    }

    fn description(&self) -> &str {
        "List all entities in this instance and their status (CEO only)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult> {
        let pool: &Arc<EntityPool> = match &self.entity_pool {
            Some(p) => p,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("multi-entity mode not enabled (no instance config)".to_string()),
                });
            }
        };
        let ids = pool.list();
        let summary = format!(
            "entities: {}",
            if ids.is_empty() {
                "none".to_string()
            } else {
                ids.join(", ")
            }
        );
        Ok(ToolResult {
            success: true,
            output: summary,
            error: None,
        })
    }
}

/// Creates a new entity (CEO only). Persists to config [[instance.entities]], creates workspace/entities/<id>/, and adds to pool for current run.
pub struct CreateEntityTool {
    entity_pool: Option<Arc<EntityPool>>,
    agent_max: Option<u32>,
    config_path: PathBuf,
    workspace_dir: PathBuf,
}

impl CreateEntityTool {
    pub fn new(
        entity_pool: Option<Arc<EntityPool>>,
        agent_max: Option<u32>,
        config_path: PathBuf,
        workspace_dir: PathBuf,
    ) -> Self {
        Self {
            entity_pool,
            agent_max,
            config_path,
            workspace_dir,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateEntityArgs {
    id: String,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    team_id: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    skills: Option<Vec<String>>,
}

#[async_trait]
impl Tool for CreateEntityTool {
    fn name(&self) -> &str {
        "create_entity"
    }

    fn description(&self) -> &str {
        "Create a new entity in this instance (CEO only). Persists to config [[instance.entities]], creates workspace/entities/<id>/ with IDENTITY.md and AGENTS.md. You must define the member's responsibilities and workflow (50–200 words) and write or update that entity's IDENTITY.md and AGENTS.md after creation. Subject to agent_max limit."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Unique entity id" },
                "provider": { "type": "string", "description": "Optional provider override" },
                "model": { "type": "string", "description": "Optional model override" },
                "team_id": { "type": "string", "description": "Optional team id" },
                "role": { "type": "string", "description": "Optional role" },
                "skills": { "type": "array", "items": { "type": "string" }, "description": "Optional skills allowlist" }
            },
            "required": ["id"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let pool: &Arc<EntityPool> = match &self.entity_pool {
            Some(p) => p,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("multi-entity mode not enabled".to_string()),
                });
            }
        };
        let args: CreateEntityArgs = serde_json::from_value(args).map_err(|e| {
            anyhow::anyhow!("invalid create_entity arguments: {}", e)
        })?;
        let id = args.id.trim();
        if id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("entity id cannot be empty".to_string()),
            });
        }
        if id == CEO_ENTITY_ID {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("entity id '{}' is reserved", CEO_ENTITY_ID)),
            });
        }

        let mut config = Config::load_from_path(&self.config_path)
            .await
            .context("load config for create_entity")?;
        let instance = config.instance.get_or_insert_with(Default::default);
        let limit = self.agent_max.unwrap_or(u32::MAX);
        let current_count = if crate::entity::EntityPool::ceo_enabled(instance) {
            instance.entities.len() as u32 + 1
        } else {
            instance.entities.len() as u32
        };
        if current_count >= limit {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "entity limit reached (agent_max = {}); cannot create entity '{}'",
                    limit, id
                )),
            });
        }
        if instance.entities.iter().any(|e| e.id == id) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("entity '{}' already exists", id)),
            });
        }

        let entity_config = EntityConfig {
            id: id.to_string(),
            provider: args.provider,
            model: args.model,
            team_id: args.team_id,
            role: args.role,
            skills: args.skills,
        };
        instance.entities.push(entity_config.clone());
        config.save().await.context("save config after create_entity")?;

        let entity_dir = entity_workspace_dir(&self.workspace_dir, id);
        fs::create_dir_all(&entity_dir)
            .await
            .with_context(|| format!("create entity workspace {}", entity_dir.display()))?;
        for sub in ["memory", "state", "sessions"] {
            let p = entity_dir.join(sub);
            if !p.exists() {
                let _ = fs::create_dir(&p).await;
            }
        }
        crate::entity::scaffold_entity_workspace(
            &self.workspace_dir,
            id,
            entity_config.role.as_deref(),
        )
        .await
        .with_context(|| format!("scaffold entity workspace for {}", id))?;

        match pool.create_entity(entity_config, self.agent_max) {
            Ok(runtime) => Ok(ToolResult {
                success: true,
                output: format!(
                    "entity '{}' created, persisted to config, workspace at {}. You must now write or update this entity's IDENTITY.md and AGENTS.md with detailed identity, responsibilities, skills/tools, and workflow (50–200 words each); use file_write to the paths under {}.",
                    runtime.id,
                    entity_dir.display(),
                    entity_dir.display()
                ),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some::<String>(e.to_string()),
            }),
        }
    }
}

/// Creates a team (CEO only). Persists to config [instance.teams] and creates workspace/teams/<id>/.
pub struct CreateTeamTool {
    config_path: PathBuf,
    workspace_dir: PathBuf,
}

impl CreateTeamTool {
    pub fn new(config_path: PathBuf, workspace_dir: PathBuf) -> Self {
        Self {
            config_path,
            workspace_dir,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateTeamArgs {
    id: String,
    #[serde(default)]
    name: Option<String>,
}

#[async_trait]
impl Tool for CreateTeamTool {
    fn name(&self) -> &str {
        "create_team"
    }

    fn description(&self) -> &str {
        "Create a team in this instance (CEO only). Persists to config [instance.teams] and creates workspace/teams/<id>/."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Team id" },
                "name": { "type": "string", "description": "Optional display name" }
            },
            "required": ["id"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let args: CreateTeamArgs = serde_json::from_value(args)
            .map_err(|e| anyhow::anyhow!("invalid create_team arguments: {}", e))?;
        let id = args.id.trim();
        if id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("team id cannot be empty".to_string()),
            });
        }
        if id == CEO_ENTITY_ID {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("team id '{}' is reserved", CEO_ENTITY_ID)),
            });
        }

        let mut config = Config::load_from_path(&self.config_path)
            .await
            .context("load config for create_team")?;
        let instance = config.instance.get_or_insert_with(Default::default);
        if instance.teams.iter().any(|t| t.id == id) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("team '{}' already exists", id)),
            });
        }
        instance.teams.push(TeamConfig {
            id: id.to_string(),
            name: args.name.filter(|s| !s.trim().is_empty()),
        });
        config.save().await.context("save config after create_team")?;

        let team_dir = team_workspace_dir(&self.workspace_dir, id);
        fs::create_dir_all(&team_dir)
            .await
            .with_context(|| format!("create team workspace {}", team_dir.display()))?;

        Ok(ToolResult {
            success: true,
            output: format!("team '{}' created and persisted to config; workspace at {}", id, team_dir.display()),
            error: None,
        })
    }
}

/// Assigns a task to an entity (CEO only). Phase 2: stub for phase 3 MessageBus.
pub struct AssignTaskTool {
    _entity_pool: Option<Arc<EntityPool>>,
}

impl AssignTaskTool {
    pub fn new(entity_pool: Option<Arc<EntityPool>>) -> Self {
        Self {
            _entity_pool: entity_pool,
        }
    }
}

#[async_trait]
impl Tool for AssignTaskTool {
    fn name(&self) -> &str {
        "assign_task"
    }

    fn description(&self) -> &str {
        "Assign a task to an entity (CEO only). Phase 2: task delivery will be implemented with MessageBus in phase 3."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "string", "description": "Target entity id" },
                "task": { "type": "string", "description": "Task description or message" }
            },
            "required": ["entity_id", "task"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "assign_task: task delivery will be implemented with MessageBus in phase 3.".to_string(),
            error: None,
        })
    }
}

/// Build CEO tools. Only include when entity_pool is present (caller filters by current_entity_id == ceo in phase 2d).
/// config_path and workspace_dir are used to persist create_team/create_entity to config and create entity/team workspace dirs.
pub fn ceo_tools(
    entity_pool: Option<Arc<EntityPool>>,
    agent_max: Option<u32>,
    config_path: PathBuf,
    workspace_dir: PathBuf,
) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(InstanceStatusTool::new(entity_pool.clone())),
        Arc::new(CreateEntityTool::new(
            entity_pool.clone(),
            agent_max,
            config_path.clone(),
            workspace_dir.clone(),
        )),
        Arc::new(CreateTeamTool::new(config_path, workspace_dir)),
        Arc::new(AssignTaskTool::new(entity_pool)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn instance_status_returns_error_when_no_pool() {
        let tool = InstanceStatusTool::new(None);
        let r = tool.execute(json!({})).await.unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap_or_default().contains("multi-entity"));
    }

    #[tokio::test]
    async fn instance_status_lists_entities() {
        let pool = EntityPool::from_config(&Config {
            instance: Some(crate::config::InstanceConfig {
                preset: Some("startup".to_string()),
                default_provider: None,
                default_model: None,
                ceo: Some(crate::config::CeoConfig {
                    enabled: Some(true),
                }),
                entities: vec![
                    EntityConfig {
                        id: "writer".to_string(),
                        provider: None,
                        model: None,
                        team_id: None,
                        role: None,
                        skills: None,
                    },
                ],
                teams: vec![],
                projects: vec![],
            }),
            ..Config::default()
        }).unwrap();
        let tool = InstanceStatusTool::new(Some(pool));
        let r = tool.execute(json!({})).await.unwrap();
        assert!(r.success);
        assert!(r.output.contains("ceo"));
        assert!(r.output.contains("writer"));
    }

    #[tokio::test]
    async fn create_entity_rejects_reserved_ceo_id() {
        let pool = EntityPool::from_config(&Config {
            instance: Some(crate::config::InstanceConfig {
                preset: Some("startup".to_string()),
                default_provider: None,
                default_model: None,
                ceo: Some(crate::config::CeoConfig {
                    enabled: Some(true),
                }),
                entities: vec![],
                teams: vec![],
                projects: vec![],
            }),
            ..Config::default()
        }).unwrap();
        let tool = CreateEntityTool::new(
            Some(pool),
            Some(10),
            PathBuf::from("/nonexistent/config.toml"),
            PathBuf::from("/nonexistent/workspace"),
        );
        let r = tool
            .execute(json!({ "id": "ceo" }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap_or_default().contains("reserved"));
    }

    #[tokio::test]
    async fn create_entity_respects_agent_max() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        let workspace_dir = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();
        let config_toml = r#"
default_provider = "openrouter"
default_model = "gpt-4"
default_temperature = 0.7
[instance]
preset = "startup"
[instance.ceo]
enabled = true
"#;
        std::fs::write(&config_path, config_toml).unwrap();

        let pool = EntityPool::from_config(&Config {
            instance: Some(crate::config::InstanceConfig {
                preset: Some("startup".to_string()),
                default_provider: None,
                default_model: None,
                ceo: Some(crate::config::CeoConfig {
                    enabled: Some(true),
                }),
                entities: vec![],
                teams: vec![],
                projects: vec![],
            }),
            ..Config::default()
        }).unwrap();
        let tool = CreateEntityTool::new(
            Some(pool.clone()),
            Some(2),
            config_path.clone(),
            workspace_dir.clone(),
        );
        let r1 = tool.execute(json!({ "id": "a" })).await.unwrap();
        assert!(r1.success, "first create should succeed: {:?}", r1.error);
        let r2 = tool.execute(json!({ "id": "b" })).await.unwrap();
        assert!(!r2.success, "second create should hit agent_max limit");
        assert!(r2.error.unwrap_or_default().contains("limit"));
    }
}
