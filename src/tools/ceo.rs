//! CEO-only tools for multi-entity instances: create_team, create_entity, assign_task, instance_status,
//! ceo_skill_grant, clawhub_import_global, admin_skill_remove.
//! Exposed only when the current run target is the CEO entity (see phase 2d wiring).
//! create_team and create_entity persist to config.toml and create entity/team workspace dirs.

use super::entity_skills::normalize_entity_tool_allowlist_for_entity;
use super::skill_pack::{AdminSkillRemoveTool, ClawhubImportGlobalTool};
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
    tool_allowlist: Option<Vec<String>>,
    #[serde(default)]
    skill_allowlist: Option<Vec<String>>,
    #[serde(default)]
    identity_md: Option<String>,
    #[serde(default)]
    soul_md: Option<String>,
    #[serde(default)]
    agents_md: Option<String>,
}

const DEFAULT_TEAM_ID: &str = "unassigned";

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
                "tool_allowlist": { "type": "array", "items": { "type": "string" }, "description": "Optional per-entity executable tool names. Call tool_inventory first; elevated_dev_only tools (e.g. shell) require a dev/engineering-like role." },
                "skill_allowlist": { "type": "array", "items": { "type": "string" }, "description": "Optional skill package ids for load_skill." },
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
        let tools = match normalize_entity_tool_allowlist_for_entity(
            args.tool_allowlist,
            args.role.as_deref(),
        ) {
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
            tool_allowlist: Some(tools),
            skill_allowlist: args.skill_allowlist,
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

/// Grants a skill package id to an entity's `skill_allowlist` (CEO only). Persists to `config.toml` and updates the in-memory pool.
pub struct CeoSkillGrantTool {
    entity_pool: Option<Arc<EntityPool>>,
    config_path: PathBuf,
}

impl CeoSkillGrantTool {
    pub fn new(entity_pool: Option<Arc<EntityPool>>, config_path: PathBuf) -> Self {
        Self {
            entity_pool,
            config_path,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CeoSkillGrantArgs {
    entity_id: String,
    skill_id: String,
}

#[async_trait]
impl Tool for CeoSkillGrantTool {
    fn name(&self) -> &str {
        "ceo_skill_grant"
    }

    fn description(&self) -> &str {
        "Grant a skill package id (directory name) to an entity's skill_allowlist so load_skill can read it. CEO only; persists to [[instance.entities]]."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "string", "description": "Target entity id (not reserved id misuse)" },
                "skill_id": { "type": "string", "description": "Skill package id / slug (single path segment)" }
            },
            "required": ["entity_id", "skill_id"],
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
        let args: CeoSkillGrantArgs = serde_json::from_value(args)
            .map_err(|e| anyhow::anyhow!("invalid ceo_skill_grant arguments: {}", e))?;
        let entity_id = args.entity_id.trim();
        let skill_id = args.skill_id.trim();
        if entity_id.is_empty() || skill_id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("entity_id and skill_id are required".into()),
            });
        }
        if skill_id.contains("..") || skill_id.contains('/') || skill_id.contains('\\') {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("skill_id must be a single package id (no path separators)".into()),
            });
        }
        if pool.get(entity_id).is_none() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("unknown entity '{entity_id}'")),
            });
        }

        let mut config = Config::load_from_path(&self.config_path)
            .await
            .context("load config for ceo_skill_grant")?;
        let instance = config
            .instance
            .as_mut()
            .context("instance config missing")?;
        let entity_cfg = instance
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .context("entity not found in config")?;
        let mut list = entity_cfg.skill_allowlist.take().unwrap_or_default();
        if !list.iter().any(|s| s == skill_id) {
            list.push(skill_id.to_string());
        }
        entity_cfg.skill_allowlist = Some(list);
        config
            .save()
            .await
            .context("save config after ceo_skill_grant")?;

        match pool.merge_skill_allowlist(entity_id, &[skill_id.to_string()]) {
            Ok(()) => Ok(ToolResult {
                success: true,
                output: format!(
                    "Granted skill '{skill_id}' to entity '{entity_id}' (skill_allowlist updated)."
                ),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}

/// Build CEO tools. Only include when entity_pool is present (caller filters by current_entity_id == ceo in phase 2d).
/// config_path and workspace_dir are used to persist create_team/create_entity to config and create entity/team workspace dirs.
pub fn ceo_tools(
    entity_pool: Option<Arc<EntityPool>>,
    agent_max: Option<u32>,
    config_path: PathBuf,
    workspace_dir: PathBuf,
    config: Arc<Config>,
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
        Arc::new(CreateTeamTool::new(config_path.clone(), workspace_dir)),
        Arc::new(AssignTaskTool::new(entity_pool.clone())),
        Arc::new(CeoSkillGrantTool::new(entity_pool.clone(), config_path)),
        Arc::new(ClawhubImportGlobalTool::new(config.clone())),
        Arc::new(AdminSkillRemoveTool::new(config)),
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
                    tool_allowlist: None,
                    skill_allowlist: None,
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
