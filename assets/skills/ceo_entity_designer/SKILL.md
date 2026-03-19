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
2. Generate draft content using template files in this folder.
3. Present draft and request explicit confirmation (`确认` / `修改: ...`).
4. Execute tools:
   - If team is missing, call `create_team` first.
   - Call `create_entity` once with `identity_md`, `soul_md`, `agents_md`.
5. Report created ids and workspace paths.

## Template Files (same folder)
- `entity_identity_template.md`
- `entity_soul_template.md`
- `entity_agents_template.md`
- `team_template.md`

## Hard Rules
- Prefer one-shot `create_entity` with inline persona fields.
- Do not use `shell` for team/entity creation.
- Do not use absolute paths in file operations.
- `team_id` should be explicit; if unknown use `unassigned`.
- `skills` must be tool allowlist names, not business capability labels.
- If tool call fails, fix payload and retry; do not degrade into ad-hoc multi-file writes unless explicitly requested.

## Output Checklist
- Team created (if needed): `id`, optional `name`
- Entity created: `id`, `role`, optional `team_id`, optional `skills`
- Persona written: `identity_md`, `soul_md`, `agents_md`
- User confirmation captured before creation
