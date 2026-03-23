---
name: ceo_entity_designer
description: >-
  Standardizes CEO team/entity creation workflow. Converts hiring intent into
  structured team/entity payloads and uses one-shot create_entity with inline
  persona markdown to reduce extra file_write calls.
---
# CEO Entity Designer

Use this skill when CEO handles "create role/member/team" requests.

## When To Use
- User asks to create a role/employee/member/hiring request.
- User asks to create a team/group/department.
- Existing entity draft lacks clear persona structure and needs refinement.

## Inputs Required
- Target type: `entity` or `team`
- Entity minimum fields: `id`, `role`, optional `team_id`, optional `skills`
- Team minimum fields: `id`, optional `name`
- Business goal, collaboration boundaries, and output expectations

## Intent Normalization
- "角色/岗位/员工/成员/招人" => treat as entity creation intent.
- "团队/小组/部门" => treat as team creation intent.
- If ambiguous, ask one clarification question before any tool call.

## Required Workflow
1. Clarify minimum required fields with the user.
2. **Before choosing `skills`**: call `tool_inventory` (`scope: "full"` recommended) and use only names from `entity_assignable_skills` with correct `tier` (standard vs `elevated_dev_only`). Do not invent tool names.
3. Generate draft content using template files in this folder.
4. Present draft and request explicit confirmation (`确认` / `修改: ...`).
5. Execute tools:
   - If team is missing, call `create_team` first.
   - Call `create_entity` once with `identity_md`, `soul_md`, `agents_md`.
6. Report created ids and workspace paths.

## Detailed Team/Entity Procedure (authoritative)
1. **Intent normalization**
   - "角色/岗位/员工/成员/招人" => entity creation.
   - "团队/小组/部门" => team creation.
   - If ambiguous, ask one clarification question before any tool call.
2. **Collect minimal fields**
   - Team: `id`, optional `name`.
   - Entity: `id`, `role`, optional `team_id` (default `unassigned`), optional `skills`.
3. **Prepare markdown before execution**
   - Use folder templates to generate `identity_md`, `soul_md`, `agents_md`.
   - Map `skills` from `tool_inventory` only; elevated tools (e.g. `shell`) only for dev/engineering-like `role` text.
   - Keep role duties concrete and distinct; avoid copy-only variants.
4. **Confirmation gate**
   - Show draft and require explicit confirmation before `create_team`/`create_entity`.
5. **Execute in strict order**
   - If target team does not exist, call `create_team` first.
   - Then call `create_entity` once with inline persona markdown.
6. **Report and close**
   - Return created ids and workspace paths.
   - If tool fails, correct payload and retry; do not degrade to random multi-file writes.

## Template Files (same folder)
- `entity_identity_template.md`
- `entity_soul_template.md`
- `entity_agents_template.md`
- `team_template.md`

## Hard Rules
- Never call `create_entity` without prepared persona markdown unless user explicitly requests minimal scaffold.
- Never skip confirmation when creation payload is materially changed.
- Never bypass this workflow with shell commands.
- Prefer one-shot `create_entity` with inline persona fields.
- Do not use `shell` for team/entity creation.
- Do not use absolute paths in file operations.
- `team_id` should be explicit; if unknown use `unassigned`.
- `skills` must be assignable tool names from `tool_inventory` (`entity_assignable_skills`), not business labels. CEO-only tools (`create_entity`, `instance_status`, …) are listed under `not_assignable_to_entities` and must not be put in `skills`.
- If tool call fails, fix payload and retry; do not degrade into ad-hoc multi-file writes unless explicitly requested.

## Output Checklist
- `tool_inventory` consulted for `skills` tier and names
- Team created (if needed): `id`, optional `name`
- Entity created: `id`, `role`, optional `team_id`, optional `skills`
- Persona written: `identity_md`, `soul_md`, `agents_md`
- User confirmation captured before creation
