//! Skill pack tools: local index, `load_skill`, ClawHub registry, import to global store.

use super::traits::{Tool, ToolResult};
use crate::clawhub::{ClawHubRegistry, DEFAULT_REGISTRY_URL};
use crate::entity::{EntityPool, CEO_ENTITY_ID};
use crate::skills::resolve_shared_skills_dir;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn registry_client(config: &crate::config::Config) -> Result<ClawHubRegistry> {
    let c = &config.skills.clawhub;
    let base = c
        .registry_url
        .clone()
        .unwrap_or_else(|| DEFAULT_REGISTRY_URL.to_string());
    ClawHubRegistry::new(base, c.timeout_ms, c.token.clone())
}

fn load_skill_allowed(entity_id: &str, slug: &str, pool: Option<&Arc<EntityPool>>) -> bool {
    let Some(pool) = pool else {
        return false;
    };
    let Some(e) = pool.get(entity_id) else {
        return false;
    };
    if entity_id == CEO_ENTITY_ID && e.skill_allowlist.is_empty() {
        return true;
    }
    e.skill_allowlist.iter().any(|s| s == slug)
}

fn resolve_skill_root(workspace_dir: &Path, shared_root: &Path, name: &str) -> Option<PathBuf> {
    let n = name.trim();
    if n.is_empty() || n.contains("..") || n.contains('/') {
        return None;
    }
    let w = workspace_dir.join("skills").join(n);
    if w.join("SKILL.md").exists() || w.join("SKILL.toml").exists() {
        return Some(w);
    }
    let s = shared_root.join(n);
    if s.join("SKILL.md").exists() || s.join("SKILL.toml").exists() {
        return Some(s);
    }
    None
}

fn read_skill_text(root: &Path) -> Result<String> {
    let md = root.join("SKILL.md");
    let toml = root.join("SKILL.toml");
    if md.exists() {
        return std::fs::read_to_string(&md).with_context(|| format!("read {}", md.display()));
    }
    if toml.exists() {
        return std::fs::read_to_string(&toml).with_context(|| format!("read {}", toml.display()));
    }
    anyhow::bail!("no SKILL.md or SKILL.toml under {}", root.display());
}

/// List skill package directories under workspace and shared roots.
pub struct SkillIndexTool {
    workspace_dir: PathBuf,
    shared_skills_dir: PathBuf,
}

impl SkillIndexTool {
    pub fn new(workspace_dir: PathBuf, shared_skills_dir: PathBuf) -> Self {
        Self {
            workspace_dir,
            shared_skills_dir,
        }
    }

    fn list_dir_skills(dir: &Path, source_label: &str) -> Vec<String> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return out;
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if p.join("SKILL.md").exists() || p.join("SKILL.toml").exists() {
                out.push(format!("{name} ({source_label})"));
            }
        }
        out.sort();
        out
    }
}

#[async_trait]
impl Tool for SkillIndexTool {
    fn name(&self) -> &str {
        "skill_index"
    }

    fn description(&self) -> &str {
        "List skill packages available locally: instance workspace skills/ and cluster shared skills/ (before querying ClawHub)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult> {
        let mut lines = Vec::new();
        lines.extend(Self::list_dir_skills(
            &self.workspace_dir.join("skills"),
            "instance",
        ));
        lines.extend(Self::list_dir_skills(&self.shared_skills_dir, "shared"));
        let output = if lines.is_empty() {
            "No skill packages found under workspace/skills or shared skills dir.".to_string()
        } else {
            lines.join("\n")
        };
        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

pub struct LoadSkillTool {
    workspace_dir: PathBuf,
    shared_skills_dir: PathBuf,
    target_entity_id: Option<String>,
    entity_pool: Option<Arc<EntityPool>>,
}

impl LoadSkillTool {
    pub fn new(
        workspace_dir: PathBuf,
        shared_skills_dir: PathBuf,
        target_entity_id: Option<String>,
        entity_pool: Option<Arc<EntityPool>>,
    ) -> Self {
        Self {
            workspace_dir,
            shared_skills_dir,
            target_entity_id,
            entity_pool,
        }
    }
}

#[async_trait]
impl Tool for LoadSkillTool {
    fn name(&self) -> &str {
        "load_skill"
    }

    fn description(&self) -> &str {
        "Load skill package text (SKILL.md or SKILL.toml) by package id when allowed by entity skill_allowlist."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Skill package id (directory name)" }
            },
            "required": ["name"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let name = args
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim();
        if name.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("name is required".into()),
            });
        }
        let Some(ref eid) = self.target_entity_id else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("load_skill requires a target entity context".into()),
            });
        };
        if !load_skill_allowed(eid, name, self.entity_pool.as_ref()) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "skill package '{name}' is not allowed for entity '{eid}' (skill_allowlist / CEO policy)"
                )),
            });
        }
        let Some(root) = resolve_skill_root(&self.workspace_dir, &self.shared_skills_dir, name)
        else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("unknown skill package '{name}' on disk")),
            });
        };
        match read_skill_text(&root) {
            Ok(text) => Ok(ToolResult {
                success: true,
                output: text,
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

pub struct ClawhubSearchTool {
    config: Arc<crate::config::Config>,
}

impl ClawhubSearchTool {
    pub fn new(config: Arc<crate::config::Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for ClawhubSearchTool {
    fn name(&self) -> &str {
        "clawhub_search"
    }

    fn description(&self) -> &str {
        "Search the ClawHub public registry (GET /api/v1/search). Requires [skills.clawhub] enabled."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.config.skills.clawhub.enabled {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("ClawHub is disabled; set [skills.clawhub] enabled = true".into()),
            });
        }
        let q = args
            .get("query")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let reg = registry_client(self.config.as_ref())?;
        match reg.search(q).await {
            Ok(text) => Ok(ToolResult {
                success: true,
                output: text,
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

pub struct ClawhubExploreTool {
    config: Arc<crate::config::Config>,
}

impl ClawhubExploreTool {
    pub fn new(config: Arc<crate::config::Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for ClawhubExploreTool {
    fn name(&self) -> &str {
        "clawhub_explore"
    }

    fn description(&self) -> &str {
        "Browse recent skills on ClawHub (GET /api/v1/skills). Requires [skills.clawhub] enabled."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "description": "Max items (1-200)", "default": 25 }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.config.skills.clawhub.enabled {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("ClawHub is disabled; set [skills.clawhub] enabled = true".into()),
            });
        }
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(25) as u32;
        let reg = registry_client(self.config.as_ref())?;
        match reg.explore(limit).await {
            Ok(text) => Ok(ToolResult {
                success: true,
                output: text,
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

fn extract_zip_to_path(zip_bytes: &[u8], dest: &Path) -> Result<()> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader).context("open zip")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let Some(rel) = file.enclosed_name() else {
            continue;
        };
        let out_path = dest.join(rel);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            std::fs::write(&out_path, buf)?;
        }
    }
    Ok(())
}

fn find_skill_package_root(extracted: &Path) -> Result<PathBuf> {
    if extracted.join("SKILL.md").exists() || extracted.join("SKILL.toml").exists() {
        return Ok(extracted.to_path_buf());
    }
    let entries = std::fs::read_dir(extracted).context("read extract dir")?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && (p.join("SKILL.md").exists() || p.join("SKILL.toml").exists()) {
            return Ok(p);
        }
    }
    anyhow::bail!("archive does not contain a valid skill pack (SKILL.md / SKILL.toml)");
}

pub struct ClawhubImportGlobalTool {
    config: Arc<crate::config::Config>,
}

impl ClawhubImportGlobalTool {
    pub fn new(config: Arc<crate::config::Config>) -> Self {
        Self { config }
    }
}

#[derive(Deserialize)]
struct ImportArgs {
    slug: String,
}

#[async_trait]
impl Tool for ClawhubImportGlobalTool {
    fn name(&self) -> &str {
        "clawhub_import_global"
    }

    fn description(&self) -> &str {
        "Download a skill from ClawHub, run security audit, and install under the shared skills directory (global store)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "slug": { "type": "string", "description": "ClawHub skill slug" }
            },
            "required": ["slug"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.config.skills.clawhub.enabled {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("ClawHub is disabled; set [skills.clawhub] enabled = true".into()),
            });
        }
        let a: ImportArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("invalid arguments: {e}")),
                });
            }
        };
        let slug = a.slug.trim();
        if slug.is_empty() || slug.contains("..") || slug.contains('/') {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("invalid slug".into()),
            });
        }
        let reg = match registry_client(self.config.as_ref()) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                });
            }
        };
        let zip_bytes = match reg.download_zip(slug).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                });
            }
        };
        let tmp = tempfile::tempdir().context("tempdir")?;
        extract_zip_to_path(&zip_bytes, tmp.path())?;
        let skill_root = match find_skill_package_root(tmp.path()) {
            Ok(p) => p,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                });
            }
        };
        let report = match crate::skills::audit::audit_skill_directory(&skill_root) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("audit error: {e}")),
                });
            }
        };
        if !report.is_clean() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("security audit failed: {}", report.summary())),
            });
        }
        let dest_root = resolve_shared_skills_dir(self.config.as_ref()).join(slug);
        if dest_root.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "skill '{}' already exists at {}",
                    slug,
                    dest_root.display()
                )),
            });
        }
        std::fs::create_dir_all(dest_root.parent().unwrap_or_else(|| Path::new(".")))?;
        copy_dir_all(&skill_root, &dest_root)?;
        Ok(ToolResult {
            success: true,
            output: format!(
                "Installed skill '{}' to {} (audit passed).",
                slug,
                dest_root.display()
            ),
            error: None,
        })
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src).with_context(|| format!("read {}", src.display()))? {
        let e = e?;
        let ty = e.file_type()?;
        let s = e.path();
        let d = dst.join(e.file_name());
        if ty.is_dir() {
            copy_dir_all(&s, &d)?;
        } else {
            std::fs::copy(&s, &d).with_context(|| format!("copy {:?} -> {:?}", s, d))?;
        }
    }
    Ok(())
}

pub struct AdminSkillRemoveTool {
    config: Arc<crate::config::Config>,
}

impl AdminSkillRemoveTool {
    pub fn new(config: Arc<crate::config::Config>) -> Self {
        Self { config }
    }
}

#[derive(Deserialize)]
struct AdminRemoveArgs {
    slug: String,
}

#[async_trait]
impl Tool for AdminSkillRemoveTool {
    fn name(&self) -> &str {
        "admin_skill_remove"
    }

    fn description(&self) -> &str {
        "Remove a skill package directory from the local shared skills store (does not delete from ClawHub registry)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "slug": { "type": "string", "description": "Package directory name under shared skills" }
            },
            "required": ["slug"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let a: AdminRemoveArgs =
            serde_json::from_value(args).map_err(|e| anyhow::anyhow!("invalid arguments: {e}"))?;
        let slug = a.slug.trim();
        if slug.is_empty() || slug.contains("..") || slug.contains('/') {
            anyhow::bail!("invalid slug");
        }
        let path = resolve_shared_skills_dir(self.config.as_ref()).join(slug);
        if !path.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("no shared skill at {}", path.display())),
            });
        }
        std::fs::remove_dir_all(&path).with_context(|| format!("remove {}", path.display()))?;
        Ok(ToolResult {
            success: true,
            output: format!("Removed {}", path.display()),
            error: None,
        })
    }
}
