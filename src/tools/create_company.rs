//! Admin tool: create_company (single-step apply) for cluster instance creation.

use super::traits::{Tool, ToolResult};
use crate::config::{Config, EntityConfig, InstanceConfig};
use crate::instance_registry::InstanceRegistry;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use multiclaw::instance_manager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Clone)]
pub struct CreateCompanyTool {
    /// Current runtime config path (used to enforce admin-only and locate cluster_root).
    config_path: PathBuf,
}

impl CreateCompanyTool {
    #[must_use]
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CreateCompanyAction {
    Apply,
}

#[derive(Debug, Deserialize)]
struct CreateCompanyArgs {
    action: CreateCompanyAction,

    /// Stable instance id under cluster_root/instances/<id>
    company_id: String,

    /// Optional preset name (startup/enterprise/brainstorm/freeform/project/...)
    #[serde(default)]
    preset: Option<String>,

    /// Optional business domain hint (e.g. "新媒体运营"). Used to auto-fill role packs when entities are omitted.
    #[serde(default)]
    business_domain: Option<String>,

    /// Optional max number of entities allowed in this instance.
    #[serde(default)]
    agent_max: Option<u32>,

    /// Instance-level persona markdown (written to workspace root on apply).
    #[serde(default)]
    instance_identity_md: Option<String>,
    #[serde(default)]
    instance_soul_md: Option<String>,
    #[serde(default)]
    instance_agents_md: Option<String>,

    /// CEO persona markdown (written to workspace/entities/ceo on apply).
    #[serde(default)]
    ceo_identity_md: Option<String>,
    #[serde(default)]
    ceo_soul_md: Option<String>,
    #[serde(default)]
    ceo_agents_md: Option<String>,

    /// Optional list of initial entities to persist and scaffold.
    #[serde(default)]
    entities: Vec<EntityDraft>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct EntityDraft {
    id: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    team_id: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    skills: Option<Vec<String>>,
    #[serde(default)]
    identity_md: Option<String>,
    #[serde(default)]
    soul_md: Option<String>,
    #[serde(default)]
    agents_md: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompanyDraftFile {
    version: u32,
    company_id: String,
    preset: Option<String>,
    business_domain: Option<String>,
    agent_max: Option<u32>,
    instance_identity_md: Option<String>,
    instance_soul_md: Option<String>,
    instance_agents_md: Option<String>,
    ceo_identity_md: Option<String>,
    ceo_soul_md: Option<String>,
    ceo_agents_md: Option<String>,
    entities: Vec<EntityDraft>,
    confirmed: bool,
    created_at: String,
    confirmed_at: Option<String>,
}

impl CompanyDraftFile {
    fn from_args(args: &CreateCompanyArgs) -> Self {
        let entities = if args.entities.is_empty() {
            default_entities_for(args.preset.as_deref(), args.business_domain.as_deref())
        } else {
            args.entities.clone()
        };
        Self {
            version: 1,
            company_id: args.company_id.clone(),
            preset: args.preset.clone(),
            business_domain: args.business_domain.clone(),
            agent_max: args.agent_max,
            instance_identity_md: args.instance_identity_md.clone(),
            instance_soul_md: args.instance_soul_md.clone(),
            instance_agents_md: args.instance_agents_md.clone(),
            ceo_identity_md: args.ceo_identity_md.clone(),
            ceo_soul_md: args.ceo_soul_md.clone(),
            ceo_agents_md: args.ceo_agents_md.clone(),
            entities,
            confirmed: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            confirmed_at: None,
        }
    }
}

fn has_inline_apply_payload(args: &CreateCompanyArgs) -> bool {
    args.preset.is_some()
        || args.business_domain.is_some()
        || args.agent_max.is_some()
        || args.instance_identity_md.is_some()
        || args.instance_soul_md.is_some()
        || args.instance_agents_md.is_some()
        || args.ceo_identity_md.is_some()
        || args.ceo_soul_md.is_some()
        || args.ceo_agents_md.is_some()
        || !args.entities.is_empty()
}

async fn resolve_default_provider_model(cluster_root: &Path) -> (Option<String>, Option<String>) {
    let mut provider = None;
    let mut model = None;

    // Prefer admin instance defaults.
    let admin_cfg = cluster_root
        .join("instances")
        .join(instance_manager::ADMIN_INSTANCE_ID)
        .join("config.toml");
    if let Ok(cfg) = Config::load_from_path(&admin_cfg).await {
        provider = cfg.default_provider.clone();
        model = cfg.default_model.clone();
    }

    // Fallback to root config defaults.
    if provider.is_none() || model.is_none() {
        let root_cfg = cluster_root.join("config.toml");
        if let Ok(cfg) = Config::load_from_path(&root_cfg).await {
            if provider.is_none() {
                provider = cfg.default_provider.clone();
            }
            if model.is_none() {
                model = cfg.default_model.clone();
            }
        }
    }

    // Final fallback to built-in defaults.
    let defaults = Config::default();
    if provider.is_none() {
        provider = defaults.default_provider;
    }
    if model.is_none() {
        model = defaults.default_model;
    }

    (provider, model)
}

fn is_ceo_entity_id(id: &str) -> bool {
    let lower = id.trim().to_ascii_lowercase();
    lower == crate::entity::CEO_ENTITY_ID || lower.starts_with("ceo_")
}

fn is_ceo_candidate(ed: &EntityDraft) -> bool {
    if is_ceo_entity_id(&ed.id) {
        return true;
    }
    ed.role
        .as_deref()
        .map(|r| r.to_ascii_lowercase().contains("ceo"))
        .unwrap_or(false)
}

fn role_based_default_skills(role: Option<&str>) -> Option<Vec<String>> {
    let role = role.unwrap_or("").to_ascii_lowercase();
    if role.contains("增长") || role.contains("growth") {
        return Some(vec![
            "memory_recall".into(),
            "memory_store".into(),
            "file_read".into(),
            "file_write".into(),
            "web_search_tool".into(),
            "content_search".into(),
        ]);
    }
    if role.contains("设计") || role.contains("design") {
        return Some(vec![
            "file_read".into(),
            "file_write".into(),
            "file_edit".into(),
            "image_info".into(),
            "memory_recall".into(),
            "memory_store".into(),
        ]);
    }
    if role.contains("内容") || role.contains("content") {
        return Some(vec![
            "file_read".into(),
            "file_write".into(),
            "file_edit".into(),
            "memory_recall".into(),
            "memory_store".into(),
            "web_search_tool".into(),
        ]);
    }
    if role.contains("运营") || role.contains("ops") {
        return Some(vec![
            "memory_recall".into(),
            "memory_store".into(),
            "file_read".into(),
            "file_write".into(),
            "schedule".into(),
        ]);
    }
    None
}

fn fallback_entity_identity(company_id: &str, ed: &EntityDraft) -> String {
    let role = ed.role.as_deref().unwrap_or("成员");
    let team = ed.team_id.as_deref().unwrap_or("未分配团队");
    format!(
        "# IDENTITY.md — {id}（{role}）\n\n## 身份\n- 所属实例：{company_id}\n- 实体 ID：{id}\n- 角色：{role}\n- 团队：{team}\n\n## 核心职责\n1. 围绕 {role} 目标执行可交付工作，不越权替代 CEO 决策。\n2. 对外输出需结构化、可复用，并标注产出路径。\n3. 与其他实体协作时明确输入、输出、截止时间。\n",
        company_id = company_id,
        id = ed.id,
        role = role,
        team = team
    )
}

fn fallback_entity_soul(company_id: &str, ed: &EntityDraft) -> String {
    let role = ed.role.as_deref().unwrap_or("成员");
    format!(
        "# SOUL.md — {id}\n\n你是实例 {company_id} 的 {role}。每次会话先确认任务边界，再产出可交付结果；当需求不清晰时先提问澄清，不靠猜测推进。\n",
        id = ed.id,
        company_id = company_id,
        role = role
    )
}

fn fallback_entity_agents(_company_id: &str, ed: &EntityDraft) -> String {
    let role = ed.role.as_deref().unwrap_or("成员");
    let skills = ed
        .skills
        .clone()
        .or_else(|| role_based_default_skills(ed.role.as_deref()))
        .unwrap_or_else(|| {
            vec![
                "file_read".into(),
                "file_write".into(),
                "memory_recall".into(),
            ]
        });
    let skills_text = skills.join(", ");
    format!(
        "# AGENTS.md — {id} 工作规范\n\n## 每会话必做\n1. 阅读 IDENTITY.md/SOUL.md，确认当前角色目标（{role}）。\n2. 使用 memory_recall 检索最近任务上下文。\n3. 产出完成后记录结果与路径。\n\n## 推荐工具\n- {skills_text}\n\n## 协作边界\n- 向 CEO 汇报进展与阻塞；不擅自修改其它实体职责。\n- 与跨团队协作时，先给出输入/输出契约再执行。\n\n## 产出要求\n- 输出应可审计：结论、依据、文件路径、下一步建议。\n- 关键决策写入 memory，便于后续接力。\n",
        id = ed.id,
        role = role,
        skills_text = skills_text,
    )
}

fn default_entities_for(preset: Option<&str>, business_domain: Option<&str>) -> Vec<EntityDraft> {
    let preset = preset.unwrap_or("").to_ascii_lowercase();
    let domain = business_domain.unwrap_or("").to_string();

    if preset == "startup" {
        // Minimal sensible pack. Domain-specific refinements below.
        let mut base = vec![
            EntityDraft {
                id: "ops".into(),
                role: Some("运营".into()),
                team_id: None,
                provider: None,
                model: None,
                skills: None,
                identity_md: None,
                soul_md: None,
                agents_md: None,
            },
            EntityDraft {
                id: "growth".into(),
                role: Some("增长".into()),
                team_id: None,
                provider: None,
                model: None,
                skills: None,
                identity_md: None,
                soul_md: None,
                agents_md: None,
            },
            EntityDraft {
                id: "design".into(),
                role: Some("设计".into()),
                team_id: None,
                provider: None,
                model: None,
                skills: None,
                identity_md: None,
                soul_md: None,
                agents_md: None,
            },
        ];

        if domain.contains("新媒体") || domain.contains("内容") {
            base.push(EntityDraft {
                id: "content".into(),
                role: Some("内容策划".into()),
                team_id: None,
                provider: None,
                model: None,
                skills: None,
                identity_md: None,
                soul_md: None,
                agents_md: None,
            });
        }
        return base;
    }

    Vec::new()
}

fn sanitize_company_id(raw: &str) -> Result<String> {
    let id = raw.trim();
    if id.is_empty() {
        bail!("company_id must be non-empty");
    }
    if id.eq_ignore_ascii_case(instance_manager::ADMIN_INSTANCE_ID) {
        bail!(
            "company_id '{}' is reserved",
            instance_manager::ADMIN_INSTANCE_ID
        );
    }
    if id.contains(std::path::MAIN_SEPARATOR) || id.contains('/') {
        bail!("company_id must not contain path separators");
    }
    Ok(id.to_string())
}

fn cluster_root_from_config_path(config_path: &Path) -> Result<PathBuf> {
    // .../<cluster_root>/instances/<id>/config.toml
    let root = config_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .context("Could not resolve cluster root from config path")?;
    Ok(root.to_path_buf())
}

fn admin_drafts_dir(cluster_root: &Path) -> PathBuf {
    cluster_root
        .join("instances")
        .join(instance_manager::ADMIN_INSTANCE_ID)
        .join("workspace")
        .join("state")
        .join("company_drafts")
}

fn draft_path_for(cluster_root: &Path, company_id: &str) -> PathBuf {
    admin_drafts_dir(cluster_root).join(format!("{company_id}.json"))
}

async fn save_draft(path: &Path, draft: &CompanyDraftFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let contents = serde_json::to_string_pretty(draft).context("serialize draft")?;
    fs::write(path, contents)
        .await
        .with_context(|| format!("Failed to write draft {}", path.display()))?;
    Ok(())
}

async fn write_text(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, content)
        .await
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

async fn ensure_entity_subdirs(entity_dir: &Path) -> Result<()> {
    fs::create_dir_all(entity_dir).await?;
    for sub in ["memory", "state", "sessions"] {
        fs::create_dir_all(entity_dir.join(sub)).await?;
    }
    Ok(())
}

async fn apply_to_instance(
    cluster_root: &Path,
    company_id: &str,
    draft: &CompanyDraftFile,
) -> Result<(u16, PathBuf)> {
    let preset = draft.preset.as_deref();
    let entry = instance_manager::instance_create(
        cluster_root,
        company_id,
        crate::instance_registry::InstanceRole::Normal,
        preset,
    )
    .await?;

    // Update constraints.agent_max in instances.json
    if draft.agent_max.is_some() {
        let mut reg = InstanceRegistry::load(cluster_root).await?;
        if let Some(e) = reg.get_mut(company_id) {
            let mut constraints = e.constraints.clone().unwrap_or_default();
            constraints.agent_max = draft.agent_max;
            e.constraints = Some(constraints);
        }
        reg.save(cluster_root).await?;
    }

    let config_path = entry
        .config_path
        .as_deref()
        .map(PathBuf::from)
        .context("created instance missing config_path")?;
    let workspace_dir = entry
        .workspace_path
        .as_deref()
        .map(PathBuf::from)
        .context("created instance missing workspace_path")?;

    // Ensure [instance] + ceo enabled + entities persisted (so CEO tools see them).
    let mut cfg = Config::load_from_path(&config_path).await?;
    if cfg.default_provider.is_none() || cfg.default_model.is_none() {
        let (provider, model) = resolve_default_provider_model(cluster_root).await;
        if cfg.default_provider.is_none() {
            cfg.default_provider = provider;
        }
        if cfg.default_model.is_none() {
            cfg.default_model = model;
        }
    }
    let inst: &mut InstanceConfig = cfg.instance.get_or_insert_with(Default::default);
    inst.preset = draft
        .preset
        .clone()
        .or_else(|| preset.map(ToString::to_string));
    if inst.ceo.is_none() {
        inst.ceo = Some(crate::config::CeoConfig {
            enabled: Some(true),
        });
    } else if let Some(ref mut ceo) = inst.ceo {
        if ceo.enabled.is_none() {
            ceo.enabled = Some(true);
        }
    }

    // Merge entities (skip duplicates and any CEO-like ids).
    for ed in &draft.entities {
        let id = ed.id.trim();
        if id.is_empty() || is_ceo_candidate(ed) {
            continue;
        }
        if inst.entities.iter().any(|e| e.id == id) {
            continue;
        }
        inst.entities.push(EntityConfig {
            id: id.to_string(),
            provider: ed.provider.clone(),
            model: ed.model.clone(),
            team_id: ed.team_id.clone(),
            role: ed.role.clone(),
            skills: ed
                .skills
                .clone()
                .or_else(|| role_based_default_skills(ed.role.as_deref())),
        });
    }
    cfg.save().await?;

    // Write instance-level persona (overwrite scaffold with the detailed draft).
    if let Some(ref md) = draft.instance_identity_md {
        write_text(&workspace_dir.join("IDENTITY.md"), md).await?;
    }
    if let Some(ref md) = draft.instance_soul_md {
        write_text(&workspace_dir.join("SOUL.md"), md).await?;
    }
    if let Some(ref md) = draft.instance_agents_md {
        write_text(&workspace_dir.join("AGENTS.md"), md).await?;
    }

    // Ensure CEO workspace and write CEO persona.
    let ceo_dir = crate::entity::entity_workspace_dir(&workspace_dir, crate::entity::CEO_ENTITY_ID);
    ensure_entity_subdirs(&ceo_dir).await?;
    crate::entity::scaffold_entity_workspace(
        &workspace_dir,
        crate::entity::CEO_ENTITY_ID,
        Some("CEO"),
    )
    .await
    .with_context(|| "scaffold ceo workspace during create_company apply")?;
    if let Some(ref md) = draft.ceo_identity_md {
        write_text(&ceo_dir.join("IDENTITY.md"), md).await?;
    }
    if let Some(ref md) = draft.ceo_soul_md {
        write_text(&ceo_dir.join("SOUL.md"), md).await?;
    }
    if let Some(ref md) = draft.ceo_agents_md {
        write_text(&ceo_dir.join("AGENTS.md"), md).await?;
    }

    // Scaffold entities workspaces and persona files.
    for ed in &draft.entities {
        let id = ed.id.trim();
        if id.is_empty() || is_ceo_candidate(ed) {
            continue;
        }
        let entity_dir = crate::entity::entity_workspace_dir(&workspace_dir, id);
        ensure_entity_subdirs(&entity_dir).await?;
        let identity = ed
            .identity_md
            .clone()
            .unwrap_or_else(|| fallback_entity_identity(company_id, ed));
        let soul = ed
            .soul_md
            .clone()
            .unwrap_or_else(|| fallback_entity_soul(company_id, ed));
        let agents = ed
            .agents_md
            .clone()
            .unwrap_or_else(|| fallback_entity_agents(company_id, ed));
        write_text(&entity_dir.join("IDENTITY.md"), &identity).await?;
        write_text(&entity_dir.join("SOUL.md"), &soul).await?;
        write_text(&entity_dir.join("AGENTS.md"), &agents).await?;
    }

    Ok((entry.gateway_port.unwrap_or(0), workspace_dir))
}

#[async_trait]
impl Tool for CreateCompanyTool {
    fn name(&self) -> &str {
        "create_company"
    }

    fn description(&self) -> &str {
        "Create a company (cluster instance) in a single apply call. Admin-only. Expects full payload after user confirmation and writes instance/CEO/entity persona files plus constraints like agent_max."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["apply"] },
                "company_id": { "type": "string", "description": "Instance id (directory name) under cluster_root/instances/<id>." },
                "preset": { "type": "string", "description": "Optional instance preset (startup/enterprise/brainstorm/freeform/project/...)." },
                "business_domain": { "type": "string", "description": "Optional business domain hint (e.g. 新媒体运营). When entities are omitted, preset role packs may be auto-filled." },
                "agent_max": { "type": "integer", "minimum": 1, "description": "Optional maximum number of entities allowed in this instance (constraints.agent_max)." },
                "instance_identity_md": { "type": "string", "description": "Optional instance-level IDENTITY.md content to write on apply." },
                "instance_soul_md": { "type": "string", "description": "Optional instance-level SOUL.md content to write on apply." },
                "instance_agents_md": { "type": "string", "description": "Optional instance-level AGENTS.md content to write on apply." },
                "ceo_identity_md": { "type": "string", "description": "Optional CEO entity IDENTITY.md content to write on apply." },
                "ceo_soul_md": { "type": "string", "description": "Optional CEO entity SOUL.md content to write on apply." },
                "ceo_agents_md": { "type": "string", "description": "Optional CEO entity AGENTS.md content to write on apply." },
                "entities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "role": { "type": "string" },
                            "team_id": { "type": "string" },
                            "provider": { "type": "string" },
                            "model": { "type": "string" },
                            "skills": { "type": "array", "items": { "type": "string" } },
                            "identity_md": { "type": "string" },
                            "soul_md": { "type": "string" },
                            "agents_md": { "type": "string" }
                        },
                        "required": ["id"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["action", "company_id"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !instance_manager::is_admin_instance(&self.config_path) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("create_company is admin-only (run with --instance admin)".into()),
            });
        }

        let args: CreateCompanyArgs = serde_json::from_value(args)
            .map_err(|e| anyhow::anyhow!("invalid create_company arguments: {e}"))?;
        let company_id = sanitize_company_id(&args.company_id)?;
        let cluster_root = cluster_root_from_config_path(&self.config_path)?;
        let admin_workspace = cluster_root
            .join("instances")
            .join(instance_manager::ADMIN_INSTANCE_ID)
            .join("workspace");
        // Ensure admin skill templates exist in-place before create workflow.
        let _ = crate::entity::scaffold_admin_workspace(&admin_workspace).await;
        let draft_path = draft_path_for(&cluster_root, &company_id);

        match args.action {
            CreateCompanyAction::Apply => {
                if !has_inline_apply_payload(&args) {
                    bail!(
                        "create_company now supports single-step apply only. \
Provide full company payload (instance/ceo/entities/resource fields) in action=apply."
                    );
                }
                let mut draft = CompanyDraftFile::from_args(&args);
                draft.confirmed = true;
                draft.confirmed_at = Some(chrono::Utc::now().to_rfc3339());
                // Keep an auditable snapshot for each apply.
                save_draft(&draft_path, &draft).await?;
                let (port, workspace_dir) =
                    apply_to_instance(&cluster_root, &company_id, &draft).await?;
                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Company '{}' created (port {}). Workspace: {}. Draft: {}",
                        company_id,
                        port,
                        workspace_dir.display(),
                        draft_path.display()
                    ),
                    error: None,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiclaw::instance_manager::ensure_admin_instance;
    use tempfile::TempDir;

    #[tokio::test]
    async fn create_company_apply_single_step_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let (_ws, admin_cfg) = ensure_admin_instance(root).await.unwrap();
        let tool = CreateCompanyTool::new(admin_cfg.clone());

        let apply_args = json!({
            "action": "apply",
            "company_id": "newmedia",
            "preset": "startup",
            "agent_max": 10,
            "instance_identity_md": "# IDENTITY\nnewmedia",
            "ceo_identity_md": "# CEO\n",
            "entities": [{"id":"ops","role":"运营"}]
        });
        let r = tool.execute(apply_args).await.unwrap();
        assert!(r.success, "{:?}", r.error);

        // Verify instance directory exists
        assert!(root
            .join("instances")
            .join("newmedia")
            .join("config.toml")
            .exists());
        assert!(root
            .join("instances")
            .join("newmedia")
            .join("workspace")
            .join("IDENTITY.md")
            .exists());
        // Verify constraints stored
        let reg = InstanceRegistry::load(root).await.unwrap();
        let e = reg.get("newmedia").unwrap();
        assert_eq!(e.constraints.as_ref().and_then(|c| c.agent_max), Some(10));
    }

    #[tokio::test]
    async fn create_company_apply_with_inline_payload_single_call() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let (_ws, admin_cfg) = ensure_admin_instance(root).await.unwrap();
        let tool = CreateCompanyTool::new(admin_cfg.clone());

        let r = tool
            .execute(json!({
                "action": "apply",
                "company_id": "assistant",
                "preset": "startup",
                "agent_max": 6,
                "instance_identity_md": "# IDENTITY\nassistant",
                "ceo_identity_md": "# CEO\nassistant-ceo",
                "entities": [
                    {"id":"ceo_01","role":"CEO"},
                    {"id":"ops","role":"运营"}
                ]
            }))
            .await
            .unwrap();
        assert!(r.success, "{:?}", r.error);

        let assistant = root.join("instances").join("assistant").join("workspace");
        assert!(assistant.join("entities").join("ceo").exists());
        assert!(assistant
            .join("entities")
            .join("ceo")
            .join("skills")
            .join("ceo_entity_designer")
            .join("SKILL.md")
            .exists());
        assert!(!assistant.join("entities").join("ceo_01").exists());
        assert!(assistant
            .join("entities")
            .join("ops")
            .join("IDENTITY.md")
            .exists());

        let cfg =
            std::fs::read_to_string(root.join("instances").join("assistant").join("config.toml"))
                .unwrap();
        assert!(cfg.contains("[[instance.entities]]"));
        assert!(cfg.contains("id = \"ops\""));
        assert!(!cfg.contains("id = \"ceo_01\""));
        assert!(cfg.contains("default_provider"));
        assert!(cfg.contains("default_model"));
    }

    #[tokio::test]
    async fn create_company_apply_requires_full_payload() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let (_ws, admin_cfg) = ensure_admin_instance(root).await.unwrap();
        let tool = CreateCompanyTool::new(admin_cfg);
        let res = tool
            .execute(json!({"action":"apply","company_id":"missing_payload_only"}))
            .await;
        assert!(res.is_err());
    }
}
