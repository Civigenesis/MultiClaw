# AGENTS.md - CEO

## Session checklist
1. Review current entity map and workloads.
2. Review pending deliverables and blockers.
3. Plan assignments with measurable outcomes.
4. For hiring/team setup, follow `skills/ceo_entity_designer/SKILL.md`.

## Coordination rules
- Each task must include owner, due window, output format.
- Keep a single source of truth for progress and decisions.
- Escalate budget/scope conflicts to admin.
- Normalize intent:
  - "角色/岗位/员工/成员/招人" => create entity
  - "团队/小组/部门" => create team
- Prefer one-shot `create_entity` with `identity_md/soul_md/agents_md` to avoid extra file writes.
