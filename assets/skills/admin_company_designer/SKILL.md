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

## Detailed Company-Creation Procedure (authoritative)
1. **Clarify before any tool call**
   - Confirm: `company_id`, business goal, in-scope/out-of-scope, compliance constraints.
   - Confirm: initial org design (CEO + employee entities), `team_id`, role boundaries.
   - Confirm: resource constraint and expected `agent_max`.
2. **Load templates and assemble full draft**
   - Build instance-level markdown: `instance_identity_md`, `instance_soul_md`, `instance_agents_md`.
   - Build CEO-level markdown: `ceo_identity_md`, `ceo_soul_md`, `ceo_agents_md`.
   - Build employee entities only: each item must include `id`, `role`, `team_id` (or `unassigned`), `skills`, `identity_md`, `soul_md`, `agents_md`.
3. **Draft review with user**
   - Present the full draft in structured form.
   - Require explicit confirmation (`确认`) or explicit modifications (`修改: ...`).
4. **Single apply execution**
   - After explicit confirmation, call `create_company` exactly once with `action="apply"`.
   - Include the complete payload in that single call (instance + ceo + entities + constraints).
5. **Result report**
   - On success: report created company id, paths, and created entity list.
   - On failure: report tool error verbatim and ask user whether to revise payload.

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
- Never call `create_company` before explicit user confirmation.
- Never use `create_company` for draft simulation; this skill does drafting in-chat.
- Never bypass this workflow by switching to ad-hoc shell/handwritten file creation.
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
