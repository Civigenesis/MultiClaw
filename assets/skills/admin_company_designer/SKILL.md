---
name: admin_company_designer
description: >-
  Generates structured company creation drafts for admin, including
  instance/CEO/entity persona markdown and role-specific configuration.
  Use when creating a new company/instance or refining company draft content
  before calling create_company.
---
# Admin Company Designer

Use this skill when admin handles "create company/instance" requests.

## When To Use
- User asks to create a new company or instance.
- Existing draft lacks role-specific detail and must be refined.
- Admin needs consistent markdown structure for instance/CEO/entities.

## Inputs Required
- `company_id`
- `business_domain`
- company goal, scope, constraints, and compliance boundaries
- initial entity roles and target `agent_max`

## Required Workflow
1. Clarify requirements with the user (goal, scope, constraints, teams).
2. Generate markdown content using template files in this folder.
3. Present draft and request explicit user confirmation.
4. After confirmation, call `create_company` once with `action="apply"` and full payload.

## Template Files (same folder)
- `instance_identity_template.md`
- `instance_soul_template.md`
- `instance_agents_template.md`
- `ceo_identity_template.md`
- `ceo_soul_template.md`
- `ceo_agents_template.md`
- `entity_identity_template.md`
- `entity_soul_template.md`
- `entity_agents_template.md`

## Hard Rules
- `entities[]` includes employee entities only; never place CEO in `entities[]`.
- CEO content must use: `ceo_identity_md`, `ceo_soul_md`, `ceo_agents_md`.
- For every employee entity, generate all fields:
  - `identity_md`
  - `soul_md`
  - `agents_md`
- `team_id` should be explicit for every entity; if unknown, use `unassigned`.
- `skills` must contain tool allowlist names (e.g. `file_read`, `memory_recall`), not business capability labels.
- Role descriptions must be materially different (not name-only variations).
- Do not call shell commands for company creation flow.

## Output Checklist Before Apply
- Instance layer: `instance_identity_md`, `instance_soul_md`, `instance_agents_md`
- CEO layer: `ceo_identity_md`, `ceo_soul_md`, `ceo_agents_md`
- Entity layer: each entity has `id`, `role`, optional `team_id`, optional `skills`, and three markdown fields
- Resource layer: `agent_max` plus rationale
- Confirmation: user explicitly confirmed the draft
