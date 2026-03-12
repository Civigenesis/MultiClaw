# MultiClaw 实施方案 — 当前进度与后续 TODO

> 便于下次恢复对话。最后更新：2026-03-09。

---

## 一、本轮已完成（当前进度）

### 1.1 配置加载：provider/model 正确读取

**问题**：运行时仍使用 openrouter，未正确读取 startup1 的 `[instance]` 或顶层 provider/model。

**原因**：
- 解析时只看了顶层 `config.default_provider` / `config.default_model`，未使用 `config.instance.default_provider` / `default_model`。
- 最小 config 无这两项时为 `None`，代码用 `unwrap_or("openrouter")` 静默回退。

**修改**（`src/agent/loop_.rs`）：
- 解析顺序：CLI/实体 override → **`config.instance.default_provider` / `default_model`** → 顶层。
- 合并后仍缺 provider 或 model 时 **`bail!` 明确报错**，不再静默回退。
- 两处构建 provider 的路径均已统一。

### 1.2 实体 scaffold：只保留 AGENTS.md

**问题**：每个实体下同时存在 `agent.md` 和 `AGENTS.md`，重复且易混淆。

**修改**（`src/entity/mod.rs`）：
- 从 `scaffold_entity_workspace` 中移除对 `agent.md` 的写入，**仅保留 AGENTS.md** 作为规范文件。
- 注释标明不生成 agent.md（AGENTS.md 为规范名）。

### 1.3 实例层 vs CEO 层：采用方案 2

**设计选择**：
- **方案 2**：实例层 = 实例/公司目标与默认对话身份；CEO 实体 = 管理与运营（建队、创建实体、分配任务）。
- 实例层（`workspace/` 根）IDENTITY/SOUL/AGENTS 描述「实例身份与目标」；CEO（`workspace/entities/ceo/`）单独负责管理。

**修改**（`src/entity/mod.rs`）：
- `scaffold_instance_workspace` 的 IDENTITY/SOUL/AGENTS 模板改为「实例身份与目标」「与 CEO 的分工」等文案。
- 注释说明：实例层 = 公司/团队身份与目标（默认对话）；CEO = 管理与协调。

### 1.4 其他已做

- 实体 scaffold 在写入前 `create_dir_all(entity_dir)`，避免 CEO scaffold 时报错。
- 实例创建时若 preset 为 startup/enterprise：写入 `[instance]`、`[instance.ceo]`，并 scaffold `workspace/entities/ceo/`（详细 IDENTITY/AGENTS）。
- `create_entity` 成功提示中要求 CEO 为该实体撰写/完善 IDENTITY.md 与 AGENTS.md（50–200 字）。
- 已验证：startup1 配置 qwen-coding-plan / qwen3.5-plus，CEO 执行 instance_status、create_team、create_entity，analyst 获得详细 IDENTITY/AGENTS。

---

## 二、后续 TODO（建议按序或按需）

### 2.1 文档与配置

- [ ] 在 `docs/config-reference.md` 中补充 `[instance]` 下 `default_provider`、`default_model` 的说明及与顶层优先级。
- [ ] （可选）`instance create --preset startup` 生成的最小 config 中增加注释行，提示用户添加 `default_provider` / `default_model`（或保持当前“缺则报错”策略）。

### 2.2 已有实体目录清理

- [ ] 若存量实体目录中存在多余的 `agent.md`，可统一删除或合并进 `AGENTS.md`（脚本或文档说明即可）。

### 2.3 阶段 2 收尾（多实体）

- [ ] 阶段 2 其余项对照 [改造实施计划-阶段2-多实体](../完整方案/改造实施计划-阶段2-多实体.md) 做完成度检查与测试。
- [ ] 阶段 2.5 技能作用域、ClawHub 等按 [改造实施计划-阶段2.5-技能](../完整方案/改造实施计划-阶段2.5-技能.md) 推进。

### 2.4 阶段 3（主动性对话）

- [ ] MessageBus / ConversationBus、`assign_task` 真实投递（当前为占位）。
- [ ] ProactiveScheduler、主动触达逻辑，参见 [改造实施计划-阶段3-主动性对话](../完整方案/改造实施计划-阶段3-主动性对话.md)。

### 2.5 阶段 4–6

- [ ] 通信、可观测、故障恢复按 [总览](../完整方案/改造实施计划-总览.md) 中各阶段文档推进。

---

## 三、关键文件索引

| 用途 | 路径 |
|------|------|
| 配置解析（instance 优先、缺则报错） | `src/agent/loop_.rs` |
| 实体/实例 scaffold、方案 2 文案 | `src/entity/mod.rs` |
| 实例创建、CEO scaffold、minimal config | `src/instance_manager.rs` |
| CEO 工具、create_entity 提示 | `src/tools/ceo.rs` |
| 实施计划总览 | `docs/multiclaw/完整方案/改造实施计划-总览.md` |
| 阶段 2 多实体 | `docs/multiclaw/完整方案/改造实施计划-阶段2-多实体.md` |

---

## 四、恢复对话时可用的简短上下文

- **配置**：provider/model 现按「CLI/实体 override → [instance] → 顶层」解析；缺则报错，不再静默 openrouter。
- **实体**：只生成 IDENTITY.md、SOUL.md、AGENTS.md；不生成 agent.md。
- **实例 vs CEO**：实例层 = 公司/目标与默认对话；CEO = 管理运营；两套配置不同是预期行为。
- **验证**：startup1 已用 qwen-coding-plan / qwen3.5-plus 跑通 CEO 建队、建实体、写 analyst 详细配置。
