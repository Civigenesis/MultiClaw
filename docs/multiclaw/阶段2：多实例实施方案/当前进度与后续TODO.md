# MultiClaw 实施方案 — 当前进度与后续 TODO

> 便于下次恢复对话。最后更新：2026-03-18。

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

### 1.5 董事长（Admin）实例：首次初始化与默认对话对象

**设计目标**：默认初始化的第一个全局实例应为董事长实例（admin），不指定实例时作为默认对话对象，用于创建公司、了解全局状态、向 CEO 传递信息。

**修改**：
- **`src/entity/mod.rs`**：新增 `scaffold_admin_workspace(workspace_dir)`，写入董事长专用 IDENTITY.md、SOUL.md、AGENTS.md（创建公司/instance list/admin-message 等说明）。
- **`src/instance_manager.rs`**：新增 `ensure_admin_instance(cluster_root)`，创建 `instances/admin/`、最小 config、注册表，并调用 `scaffold_admin_workspace`；`ADMIN_INSTANCE_ID`、`ADMIN_GATEWAY_PORT` 常量导出。
- **`src/config/schema.rs`**：`load_or_init` 时：① 集群模式且未指定 `--instance` 时**默认使用 admin**；② 首次初始化（无 config、无集群）时**自动创建集群 + admin 并加载 admin**，保证第一全局实例为董事长。
- **`src/onboard/wizard.rs`**：`ensure_admin_instance` 委托给 `instance_manager::ensure_admin_instance`；引导/快速设置时若当前 workspace 为 admin，**不再用通用 agent 模版覆盖**，保留董事长 scaffold。
- **`src/onboard/wizard.rs`（本次补充）**：当检测到“首次初始化且无现有 config”时，onboard 将**默认创建 cluster + admin**，避免用户在首次初始化时落到通用 workspace 模板（导致误以为 admin 未生效）。
- **Admin 能力**：创建公司 = CLI `multiclaw instance create`（仅 admin 实例可执行）；查看状态 = `multiclaw instance list` / `instance status`；传递信息给 CEO = Gateway `POST /api/admin-message`。董事长 AGENTS.md 中已说明上述能力与用法。

---

## 二、后续 TODO（建议按序或按需）

### 2.1 文档与配置

- [ ] 在 `docs/config-reference.md` 中补充 `[instance]` 下 `default_provider`、`default_model` 的说明及与顶层优先级。
- [ ] （可选）`instance create --preset startup` 生成的最小 config 中增加注释行，提示用户添加 `default_provider` / `default_model`（或保持当前“缺则报错”策略）。

### 2.2 已有实体目录清理

- [ ] 若存量实体目录中存在多余的 `agent.md`，可统一删除或合并进 `AGENTS.md`（脚本或文档说明即可）。

### 2.3 阶段 2 收尾（多实体）— 方案已落地

**P0（已纳入本次实现）**：
- [x] **agent_max 注入**：从集群 `instances.json` 的 `constraints.agent_max` 解析，在 `run()` 中传入 CEO 工具，使 `create_entity` 遵守实例上限。
- [x] **每实例/每实体独立 agent、skill、memory**：  
  - 实例：集群模式下每实例已有独立 config/workspace（`instances/<id>/`）。  
  - 实体：当 `target_entity_id` 设定时，**memory** 使用 `workspace/entities/<id>/` 作为存储根；**skills** 从实体 workspace（`entities/<id>/skills/`）加载；**agent** 已按实体 provider/model 与 prompt 目录隔离。  
- [x] **实体 skills 白名单**：当实体配置了 `skills` 且非空时，工具列表按 `skills_allowlist` 过滤，仅暴露允许的工具。
- [x] **CEO 完整建队与管理**：`instance_status` 输出包含本实例的 **teams** 与 **entities**，便于 CEO 查看并管理团队与成员。

**其余**：
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
- **隔离**：每实例独立 config/workspace；多实体时每实体独立 memory 根（`entities/<id>/`）、skills（`entities/<id>/skills/`）、prompt 与 provider/model；实体 `skills` 非空时工具按白名单过滤。
- **agent_max**：集群模式下从 `instances.json` 当前实例的 `constraints.agent_max` 注入 CEO 工具。
- **验证**：startup1 已用 qwen-coding-plan / qwen3.5-plus 跑通 CEO 建队、建实体、写 analyst 详细配置。
