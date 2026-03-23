//! Read-only catalog of tools for entity skill assignment and governance.

use super::entity_skills::{tool_inventory_document, ToolInventoryScope};
use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum InventoryScopeArg {
    Assignable,
    Full,
}

#[derive(Default, Deserialize)]
struct ToolInventoryArgs {
    #[serde(default)]
    scope: Option<InventoryScopeArg>,
}

/// Lists tool names, descriptions, and assignment policy for entity `skills` configuration.
pub struct ToolInventoryTool;

impl ToolInventoryTool {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ToolInventoryTool {
    fn name(&self) -> &str {
        "tool_inventory"
    }

    fn description(&self) -> &str {
        "Query MultiClaw tool catalog for entity tool assignment: names, descriptions, tiers (standard vs elevated), and which tools are admin-only or CEO-only (not assignable in entities[].tool_allowlist). Call before create_company or create_entity when configuring tool_allowlist / skill_allowlist."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["assignable", "full"],
                    "description": "assignable: tools valid in entities[].tool_allowlist. full: also lists admin/CEO-only tools and policy notes."
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let parsed: ToolInventoryArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("invalid tool_inventory arguments: {e}")),
                });
            }
        };
        let scope = match parsed.scope.unwrap_or(InventoryScopeArg::Full) {
            InventoryScopeArg::Assignable => ToolInventoryScope::Assignable,
            InventoryScopeArg::Full => ToolInventoryScope::Full,
        };
        let doc = tool_inventory_document(scope);
        let pretty =
            serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
        Ok(ToolResult {
            success: true,
            output: pretty,
            error: None,
        })
    }
}
