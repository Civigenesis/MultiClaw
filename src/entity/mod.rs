//! Single-instance multi-entity runtime: EntityRuntime and EntityPool.
//!
//! Built from `Config::instance`. Used by the agent loop to resolve
//! provider/model and skills allowlist per target entity (including ceo).
//! Also provides entity workspace scaffolding (IDENTITY.md, SOUL.md, AGENTS.md)
//! so each entity has its own persona files for independent decision-making.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::config::{Config, EntityConfig, InstanceConfig};

/// Directory name under instance workspace for per-entity workspaces.
pub const ENTITIES_DIR: &str = "entities";
/// Directory name under instance workspace for per-team workspaces.
pub const TEAMS_DIR: &str = "teams";

/// Resolve the workspace root for an entity. Instance workspace layout:
/// `workspace/entities/<entity_id>/` (memory, state, sessions, skills, etc.).
#[must_use]
pub fn entity_workspace_dir(workspace_dir: &Path, entity_id: &str) -> PathBuf {
    workspace_dir.join(ENTITIES_DIR).join(entity_id)
}

/// Resolve the workspace root for a team. Instance workspace layout:
/// `workspace/teams/<team_id>/`.
#[must_use]
pub fn team_workspace_dir(workspace_dir: &Path, team_id: &str) -> PathBuf {
    workspace_dir.join(TEAMS_DIR).join(team_id)
}

/// Scaffold persona files (IDENTITY.md, SOUL.md, AGENTS.md) in an entity's workspace
/// with detailed content (50–200 words): identity, responsibilities, skills/tools, workflow.
/// Only creates files that do not already exist (idempotent). Does not create agent.md (AGENTS.md is canonical).
pub async fn scaffold_entity_workspace(
    workspace_dir: &Path,
    entity_id: &str,
    role: Option<&str>,
) -> Result<()> {
    let entity_dir = entity_workspace_dir(workspace_dir, entity_id);
    let display_name = role.unwrap_or(entity_id);
    let is_ceo = entity_id.eq_ignore_ascii_case(CEO_ENTITY_ID)
        || role.map(|r| r.to_lowercase().contains("ceo")).unwrap_or(false);

    let (identity, agents) = if is_ceo {
        (
            detailed_ceo_identity(display_name, entity_id),
            detailed_ceo_agents(display_name),
        )
    } else {
        (
            detailed_entity_identity(display_name, entity_id, role),
            detailed_entity_agents(display_name, entity_id, role),
        )
    };

    tokio::fs::create_dir_all(&entity_dir)
        .await
        .with_context(|| format!("create entity dir {}", entity_dir.display()))?;

    let soul = format!(
        "# SOUL.md — Who You Are\n\n\
         You are **{display_name}** (entity: {entity_id}). That is your name. That is who you are.\n\n\
         - Be genuinely helpful, not performatively helpful.\n\
         - Have opinions. Be resourceful before asking.\n\
         - Each session you wake up fresh. These files ARE your memory.\n\n\
         ---\n\n\
         *This file is yours to evolve. As you learn who you are, update it.*\n"
    );

    for (filename, content) in [
        ("IDENTITY.md", identity),
        ("SOUL.md", soul),
        ("AGENTS.md", agents),
    ] {
        let path = entity_dir.join(filename);
        if !path.exists() {
            tokio::fs::write(&path, content).await?;
        }
    }

    Ok(())
}

fn detailed_ceo_identity(display_name: &str, entity_id: &str) -> String {
    format!(
        r#"# IDENTITY.md — CEO 身份与职责

## 身份
- **名称：** {display_name}
- **实体 ID：** {entity_id}
- **定位：** 本实例的决策与协调中枢，对团队组建、任务分配与执行结果负责。

## 职责
- **团队组建：** 使用 create_team 创建团队并落盘到配置；使用 create_entity 创建成员并为其生成独立 workspace 与身份描述（IDENTITY.md、AGENTS.md 需 50–200 字，明确身份、职责、技能与工作流程）。
- **任务分配：** 通过 assign_task 将任务下达给指定实体（阶段 3 MessageBus 实现投递）。
- **状态把控：** 使用 instance_status 查看当前实例下所有实体，据此规划分工与跟进。

## 技能与工具
- **create_team**：创建团队，写入 config [instance.teams]，并创建 workspace/teams/<team_id>/。
- **create_entity**：创建实体，写入 config [[instance.entities]]，创建 workspace/entities/<id>/ 并生成详细 IDENTITY.md、AGENTS.md；创建后你应补充或确认该实体的职责与工作流程描述。
- **assign_task**：向指定实体分配任务（当前为占位，阶段 3 实现）。
- **instance_status**：列出本实例全部实体。

## 工作流程
1. 了解需求后，先 instance_status 查看现有人力。
2. 缺团队则 create_team，缺成员则 create_entity（并为新成员撰写或核验身份与职责描述）。
3. 使用 assign_task 分配具体任务，并在后续会话中跟进结果、更新记忆与文件。

---
*根据实际业务调整本文件；新成员创建时务必为其写好身份与职责。*
"#,
        display_name = display_name,
        entity_id = entity_id
    )
}

fn detailed_ceo_agents(display_name: &str) -> String {
    format!(
        r#"# AGENTS.md — {display_name} 工作规范

## 每会话必做
1. 阅读 SOUL.md、IDENTITY.md，确认自己是本实例的 CEO。
2. 使用 instance_status 查看当前实体列表与状态。
3. 使用 memory_recall 回顾近期决策与任务进展。

## 创建团队与成员时
- **create_team**：给出团队 id 与可选 name，创建后可在 workspace/teams/<id>/ 下放共享说明。
- **create_entity**：必须明确该实体的**职责与工作流程**。创建完成后，用 file_write 为该实体撰写或完善 workspace/entities/<id>/IDENTITY.md 与 AGENTS.md（建议 50–200 字），包含：身份、职责、可用的技能/工具、日常工作流程。不要只留一句描述。

## 任务分配与跟进
- 用 assign_task 指定 entity_id 与 task 描述；当前为占位，后续通过 MessageBus 投递。
- 在对话中明确告知成员其职责与产出期望，并写入该成员的 AGENTS.md 或 MEMORY。

## 决策原则
- 先看 instance_status 再决定是否增人、建队。
- 每新增实体，必配清晰的身份与职责文档，便于其独立决策与协作。

---
*可根据实例类型（startup/enterprise 等）在此补充更多规范。*
"#,
        display_name = display_name
    )
}

fn detailed_entity_identity(display_name: &str, entity_id: &str, role: Option<&str>) -> String {
    let role_desc = role.unwrap_or("成员");
    format!(
        r#"# IDENTITY.md — 身份与职责

## 身份
- **名称：** {display_name}
- **实体 ID：** {entity_id}
- **角色：** {role_desc}
- **定位：** 本实例中的执行单元，在 CEO 分配的任务范围内独立决策与产出。

## 职责
- **任务执行：** 接收并完成 CEO 或系统分配的任务；必要时使用 memory_store / memory_recall 与文件读写。
- **产出交付：** 按任务要求输出文档、分析、代码等，并写入约定路径或汇报给 CEO。
- **边界：** 在未授权情况下不代替其他实体做决策；跨实体协作时通过明确任务描述与交付物沟通。

## 技能与工具
- **file_read / file_write**：读写工作区内文件，产出与记录结论。
- **memory_store / memory_recall / memory_forget**：持久化关键信息与任务上下文。
- **shell**：在安全策略允许下执行命令（构建、测试、脚本等）。
- 其他已开放工具按需使用；若有技能白名单，仅使用允许的技能。

## 工作流程
1. 每会话先读 SOUL.md、IDENTITY.md，用 memory_recall 拉取近期任务与上下文。
2. 若有待办任务，按优先级执行并记录进展；产出写入指定路径或通过记忆汇报。
3. 任务完成后更新记忆或 AGENTS.md 中的 Open Loops，便于 CEO 或下一轮会话跟进。

---
*根据实际角色（如市场分析、产品分析、报告编写）在此细化职责与流程。*
"#,
        display_name = display_name,
        entity_id = entity_id,
        role_desc = role_desc
    )
}

fn detailed_entity_agents(display_name: &str, _entity_id: &str, role: Option<&str>) -> String {
    let role_desc = role.unwrap_or("成员");
    format!(
        r#"# AGENTS.md — {display_name} 工作规范

## 每会话必做
1. 阅读 SOUL.md、IDENTITY.md，确认自己的身份与职责（{role_desc}）。
2. 使用 memory_recall 检索与本实体相关的任务与上下文。
3. 若有未完成任务，优先推进并记录结果。

## 任务执行规范
- **接收任务：** 从对话、记忆或后续 MessageBus 获取任务描述；不清楚时先澄清再执行。
- **执行与产出：** 按任务要求使用 file_write、shell、记忆等工具产出结果；输出路径与格式按约定（如无约定则写入本实体 workspace 或汇报摘要）。
- **汇报：** 重要结论与交付物路径写入 memory 或更新 AGENTS.md，便于 CEO 与后续会话使用。

## 与 CEO 的协作
- 不擅自创建其他实体或团队；需要增人时向 CEO 说明理由。
- 任务边界模糊时，在 AGENTS.md 或记忆中记录假设与待确认项，避免越权或重复劳动。

## 可扩展规则
- 在本文件末尾按需添加：常用命令、本机路径约定、与其它实体的接口约定等。

---
*根据实际角色在此补充更细的流程（如报告编写格式、市场分析产出模板等）。*
"#,
        display_name = display_name,
        role_desc = role_desc
    )
}

/// Scaffold instance-level workspace with IDENTITY.md, SOUL.md, AGENTS.md at workspace root.
/// Instance layer = company/team identity and goals (default voice for direct conversation).
/// For management and entity coordination, the CEO entity (workspace/entities/ceo/) is used.
/// Only creates files that do not already exist (idempotent).
pub async fn scaffold_instance_workspace(
    workspace_dir: &Path,
    instance_id: &str,
) -> Result<()> {
    let identity = format!(
        "# IDENTITY.md — 实例身份与目标\n\n\
         - **实例 ID：** {instance_id}\n\
         - **定位：** 本实例的默认身份，代表本实例（公司/团队）与用户直接对话。\n\
         - **与 CEO 的关系：** 管理与协调（建队、分配任务、跟进）由 CEO 实体负责；对话时指定 `--entity ceo` 即使用 CEO。\n\n\
         ---\n\n\
         在此填写本实例的名称、目标、业务范围等，作为默认对话的身份。\n"
    );

    let soul = format!(
        "# SOUL.md — Who You Are\n\n\
         You are the default voice of **{instance_id}** (this instance).\n\n\
         - Be genuinely helpful. Have opinions. Be resourceful before asking.\n\
         - Each session you wake up fresh. These files ARE your memory.\n\
         - For team/entity management, the user can switch to the CEO entity.\n\n\
         ---\n\n\
         *This file is yours to evolve.*\n"
    );

    let agents = format!(
        "# AGENTS.md — {instance_id} 实例规范\n\n\
         ## 每会话\n\n\
         1. 阅读 SOUL.md、IDENTITY.md，确认本实例的目标与身份。\n\
         2. 使用 memory_recall 获取近期上下文。\n\n\
         ## 与 CEO 的分工\n\n\
         - 实例层（本目录）：默认对话身份，描述公司/团队目标与能力。\n\
         - CEO 实体（workspace/entities/ceo/）：负责 create_team、create_entity、assign_task、instance_status 等管理操作。\n\n\
         ---\n\n\
         在此补充本实例的协作规范与约定。\n"
    );

    for (filename, content) in [
        ("IDENTITY.md", identity),
        ("SOUL.md", soul),
        ("AGENTS.md", agents),
    ] {
        let path = workspace_dir.join(filename);
        if !path.exists() {
            tokio::fs::write(&path, content).await?;
        }
    }

    Ok(())
}

/// Fixed entity id for the CEO entity.
pub const CEO_ENTITY_ID: &str = "ceo";

/// Runtime view of a single entity: id, provider/model overrides, skills allowlist.
#[derive(Debug, Clone)]
pub struct EntityRuntime {
    pub id: String,
    pub provider_override: Option<String>,
    pub model_override: Option<String>,
    pub skills_allowlist: Vec<String>,
}

/// Pool of entities (CEO + configured entities) for one instance.
/// Uses interior mutability so CEO tool `create_entity` can add entities at runtime.
#[derive(Debug, Default)]
pub struct EntityPool {
    entities: RwLock<Vec<EntityRuntime>>,
}

impl EntityPool {
    /// Build pool from config. Returns `None` if `config.instance` is absent (single-entity mode).
    pub fn from_config(config: &Config) -> Option<Arc<EntityPool>> {
        let instance = config.instance.as_ref()?;
        let mut entities = Vec::new();

        // CEO if enabled (default enabled when [instance.ceo] present and enabled != false)
        if Self::ceo_enabled(instance) {
            entities.push(EntityRuntime {
                id: CEO_ENTITY_ID.to_string(),
                provider_override: instance.default_provider.clone(),
                model_override: instance.default_model.clone(),
                skills_allowlist: vec![], // CEO gets full tool set; filtering is by current_entity_id
            });
        }

        for e in &instance.entities {
            entities.push(EntityRuntime {
                id: e.id.clone(),
                provider_override: e.provider.clone().or_else(|| instance.default_provider.clone()),
                model_override: e.model.clone().or_else(|| instance.default_model.clone()),
                skills_allowlist: e.skills.clone().unwrap_or_default(),
            });
        }

        if entities.is_empty() {
            return None;
        }
        Some(Arc::new(EntityPool {
            entities: RwLock::new(entities),
        }))
    }

    /// Used by CEO create_entity tool to count entities against agent_max.
    pub fn ceo_enabled(instance: &InstanceConfig) -> bool {
        match &instance.ceo {
            None => true,
            Some(c) => c.enabled.unwrap_or(true),
        }
    }

    /// Get entity by id (cloned).
    pub fn get(&self, id: &str) -> Option<EntityRuntime> {
        self.entities
            .read()
            .ok()?
            .iter()
            .find(|e| e.id == id)
            .cloned()
    }

    /// Get the CEO entity if present.
    pub fn get_ceo(&self) -> Option<EntityRuntime> {
        self.get(CEO_ENTITY_ID)
    }

    /// List all entity ids.
    pub fn list(&self) -> Vec<String> {
        self.entities
            .read()
            .map(|g| g.iter().map(|e| e.id.clone()).collect())
            .unwrap_or_default()
    }

    /// Create a new entity at runtime. Fails if `current_len + 1 > agent_max`.
    pub fn create_entity(
        &self,
        config: EntityConfig,
        agent_max: Option<u32>,
    ) -> Result<EntityRuntime> {
        let limit = agent_max.unwrap_or(u32::MAX);
        let mut entities = self
            .entities
            .write()
            .map_err(|_| anyhow::anyhow!("entity pool lock poisoned"))?;
        if entities.len() as u32 >= limit {
            bail!(
                "entity limit reached (agent_max = {}); cannot create entity '{}'",
                limit,
                config.id
            );
        }
        if entities.iter().any(|e| e.id == config.id) {
            bail!("entity '{}' already exists", config.id);
        }
        let runtime = EntityRuntime {
            id: config.id.clone(),
            provider_override: config.provider,
            model_override: config.model,
            skills_allowlist: config.skills.unwrap_or_default(),
        };
        entities.push(runtime.clone());
        Ok(runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InstanceConfig;

    fn instance_config_with_ceo_and_entities() -> InstanceConfig {
        InstanceConfig {
            preset: Some("enterprise".to_string()),
            default_provider: Some("openai".to_string()),
            default_model: Some("gpt-4".to_string()),
            ceo: Some(crate::config::CeoConfig {
                enabled: Some(true),
            }),
            entities: vec![
                EntityConfig {
                    id: "writer".to_string(),
                    provider: Some("openai".to_string()),
                    model: Some("gpt-4".to_string()),
                    team_id: Some("content".to_string()),
                    role: Some("writer".to_string()),
                    skills: Some(vec!["file".to_string(), "web".to_string()]),
                },
                EntityConfig {
                    id: "reviewer".to_string(),
                    provider: None,
                    model: None,
                    team_id: Some("content".to_string()),
                    role: None,
                    skills: None,
                },
            ],
            teams: vec![],
            projects: vec![],
        }
    }

    #[test]
    fn entity_runtime_from_config_builds_ceo_and_entities() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).expect("pool should be built");
        let ceo = pool.get_ceo().expect("ceo should exist");
        assert_eq!(ceo.id, CEO_ENTITY_ID);
        assert_eq!(ceo.provider_override.as_deref(), Some("openai"));
        let writer = pool.get("writer").expect("writer should exist");
        assert_eq!(writer.skills_allowlist, &["file", "web"]);
        let reviewer = pool.get("reviewer").expect("reviewer should exist");
        assert_eq!(reviewer.provider_override.as_deref(), Some("openai"));
        let ids = pool.list();
        assert_eq!(ids, vec!["ceo", "writer", "reviewer"]);
    }

    #[test]
    fn entity_runtime_skills_allowlist_respected() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        let writer = pool.get("writer").unwrap();
        assert_eq!(writer.skills_allowlist, ["file", "web"]);
        let reviewer = pool.get("reviewer").unwrap();
        assert!(reviewer.skills_allowlist.is_empty());
    }

    #[test]
    fn entity_pool_get_returns_entity_by_id() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        assert!(pool.get("writer").is_some());
        assert!(pool.get("nonexistent").is_none());
    }

    #[test]
    fn entity_pool_get_ceo_returns_ceo_entity() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        let ceo = pool.get_ceo().unwrap();
        assert_eq!(ceo.id, CEO_ENTITY_ID);
    }

    #[test]
    fn entity_pool_create_entity_respects_agent_max() {
        let pool = Arc::new(EntityPool {
            entities: RwLock::new(vec![EntityRuntime {
                id: "a".to_string(),
                provider_override: None,
                model_override: None,
                skills_allowlist: vec![],
            }]),
        });
        let config_b = EntityConfig {
            id: "b".to_string(),
            provider: None,
            model: None,
            team_id: None,
            role: None,
            skills: None,
        };
        // agent_max = 2: can add one more
        let r = pool.create_entity(config_b.clone(), Some(2));
        assert!(r.is_ok());
        assert_eq!(pool.list().len(), 2);
        // agent_max = 2: cannot add another
        let config_c = EntityConfig {
            id: "c".to_string(),
            provider: None,
            model: None,
            team_id: None,
            role: None,
            skills: None,
        };
        let r2 = pool.create_entity(config_c, Some(2));
        assert!(r2.is_err());
        assert_eq!(pool.list().len(), 2);
    }

    #[test]
    fn entity_pool_from_config_none_when_no_instance() {
        let config = Config::default();
        assert!(EntityPool::from_config(&config).is_none());
    }

    #[test]
    fn entity_pool_ceo_disabled_when_ceo_enabled_false() {
        let mut instance = instance_config_with_ceo_and_entities();
        instance.ceo = Some(crate::config::CeoConfig {
            enabled: Some(false),
        });
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        assert!(pool.get_ceo().is_none());
        let ids = pool.list();
        assert_eq!(ids, vec!["writer", "reviewer"]);
    }

    #[tokio::test]
    async fn scaffold_entity_workspace_creates_identity_soul_agents() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workspace = tmp.path();
        let entity_dir = entity_workspace_dir(workspace, "analyst");
        std::fs::create_dir_all(&entity_dir).unwrap();

        scaffold_entity_workspace(workspace, "analyst", Some("市场分析")).await.unwrap();

        let identity = std::fs::read_to_string(entity_dir.join("IDENTITY.md")).unwrap();
        assert!(identity.contains("市场分析"));
        assert!(identity.contains("analyst"));

        let soul = std::fs::read_to_string(entity_dir.join("SOUL.md")).unwrap();
        assert!(soul.contains("市场分析"));
        assert!(soul.contains("analyst"));

        let agents = std::fs::read_to_string(entity_dir.join("AGENTS.md")).unwrap();
        assert!(agents.contains("市场分析"));

        // Idempotent: second call does not overwrite
        scaffold_entity_workspace(workspace, "analyst", Some("Other")).await.unwrap();
        let identity2 = std::fs::read_to_string(entity_dir.join("IDENTITY.md")).unwrap();
        assert!(identity2.contains("市场分析"), "existing file should not be overwritten");
    }
}
