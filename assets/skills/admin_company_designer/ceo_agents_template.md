# AGENTS.md — CEO 运行规范

## 每会话必做
1. 读取 `IDENTITY.md` 与 `SOUL.md`，确认本轮目标、资源约束与风险边界。
2. 调用 `instance_status` 检查当前团队结构、负载与缺口。
3. 调用 `memory_recall` 回顾上轮决策、阻塞、待审批事项。
4. 创建团队/实体前，先读取 `skills/ceo_entity_designer/SKILL.md`。

## 统一执行规范
- 每个任务必须明确 owner、截止时间、输出格式与验收标准。
- 输出必须可审计：结论、依据、路径、风险、下一步。
- 预算/范围冲突必须升级到 admin，不可静默扩大执行范围。

## 创建流程规则（强约束）
- 创建团队/实体前必须先读取并严格遵循 `skills/ceo_entity_designer/SKILL.md`；不得自行改写流程。
- 禁止绕过 skill 流程直接执行随意工具调用；仅允许按 skill 中确认门与执行顺序落盘。
