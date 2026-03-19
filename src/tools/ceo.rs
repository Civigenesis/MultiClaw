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
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

/// Lists entities and teams in this instance (CEO only). Uses config_path when set to include teams from config.
pub struct InstanceStatusTool {
    entity_pool: Option<Arc<EntityPool>>,
    config_path: Option<PathBuf>,
}

impl InstanceStatusTool {
    pub fn new(entity_pool: Option<Arc<EntityPool>>, config_path: Option<PathBuf>) -> Self {
        Self {
            entity_pool,
            config_path,
        }
    }
}

#[async_trait]
impl Tool for InstanceStatusTool {
    fn name(&self) -> &str {
        "instance_status"
    }

    fn description(&self) -> &str {
        "List all entities and teams in this instance (CEO only). Use to see current members and teams before create_team/create_entity or assign_task."
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
        let entity_ids = pool.list();
        let entities_line = format!(
            "entities: {}",
            if entity_ids.is_empty() {
                "none".to_string()
            } else {
                entity_ids.join(", ")
            }
        );
        let mut parts = vec![entities_line];
        if let Some(ref path) = self.config_path {
            if let Ok(config) = Config::load_from_path(path).await {
                if let Some(ref inst) = config.instance {
                    if !inst.teams.is_empty() {
                        let team_ids: Vec<&str> =
                            inst.teams.iter().map(|t| t.id.as_str()).collect();
                        parts.push(format!("teams: {}", team_ids.join(", ")));
                    }
                }
            }
        }
        Ok(ToolResult {
            success: true,
            output: parts.join("\n"),
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
    #[serde(default)]
    identity_md: Option<String>,
    #[serde(default)]
    soul_md: Option<String>,
    #[serde(default)]
    agents_md: Option<String>,
}

const DEFAULT_TEAM_ID: &str = "unassigned";

fn allowed_entity_skills() -> HashSet<&'static str> {
    [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "memory_store",
        "memory_recall",
        "memory_forget",
        "schedule",
        "web_search_tool",
        "web_fetch",
        "http_request",
        "image_info",
        "pdf_read",
    ]
    .into_iter()
    .collect()
}

fn role_based_default_skills(role: Option<&str>) -> Vec<String> {
    let role = role.unwrap_or("").to_ascii_lowercase();
    if role.contains("增长") || role.contains("growth") {
        return vec![
            "memory_recall".into(),
            "memory_store".into(),
            "file_read".into(),
            "file_write".into(),
            "web_search_tool".into(),
            "content_search".into(),
        ];
    }
    if role.contains("设计") || role.contains("design") {
        return vec![
            "file_read".into(),
            "file_write".into(),
            "file_edit".into(),
            "image_info".into(),
            "memory_recall".into(),
            "memory_store".into(),
        ];
    }
    if role.contains("内容") || role.contains("content") {
        return vec![
            "file_read".into(),
            "file_write".into(),
            "file_edit".into(),
            "memory_recall".into(),
            "memory_store".into(),
            "web_search_tool".into(),
        ];
    }
    if role.contains("运营") || role.contains("ops") {
        return vec![
            "memory_recall".into(),
            "memory_store".into(),
            "file_read".into(),
            "file_write".into(),
            "schedule".into(),
        ];
    }
    vec![
        "file_read".into(),
        "file_write".into(),
        "memory_recall".into(),
    ]
}

fn normalize_entity_skills(skills: Option<Vec<String>>, role: Option<&str>) -> Result<Vec<String>> {
    let allow = allowed_entity_skills();
    let result = skills.unwrap_or_else(|| role_based_default_skills(role));
    let mut normalized = Vec::new();
    for s in result {
        let skill = s.trim().to_string();
        if skill.is_empty() {
            continue;
        }
        if !allow.contains(skill.as_str()) {
            anyhow::bail!(
                "invalid entity skill '{}': skills must be tool allowlist names (e.g. file_read, memory_recall), not business capability labels",
                skill
            );
        }
        normalized.push(skill);
    }
    Ok(normalized)
}

#[async_trait]
impl Tool for CreateEntityTool {
    fn name(&self) -> &str {
        "create_entity"
    }

    fn description(&self) -> &str {
        "Create a new entity in this instance (CEO only). Persists to config [[instance.entities]], creates workspace/entities/<id>/, and optionally writes identity_md/soul_md/agents_md directly in one call. Subject to agent_max limit."
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
                "skills": { "type": "array", "items": { "type": "string" }, "description": "Optional skills allowlist" },
                "identity_md": { "type": "string", "description": "Optional explicit IDENTITY.md content for the new entity" },
                "soul_md": { "type": "string", "description": "Optional explicit SOUL.md content for the new entity" },
                "agents_md": { "type": "string", "description": "Optional explicit AGENTS.md content for the new entity" }
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
        let args: CreateEntityArgs = serde_json::from_value(args)
            .map_err(|e| anyhow::anyhow!("invalid create_entity arguments: {}", e))?;
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

        let team_id = args.team_id.filter(|s| !s.trim().is_empty());
        let team_id = Some(team_id.unwrap_or_else(|| DEFAULT_TEAM_ID.to_string()));
        let skills = match normalize_entity_skills(args.skills, args.role.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                });
            }
        };
        let entity_config = EntityConfig {
            id: id.to_string(),
            provider: args.provider,
            model: args.model,
            team_id,
            role: args.role,
            skills: Some(skills),
        };
        instance.entities.push(entity_config.clone());
        config
            .save()
            .await
            .context("save config after create_entity")?;

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
        if let Some(identity_md) = args.identity_md.as_deref() {
            fs::write(entity_dir.join("IDENTITY.md"), identity_md).await?;
        }
        if let Some(soul_md) = args.soul_md.as_deref() {
            fs::write(entity_dir.join("SOUL.md"), soul_md).await?;
        }
        if let Some(agents_md) = args.agents_md.as_deref() {
            fs::write(entity_dir.join("AGENTS.md"), agents_md).await?;
        }

        match pool.create_entity(entity_config, self.agent_max) {
            Ok(runtime) => Ok(ToolResult {
                success: true,
                output: format!(
                    "entity '{}' created, persisted to config, workspace at {}.",
                    runtime.id,
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
        config
            .save()
            .await
            .context("save config after create_team")?;

        let team_dir = team_workspace_dir(&self.workspace_dir, id);
        fs::create_dir_all(&team_dir)
            .await
            .with_context(|| format!("create team workspace {}", team_dir.display()))?;

        Ok(ToolResult {
            success: true,
            output: format!(
                "team '{}' created and persisted to config; workspace at {}",
                id,
                team_dir.display()
            ),
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
            output: "assign_task: task delivery will be implemented with MessageBus in phase 3."
                .to_string(),
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
    let config_path_opt = Some(config_path.clone());
    vec![
        Arc::new(InstanceStatusTool::new(
            entity_pool.clone(),
            config_path_opt,
        )),
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
        let tool = InstanceStatusTool::new(None, None);
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
                entities: vec![EntityConfig {
                    id: "writer".to_string(),
                    provider: None,
                    model: None,
                    team_id: None,
                    role: None,
                    skills: None,
                }],
                teams: vec![],
                projects: vec![],
            }),
            ..Config::default()
        })
        .unwrap();
        let tool = InstanceStatusTool::new(Some(pool), None);
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
        })
        .unwrap();
        let tool = CreateEntityTool::new(
            Some(pool),
            Some(10),
            PathBuf::from("/nonexistent/config.toml"),
            PathBuf::from("/nonexistent/workspace"),
        );
        let r = tool.execute(json!({ "id": "ceo" })).await.unwrap();
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
        })
        .unwrap();
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

    #[tokio::test]
    async fn create_entity_writes_inline_persona_files() {
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
        })
        .unwrap();
        let tool = CreateEntityTool::new(Some(pool), Some(10), config_path, workspace_dir.clone());
        let r = tool
            .execute(json!({
                "id": "product_manager",
                "role": "商业产品经理",
                "identity_md": "# IDENTITY.md\n自定义身份",
                "soul_md": "# SOUL.md\n自定义灵魂",
                "agents_md": "# AGENTS.md\n自定义规范"
            }))
            .await
            .unwrap();
        assert!(r.success, "{:?}", r.error);
        let entity_dir = workspace_dir.join("entities").join("product_manager");
        let identity = std::fs::read_to_string(entity_dir.join("IDENTITY.md")).unwrap();
        let soul = std::fs::read_to_string(entity_dir.join("SOUL.md")).unwrap();
        let agents = std::fs::read_to_string(entity_dir.join("AGENTS.md")).unwrap();
        assert!(identity.contains("自定义身份"));
        assert!(soul.contains("自定义灵魂"));
        assert!(agents.contains("自定义规范"));
    }
}
