//! Shared entity skill allowlist, tiers, and normalization for `create_company` and `create_entity`.
//!
//! **Tiers**
//! - `Standard`: any employee entity may be granted these tools (subject to runtime availability).
//! - `Elevated`: high-risk (e.g. `shell`); only when `role` matches a development/engineering profile
//!   and the assigner is Admin (`create_company`) or CEO (`create_entity`).

use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::HashSet;

/// Tools that may appear in `[[instance.entities]].tool_allowlist` / create_entity payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitySkillTier {
    /// Safe defaults for most roles.
    Standard,
    /// High-risk; only for dev-like roles (see [`role_allows_elevated_skills`]).
    ElevatedDevOnly,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssignableSkillEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub tier: EntitySkillTier,
    pub policy_note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct NonAssignableToolEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub audience: &'static str,
}

/// Static catalog: assignable entity skills (name must match `Tool::name()` for runtime filtering).
pub fn assignable_skill_catalog() -> Vec<AssignableSkillEntry> {
    vec![
        AssignableSkillEntry {
            name: "file_read",
            description: "Read files within workspace/policy.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "file_write",
            description: "Write/create files within workspace/policy.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "file_edit",
            description: "Search/replace style edits in files.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "glob_search",
            description: "Search file paths by glob patterns.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "content_search",
            description: "Search file contents (ripgrep-style).",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "memory_store",
            description: "Store entries in the configured memory backend.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "memory_recall",
            description: "Recall from memory.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "memory_forget",
            description: "Remove or redact memory entries.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "schedule",
            description: "Schedule reminders / timed tasks.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "web_search_tool",
            description: "Web search (when enabled in config).",
            tier: EntitySkillTier::Standard,
            policy_note: "Runtime may omit if disabled.",
        },
        AssignableSkillEntry {
            name: "web_fetch",
            description: "Fetch remote URLs (allowlist policy).",
            tier: EntitySkillTier::Standard,
            policy_note: "Runtime may omit if disabled.",
        },
        AssignableSkillEntry {
            name: "http_request",
            description: "HTTP client (allowlist policy).",
            tier: EntitySkillTier::Standard,
            policy_note: "Runtime may omit if disabled.",
        },
        AssignableSkillEntry {
            name: "image_info",
            description: "Image metadata / vision helper.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "pdf_read",
            description: "Read text from PDFs.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical for all roles.",
        },
        AssignableSkillEntry {
            name: "shell",
            description: "Execute shell commands (policy-gated at runtime).",
            tier: EntitySkillTier::ElevatedDevOnly,
            policy_note: "Only for development/engineering-like roles; assign via Admin (create_company) or CEO (create_entity). Not for generic business roles.",
        },
        AssignableSkillEntry {
            name: "skill_index",
            description: "List skill packages under workspace/skills and the shared skills directory.",
            tier: EntitySkillTier::Standard,
            policy_note: "Typical when using skill packs.",
        },
        AssignableSkillEntry {
            name: "load_skill",
            description: "Read SKILL.md / SKILL.toml for an allowed skill package id.",
            tier: EntitySkillTier::Standard,
            policy_note: "Requires skill_id in entities[].skill_allowlist (or CEO policy for ceo).",
        },
        AssignableSkillEntry {
            name: "clawhub_search",
            description: "Search ClawHub registry (requires [skills.clawhub] enabled).",
            tier: EntitySkillTier::Standard,
            policy_note: "Omitted at runtime when ClawHub is disabled.",
        },
        AssignableSkillEntry {
            name: "clawhub_explore",
            description: "Browse recent skills on ClawHub (requires [skills.clawhub] enabled).",
            tier: EntitySkillTier::Standard,
            policy_note: "Omitted at runtime when ClawHub is disabled.",
        },
    ]
}

/// Tools that are **not** valid in `entities[].tool_allowlist` (instance/role bound; not an employee tool list).
pub fn non_assignable_tool_catalog() -> Vec<NonAssignableToolEntry> {
    vec![
        NonAssignableToolEntry {
            name: "create_company",
            description: "Create cluster company/instance (admin).",
            audience: "admin_instance_only",
        },
        NonAssignableToolEntry {
            name: "instance_status",
            description: "List entities/teams in this instance.",
            audience: "ceo_only",
        },
        NonAssignableToolEntry {
            name: "create_entity",
            description: "Add an entity under this instance.",
            audience: "ceo_only",
        },
        NonAssignableToolEntry {
            name: "create_team",
            description: "Add a team under this instance.",
            audience: "ceo_only",
        },
        NonAssignableToolEntry {
            name: "assign_task",
            description: "Assign work to an entity.",
            audience: "ceo_only",
        },
        NonAssignableToolEntry {
            name: "ceo_skill_grant",
            description: "Grant a skill package id to an entity skill_allowlist.",
            audience: "ceo_only",
        },
        NonAssignableToolEntry {
            name: "clawhub_import_global",
            description: "Download and audit a skill from ClawHub into the shared skills store.",
            audience: "ceo_only",
        },
        NonAssignableToolEntry {
            name: "admin_skill_remove",
            description: "Remove a skill from the shared skills directory.",
            audience: "ceo_only",
        },
        NonAssignableToolEntry {
            name: "delegate",
            description: "Delegate to sub-agents.",
            audience: "not_entity_skill",
        },
        NonAssignableToolEntry {
            name: "tool_inventory",
            description: "This catalog query tool.",
            audience: "meta_not_entity_skill",
        },
    ]
}

fn assignable_names_set() -> HashSet<&'static str> {
    assignable_skill_catalog()
        .into_iter()
        .map(|e| e.name)
        .collect()
}

fn tier_for_skill(name: &str) -> Option<EntitySkillTier> {
    assignable_skill_catalog()
        .into_iter()
        .find(|e| e.name == name)
        .map(|e| e.tier)
}

/// Heuristic: role string suggests development / automation / build work suitable for `shell`.
#[must_use]
pub fn role_allows_elevated_skills(role: Option<&str>) -> bool {
    let r = role.unwrap_or("").to_ascii_lowercase();
    r.contains("开发")
        || r.contains("dev")
        || r.contains("engineer")
        || r.contains("工程师")
        || r.contains("sre")
        || r.contains("backend")
        || r.contains("frontend")
        || r.contains("fullstack")
        || r.contains("构建")
        || r.contains("自动化")
        || r.contains("脚本")
}

/// Default skill packs by role keyword (optional; used when caller omits `skills`).
#[must_use]
pub fn role_based_default_skills(role: Option<&str>) -> Option<Vec<String>> {
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

fn default_skills_minimal() -> Vec<String> {
    vec![
        "file_read".into(),
        "file_write".into(),
        "memory_recall".into(),
    ]
}

/// Default packs when `skills` omitted (`create_entity` path); always returns a non-empty vec.
#[must_use]
pub fn role_based_default_skills_or_minimal(role: Option<&str>) -> Vec<String> {
    role_based_default_skills(role).unwrap_or_else(default_skills_minimal)
}

fn normalize_skill_list(source: Vec<String>, role: Option<&str>) -> Result<Vec<String>> {
    let allow = assignable_names_set();
    let mut normalized = Vec::new();
    for s in source {
        let skill = s.trim().to_string();
        if skill.is_empty() {
            continue;
        }
        if !allow.contains(skill.as_str()) {
            bail!(
                "invalid entity tool '{}': use only assignable tool names from tool_inventory (Standard/Elevated tiers); business labels are not allowed",
                skill
            );
        }
        if let Some(EntitySkillTier::ElevatedDevOnly) = tier_for_skill(&skill) {
            if !role_allows_elevated_skills(role) {
                bail!(
                    "entity tool '{}' is elevated (high-risk): assign only to development/engineering-like roles (clear role text, e.g. 开发/工程师/dev). Others: omit this tool.",
                    skill
                );
            }
        }
        normalized.push(skill);
    }
    Ok(normalized)
}

/// [`create_company`] / company draft: same `Option` chaining as legacy (`None` → role pack → minimal).
pub fn normalize_entity_tool_allowlist_for_company(
    tool_allowlist: Option<Vec<String>>,
    role: Option<&str>,
) -> Result<Vec<String>> {
    let source = tool_allowlist
        .or_else(|| role_based_default_skills(role))
        .unwrap_or_else(default_skills_minimal);
    normalize_skill_list(source, role)
}

/// [`create_entity`] CEO path: when `tool_allowlist` is `None`, use role-based packs with guaranteed fallback vec.
pub fn normalize_entity_tool_allowlist_for_entity(
    tool_allowlist: Option<Vec<String>>,
    role: Option<&str>,
) -> Result<Vec<String>> {
    let source = tool_allowlist.unwrap_or_else(|| role_based_default_skills_or_minimal(role));
    normalize_skill_list(source, role)
}

/// JSON for [`super::tool_inventory::ToolInventoryTool`].
#[must_use]
pub fn tool_inventory_document(scope: ToolInventoryScope) -> serde_json::Value {
    match scope {
        ToolInventoryScope::Assignable => serde_json::json!({
            "scope": "assignable",
            "entity_assignable_skills": assignable_skill_catalog(),
        }),
        ToolInventoryScope::Full => serde_json::json!({
            "scope": "full",
            "entity_assignable_skills": assignable_skill_catalog(),
            "not_assignable_to_entities": non_assignable_tool_catalog(),
            "policy": {
                "standard": "May appear in entities[].tool_allowlist for any role.",
                "elevated_dev_only": "e.g. shell — only when role indicates dev/engineering work; assigned by Admin (create_company) or CEO (create_entity).",
                "admin_tools": "create_company and other cluster-admin capabilities are not entity skills.",
                "ceo_tools": "create_entity, create_team, assign_task, instance_status, ceo_skill_grant, clawhub_import_global, admin_skill_remove are CEO-only (not entity tool_allowlist entries).",
            }
        }),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ToolInventoryScope {
    Assignable,
    Full,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_rejected_without_dev_role() {
        let err = normalize_entity_tool_allowlist_for_company(
            Some(vec!["shell".into()]),
            Some("社交媒体运营"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("elevated") || err.to_string().contains("shell"));
    }

    #[test]
    fn shell_ok_for_dev_role() {
        let v = normalize_entity_tool_allowlist_for_company(
            Some(vec!["shell".into(), "file_read".into()]),
            Some("后端开发工程师"),
        )
        .unwrap();
        assert!(v.contains(&"shell".into()));
    }
}
