# create_company 重复执行根因分析

> 分析日期：2026-03  
> 现象：Admin 创建公司时，`create_company` 被调用 2 次；第一次「没反应」或报错后，第二次发现公司已存在或成功。

---

## 一、现象复述

### 第一次（personal-assistant）
1. 用户确认后，Agent 首次调用 `create_company`
2. 用户批准 [Y]
3. Agent 输出：「需要修正技能名称，让我使用正确的工具白名单名称」
4. Agent 再次调用 `create_company`，用户再次批准 [Y]
5. 结果：「看起来公司已经存在了」
6. Agent 用 `shell` 查看 `instances/` 目录

### 第二次（new-media）
1. 用户确认后，Agent 首次调用 `create_company`
2. 用户批准 [Y]
3. Agent 输出：「让我修正技能名称」
4. Agent 再次调用 `create_company`，用户再次批准 [Y]
5. 第二次创建成功（第一次「没反应」）

---

## 二、根因分析

### 2.1 执行顺序与失败点

`apply_to_instance` 的执行顺序（`src/tools/create_company.rs`）：

```
1. instance_create()        ← 创建实例目录、写入 registry
2. 更新 constraints.agent_max
3. 加载 Config，合并 entities
   └── normalize_entity_skills()  ← 校验每个实体的 skills
4. cfg.save()
5. 写入 persona 文件
```

**关键点**：`instance_create` 在 `normalize_entity_skills` 之前执行。一旦 `instance_create` 成功，实例已被创建并写入 registry；若后续 `normalize_entity_skills` 失败，会返回 `Err`，但**实例已存在**。

### 2.2 触发链

1. **首次调用**  
   - 模型传入 `entities`，其中部分实体的 `skills` 含非 allowlist 名称（如业务能力名「内容策划」「新媒体运营」等）。
   - `instance_create` 成功 → 实例已创建。
   - `normalize_entity_skills` 在校验时 `bail!`：
     ```
     invalid entity skill 'X': skills must be tool allowlist names (e.g. file_read, memory_recall), not business capability labels
     ```
   - 工具返回 `Err`，agent 循环将错误信息交给模型。

2. **模型对错误的解读**  
   - 模型看到「需要修正技能名称」类信息。
   - 模型认为需要修正 `skills` 并重试，**而不是**先检查是否已经创建成功。

3. **第二次调用**  
   - `instance_create` 发现实例已存在 → `bail!("Instance 'X' already exists")`。
   - 或：若首次调用在 skills 校验前就失败（如 `instance_create` 先失败），则第二次可能成功，取决于具体失败点。

### 2.3 为何会传入非法 skills？

- **SKILL 未提供完整 allowlist**：SKILL 只写了「e.g. file_read, memory_recall」，未列出全部合法值。
- **典型错误：`shell`**：实体 `social-media-manager` 常被赋予 `shell`（如「多平台发布」场景），但 `shell` **不在** allowlist 中，会触发 `invalid entity skill 'shell'`。
- **模板占位符**：`entity_identity_template.md` 有 `{{skills}}`，模型可能填入业务能力描述或 `shell`。
- **合法 allowlist**（代码中 `allowed_entity_skills()`）：
  - `file_read`, `file_write`, `file_edit`, `glob_search`, `content_search`
  - `memory_store`, `memory_recall`, `memory_forget`
  - `schedule`, `web_search_tool`, `web_fetch`, `http_request`, `image_info`, `pdf_read`
  - **不包含** `shell`（安全原因）

### 2.4 为何会发起两次调用？

- SKILL 要求「单次 apply」「禁止在未确认时重试」。
- 但 SKILL 未明确说明：**若工具返回错误，应如何判断是否需要重试**。
- 当前仅说明「On failure: report tool error verbatim and ask user whether to revise payload」，未强调：
  - 不得在未询问用户前自动重试；
  - 若错误为「Instance already exists」，应视为已创建，先检查状态再报告。

---

## 三、结论

| 维度 | 根因 |
|------|------|
| **直接原因** | `skills` 校验失败，但 `instance_create` 已成功，导致实例已存在；重试时触发「already exists」。 |
| **技能问题** | SKILL 未给出完整 allowlist；未明确「错误后禁止自动重试」和「先检查再重试」的规则。 |
| **执行顺序** | `instance_create` 在 skills 校验之前，失败时实例已部分创建，形成不一致状态。 |

---

## 四、解决方案（建议）

### 方案 A：Skill 与模板（推荐优先）

1. **在 SKILL 中补充完整 allowlist**：
   ```
   ## Valid skills (tool allowlist)
   Only these values are allowed for entities[].skills:
   file_read, file_write, file_edit, glob_search, content_search,
   memory_store, memory_recall, memory_forget, schedule, web_search_tool,
   web_fetch, http_request, image_info, pdf_read
   ```

2. **在 Hard Rules 中增加**：
   - 若 `create_company` 返回错误，**禁止**在未向用户确认前再次调用。
   - 若错误包含「already exists」或「已存在」，视为创建成功，先汇报当前状态，不要再调用 `create_company`。

3. **在 Result report 中明确**：
   - 成功：汇报端口、路径、实体列表。
   - 失败：原样展示错误；若是 skills 相关，说明需使用 allowlist 中的名称；**要求用户明确确认后再决定是否修改并重试**。

### 方案 B：代码层面

1. **调整执行顺序**（备选）：
   - 在调用 `instance_create` 前，先对 `draft.entities` 执行 `normalize_entity_skills` 校验。
   - 这样 skills 错误会阻止实例创建，避免「实例已存在但 persona 未写完」的中间态。

2. **对「already exists」做特殊处理**（可选）：
   - 若 `instance_create` 返回「already exists」，可选择返回 `ToolResult { success: true, output: "公司已存在，当前状态：..." }`，而不是 `Err`，减少模型误判为需要重试。

### 方案 C：工具 schema

- 在 `parameters_schema` 中为 `entities[].skills` 增加 `description`，明确引用 allowlist 或示例，并注明「仅允许工具名，不得使用业务能力名」。

---

## 五、迁移前 AGENTS 的差异

迁移前 `entity/mod.rs` 中的 admin AGENTS 包含完整的 6 步创建流程，其中有：

- 步骤 5：「工具落盘（收到确认后才允许调用）」
- 步骤 6：「成功汇报 / 失败处理：成功：汇报端口...；失败：只复述 create_company 错误并请求你是否允许『手动检查』」

迁移后改为「详细步骤以 skill 为准」，模型更依赖 SKILL 的细节。若 SKILL 中缺少「失败后禁止自动重试」「already exists 视为成功」等规则，模型容易自动重试，从而出现重复执行。

---

## 六、推荐实施顺序

1. **立即**：在 SKILL 中补充 allowlist 与「禁止自动重试」规则（方案 A）。
2. **短期**：在 `apply_to_instance` 前增加 skills 预校验（方案 B.1）。
3. **可选**：优化「already exists」的返回形式（方案 B.2），并改进工具 schema（方案 C）。

---

## 七、已实施修复（2026-03）

### 现象补充：仅有实例层文件、无 entities 目录

当 `skills` 含非法值（如 `shell`）时：

1. **首次调用**：`instance_create` 成功 → 写入 `workspace/IDENTITY.md` 等（来自 `scaffold_instance_workspace`）；在合并 entities 时 `normalize_entity_skills` 失败 → **未执行** `cfg.save()`、CEO scaffold、实体 scaffold。
2. **二次调用**：`instance_create` 报「already exists」→ 整段 apply 提前返回 → **仍未**写入 CEO/实体。
3. **结果**：`workspace/` 仅有实例级三文件（IDENTITY/SOUL/AGENTS），无 `entities/` 目录。

### 已落地修改

| 修改项 | 文件 | 说明 |
|--------|------|------|
| 完整 allowlist + 禁止 shell | `assets/skills/admin_company_designer/SKILL.md` | 显式列出合法 skills，并注明禁止使用 `shell` |
| 禁止自动重试 + already exists 处理 | 同上 | 明确「不得自动重试」「已存在时视为完成」 |
| skills 预校验 | `src/tools/create_company.rs` | `validate_draft_entities` 在 `instance_create` 前执行，skills 错误时不再创建实例 |
| 幂等续写 | 同上 | 当 `instance_create` 报「already exists」时，从 registry 加载现有 entry，继续完成 persona/entity 写入 |
