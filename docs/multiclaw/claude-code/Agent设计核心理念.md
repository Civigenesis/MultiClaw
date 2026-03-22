# Agent 设计核心理念（从 s01-s12 抽象到可复用架构）

本文件是对 `learn-claude-code` 教学版 `s01-s12` 的“设计理念抽象”。目标不是复刻教学代码，而是把其中的**可迁移架构思想**整理成你后续设计自己 agent 系统时的参考清单。

---

## 一句话总纲
把 agent 做成**稳定的不变循环（Loop）** + **可插拔的 harness 机制（Tools / Memory / Persistence / Coordination）**，并通过：
1) 控制面（What/State）持久化  
2) 执行面（Where/Isolation）隔离  
3) 协作面（Who/Protocol）有握手  
来让复杂任务在多轮、多智能体、长上下文下仍然可靠。

---

## 核心设计原则（贯穿 s01-s12）
1. **循环不动，机制迭代**：`while tool_use -> 执行工具 -> 把结果喂回模型` 的闭环保持不变，其他能力通过扩展 tools/状态机加入。
2. **工具分发替代 if/elif**：每个工具对应一个 handler（并配输入 schema），循环只负责“分发”，降低扩展成本。
3. **上下文可控，而不是无限堆叠**：必须考虑 token 与“历史越来越乱”的问题，所以引入压缩策略，把旧信息移出活跃上下文。
4. **状态持久化，记忆外置**：长任务不能只依赖 messages 数组，任务、队友、事件、worktree 索引都应落盘。
5. **隔离比并发更重要**：并发/多智能体最容易“写冲突与回滚困难”，用目录或分支隔离执行通道。
6. **可观测性要内建**：不要只靠打印/对话回忆；用 append-only 事件流记录关键生命周期步骤（创建/删除/完成等）。
7. **协作靠协议，而不是靠默契**：shutdown/plan approval 等高风险动作需要结构化 request-response，并用 request_id 关联双方状态。

---

## s01-s12：每一步的核心思想

### s01：Agent Loop（智能体循环）
**解决的问题**：模型会推理，但没有“连接真实世界”的循环；每次工具调用结果需要你手工回填。  
**核心思想**：建立唯一的运行闭环：`模型 -> tool_use -> 执行工具 -> tool_result -> 再喂回模型`，由 `stop_reason` 决定何时结束。  
**落地机制**：固定的循环结构 + 把工具执行结果以 `tool_result` 追加到 messages，直到模型不再调用工具。

迁移要点：把“agent 的操作能力边界”抽象成 tools，把“与外界交互”抽象成循环闭环；后续所有机制都是在这个闭环之上叠加。

---

### s02：Tool Use（工具使用）
**解决的问题**：只有 bash 时，无法精确控制输入输出；并且纯 shell 扩展安全面大，路径不可控。  
**核心思想**：循环不变，只要新增工具就能扩展能力；但要在工具层做安全与沙箱。  
**落地机制**：  
- `safe_path()` 做路径沙箱（防止逃逸工作区）  
- `TOOL_HANDLERS = { tool_name: handler }` 用分发表替代硬编码逻辑  
- 每个工具有自己的 schema 与 handler，循环统一处理 tool_use。

迁移要点：把工具系统当作 agent 的“API 面”，并确保扩展工具时不会触碰循环内核。

---

### s03：TodoWrite（待办写入）
**解决的问题**：多步任务在长对话里容易“丢进度/重复/跑偏”。系统提示的影响力会被上下文噪声稀释。  
**核心思想**：让模型把计划当作一种“可被工具系统更新的状态”，并用约束强迫它持续同步。  
**落地机制**：  
- TodoManager：维护带状态的清单，并强制一次只能有一个 `in_progress`  
- `todo` 工具：模型用 tool 调用更新任务清单  
- nag reminder：模型连续多轮不更新计划就注入提醒

迁移要点：规划不应该是“口头承诺”，而应该变成 agent 的可验证状态（至少在教学版里是这样）。

---

### s04：Subagents（子智能体）
**解决的问题**：messages 越堆越胖，父智能体读很多工具输出，而它只需要一个结论（例如测试框架名）。  
**核心思想**：用“子对话隔离”把上下文污染隔离开。  
**落地机制**：父 agent 提供 `task` 工具；子 agent 使用**独立** `messages=[]`，跑完只返回摘要/结果文本给父 agent。

迁移要点：当你需要“复杂探索/多轮推理”，把它包装成子 agent，让父对话保持干净。

---

### s05：Skills（技能加载）
**解决的问题**：把所有领域规则塞进系统提示会浪费 token；而且“用不到的技能”应该不要出现。  
**核心思想**：把知识分成“便宜的索引”和“昂贵的正文”，按需注入。  
**落地机制**：两层注入：  
1) 第一层：系统提示里列出技能名与简短描述（低成本）  
2) 第二层：模型调用 `load_skill(name)` 时，在 `tool_result` 里注入完整 SKILL 内容（高成本但按需）

迁移要点：把可复用工作流/领域知识模块化为 skills，并通过工具按需加载，避免系统 prompt 膨胀。

---

### s06：Context Compact（上下文压缩）
**解决的问题**：上下文窗口有限；长项目读取大量文件与命令输出会爆 token，导致后续决策不可控。  
**核心思想**：旧信息不是删除就是压缩，必须有分层策略。  
**落地机制**：三层压缩（激进程度递增）：  
1) micro_compact：每轮把过旧的 tool_result 用占位符替换  
2) auto_compact：token 超过阈值时，把完整对话写到 `.transcripts/`，再让 LLM 摘要，替换 messages  
3) compact tool：提供手动触发的压缩入口

迁移要点：不要等 token 爆掉才处理；压缩策略要“可预测 + 可恢复（落盘转录）”。

---

### s07：Task System（任务系统）
**解决的问题**：s03 的 Todo 是内存清单，缺少依赖结构；真实工作需要 DAG（谁依赖谁）、状态（pending/in_progress/completed）、以及可恢复性。  
**核心思想**：把“任务”升级为持久化任务图（DAG），让多个机制共同读取同一套状态。  
**落地机制**：每个任务落盘为 JSON 文件，支持：  
- 任务依赖 `blockedBy/blocks`  
- 完成自动解锁后续  
- CRUD 工具：`task_create/task_get/task_update/task_list`

迁移要点：让“规划”进入磁盘，成为控制面（Control Plane）的稳定状态源。

---

### s08：Background Tasks（后台任务）
**解决的问题**：长命令会阻塞循环，模型只能等；但真实 agent 需要并行“跑命令 + 思考下一步”。  
**核心思想**：把慢操作丢进后台线程/进程，主循环仍保持运行；结果用队列在下一次 LLM 调用前注入。  
**落地机制**：  
- 后台运行器：`background_run`  
- `drain notifications`：每次 LLM 调用前排空通知队列，转成 `<background-results>` 注入 messages

迁移要点：并发要有“结果注入点”，否则并发会变成不可解释的状态漂移。

---

### s09：Agent Teams（智能体团队）
**解决的问题**：子智能体是一次性的；团队协作需要“跨多轮存活 + 身份 + 通信通道”。  
**核心思想**：用持久化队友 + JSONL 邮箱建立低成本通信。  
**落地机制**：  
- `config.json` 记录团队名册与每个队友状态  
- `inbox/*.jsonl` 作为 append-only 邮箱，读取时 drain-on-read  
- 队友循环：每次 LLM 调用前读取 inbox 注入上下文

迁移要点：团队通信最好有“可落盘的协议载体”，不要只靠内存消息。

---

### s10：Team Protocols（团队协议）
**解决的问题**：协作里的高风险动作（比如关机、计划审批）不能靠“自然退出/口头同意”，否则会留下半完成状态与不可预测行为。  
**核心思想**：把关键协调都做成 request-response 握手，并复用同一套 request_id + FSM（approve/reject）。  
**落地机制**：  
- shutdown protocol：领导发 shutdown_request，队友发 shutdown_response  
- plan approval：领导审查队友计划，引用同一个 request_id  
- shared FSM：`pending -> approved | rejected`

迁移要点：当涉及“是否继续执行/是否清理现场”这种决策时，协议比自由发挥更可靠。

---

### s11：Autonomous Agents（自治智能体）
**解决的问题**：如果必须由领导逐个分配，团队规模上限会很低；更理想的是自组织认领任务。  
**核心思想**：队友在闲时（IDLE）轮询任务板与收件箱，发现未认领任务就 claim，然后切回 WORK。  
**落地机制**：  
- WORK/IDLE 生命周期  
- IDLE 阶段：scan `.tasks/` 找 pending 且无 owner 的任务，claim 后工作  
- 身份重注入：压缩后消息过短时插入 identity block，避免“我是谁”丢失

迁移要点：自治需要“节拍器（轮询/超时）+ 可识别身份 + 可抢占式任务认领”。

---

### s12：Worktree + Task Isolation（Worktree 任务隔离）
**解决的问题**：到 s11 为止，任务能认领与完成了，但执行可能仍共享同一目录；多个队友并行改同一文件会污染彼此修改，回滚也难以做到“只撤一个任务”。  
**核心思想**：把任务控制面与执行面分离：  
1) task_id 管“做什么”  
2) git worktree 管“在哪做”  
3) 在隔离目录里执行命令，从根上避免写冲突。  
**落地机制**：  
- `.tasks/` 记录任务状态并带 `worktree` 字段  
- `.worktrees/index.json` 做 worktree 注册表（name/path/branch/task_id/status）  
- 创建 worktree 时可传 `task_id`，实现双向绑定并推进任务状态  
- 收尾支持 `keep`（保留继续做）和 `remove(complete_task=true)`（删除并标记任务完成）  
- `.worktrees/events.jsonl` 记录生命周期事件，便于恢复与审计

迁移要点：生产系统里“隔离执行”通常是并发安全的底座；worktree 是一种强隔离方式（也可以替换为容器/分支/沙箱目录等）。

---

## 最后完整框架：从 s_full 到你的系统蓝图

教学版 `s_full.py` 是把 `s01-s11` 的机制合体后的“完整 cockpit”，核心思想可以直接迁移到你的系统：

### 组件分层（建议的工程化拆分）
1. **Loop 内核层（稳定）**  
   - 固定闭环：LLM -> tool_use -> tool_result -> 回到下一轮  
   - `stop_reason` 决定何时结束
2. **工具与分发层（可扩展）**  
   - `TOOL_HANDLERS`：按工具名路由 handler  
   - `TOOLS`：工具 schema 给模型约束输入
3. **内存与上下文层（可控）**  
   - `microcompact/auto_compact`：在每轮前压缩旧 tool 输出  
   - `transcripts` 落盘：压缩前可恢复
4. **控制面（Control Plane）**  
   - Todo（s03）用于短期计划同步  
   - Task System（s07）用于长期 DAG 状态
5. **执行面（Execution Plane）**  
   - Background Tasks（s08）并行执行命令  
   - Teams 邮箱与队友执行（s09）  
   - 协议握手（s10）约束跨智能体决策  
   - 自治认领（s11）让团队自驱动  
   - `s12`：worktree 隔离并行写冲突
6. **协调与治理层**  
   - request_id + FSM：计划审批/关机等高风险动作
7. **可观测性层**  
   - append-only events（s12 的 events.jsonl 是典型做法；s_full 可用类似策略补齐）

---

### s_full 的“每轮执行管线”（通俗时序）
在每次 LLM 调用之前，会依次做（对应 `s_full.py`）：
1. `microcompact(messages)`：把过旧 tool_result 清理/占位  
2. 如果 token 超阈值：`auto_compact(messages)` 把对话摘要化  
3. drain background：把后台任务结果注入 messages  
4. read inbox：把领导收件箱消息注入 messages  
5. 调用 LLM（tools + system prompt）
6. 若模型触发 tool_use：执行对应 handler，把结果以 tool_result 追加
7. s03 纪律：如果 Todo 工作流活跃且模型连续不更新，插入 nag reminder
8. 如果模型手动调用 `compress`：再执行 auto_compact

可迁移的一点是：**“主循环的每一轮前处理”要显式化**，否则并发结果、邮箱消息、压缩策略都会变得不可控。

---

### 建议的“系统蓝图”（你后续设计 agent 时可照着填）
你可以把自己的 agent 体系写成下面这个抽象模板：

1. **定义 Loop 内核**：工具闭环 + 退出条件  
2. **定义 Tools 契约**：每个工具名 -> handler + schema  
3. **定义控制面状态**：tasks/todos/团队成员/协议 request 表等（落盘）  
4. **定义执行面隔离**：并行任务的隔离策略（worktree / 分支 / 容器 / 沙箱目录）  
5. **定义压缩与恢复**：micro/auto/手动压缩 + transcripts  
6. **定义协作协议**：request-response + FSM（approve/reject）  
7. **定义可观测性**：events.jsonl 这类 append-only 日志  
8. **定义自治策略**：IDLE 扫描/claim/超时关机，附带身份重注入

---

## 生产级注意（从教学到可用的差距）
`s01-s12` 已覆盖很多关键思想，但它仍是教学工程。你做自己的系统时，通常还需要额外补齐：
- 更严谨的安全治理：工具权限、路径与命令白名单、敏感操作确认
- 更完善的错误恢复：失败重试策略、死信队列、幂等写入
- 更强的并发一致性：worktree/容器资源回收、冲突检测与审计
- 更完整的事件总线：把每个关键生命周期都落日志，便于回放与分析

---

## 你接下来可以直接做的事
把你要做的 agent 体系用本文件的模板“逐项落地”，并把你最关心的 3 件事告诉我（例如：安全治理/并发隔离/任务协作），我可以帮你把架构进一步具体化成模块清单和接口草案。

---

## 附录：每章核心设计图/关键代码（用于后续实现对照）

### s01：Agent Loop（稳定闭环）
架构图（Harness 核心）：
```text
User prompt -> LLM -> tool_use -> execute -> tool_result -> (回到 LLM)
                    stop_reason != "tool_use" 时退出
```
关键代码（循环 + 退出条件）：
```python
def agent_loop(query):
    messages = [{"role": "user", "content": query}]
    while True:
        response = client.messages.create(
            model=MODEL, system=SYSTEM, messages=messages,
            tools=TOOLS, max_tokens=8000,
        )
        messages.append({"role": "assistant", "content": response.content})

        if response.stop_reason != "tool_use":
            return

        results = []
        for block in response.content:
            if block.type == "tool_use":
                output = run_bash(block.input["command"])
                results.append({
                    "type": "tool_result",
                    "tool_use_id": block.id,
                    "content": output,
                })
        messages.append({"role": "user", "content": results})
```

---

### s02：Tool Use（工具分发 + 路径沙箱）
核心架构（Dispatch map 替代 if/elif）：
```text
LLM -> tool_use(name, input) -> TOOL_HANDLERS[name](**input) -> tool_result
```
关键代码（安全路径 + 分发表）：
```python
def safe_path(p: str) -> Path:
    path = (WORKDIR / p).resolve()
    if not path.is_relative_to(WORKDIR):
        raise ValueError(f"Path escapes workspace: {p}")
    return path

TOOL_HANDLERS = {
    "bash":       lambda **kw: run_bash(kw["command"]),
    "read_file":  lambda **kw: run_read(kw["path"], kw.get("limit")),
    "write_file": lambda **kw: run_write(kw["path"], kw["content"]),
    "edit_file":  lambda **kw: run_edit(kw["path"], kw["old_text"], kw["new_text"]),
}

for block in response.content:
    if block.type == "tool_use":
        handler = TOOL_HANDLERS.get(block.name)
        output = handler(**block.input) if handler else f"Unknown tool: {block.name}"
        results.append({
            "type": "tool_result",
            "tool_use_id": block.id,
            "content": output,
        })
```

---

### s03：TodoWrite（把“计划”变成可更新状态）
核心架构（TodoManager 状态 + nag 纪律）：
```text
LLM -> todo工具(tool_result) -> TodoManager 状态更新
连续多轮不更新时 -> 注入 <reminder> 文本
```
关键代码（TodoManager 更新约束）：
```python
class TodoManager:
    def update(self, items: list) -> str:
        validated, in_progress_count = [], 0
        for item in items:
            status = item.get("status", "pending")
            if status == "in_progress":
                in_progress_count += 1
            validated.append({"id": item["id"], "text": item["text"],
                               "status": status})
        if in_progress_count > 1:
            raise ValueError("Only one task can be in_progress")
        self.items = validated
        return self.render()
```
nag reminder 注入（教学版的纪律约束）：
```python
if rounds_since_todo >= 3 and messages:
    last = messages[-1]
    if last["role"] == "user" and isinstance(last.get("content"), list):
        last["content"].insert(0, {
            "type": "text",
            "text": "<reminder>Update your todos.</reminder>",
        })
```

---

### s04：Subagents（上下文隔离：父干净、子短命）
核心架构：
```text
Parent: messages=[...] 只接收子 agent 的摘要
Subagent: messages=[] 独立跑工具，结束后丢弃上下文
```
关键代码（父提供 task 工具，子用独立 messages=[]）：
```python
def run_subagent(prompt: str) -> str:
    sub_messages = [{"role": "user", "content": prompt}]
    for _ in range(30):  # safety limit
        response = client.messages.create(
            model=MODEL, system=SUBAGENT_SYSTEM,
            messages=sub_messages,
            tools=CHILD_TOOLS, max_tokens=8000,
        )
        sub_messages.append({"role": "assistant", "content": response.content})
        if response.stop_reason != "tool_use":
            break
        results = []
        for block in response.content:
            if block.type == "tool_use":
                handler = TOOL_HANDLERS.get(block.name)
                output = handler(**block.input)
                results.append({"type": "tool_result",
                                 "tool_use_id": block.id,
                                 "content": str(output)[:50000]})
        sub_messages.append({"role": "user", "content": results})
    return "".join(b.text for b in response.content if hasattr(b, "text")) or "(no summary)"
```

---

### s05：Skills（两层注入：便宜索引 + 按需正文）
核心架构（system prompt 放技能名；load_skill 时才注入正文）：
```text
Layer 1: system prompt 里列 Skills（低成本）
Layer 2: tool_result(load_skill) 注入完整 SKILL.md（高成本）
```
关键代码（SkillLoader + load_skill 工具）：
```python
class SkillLoader:
    def load(self, name: str) -> str:
        s = self.skills.get(name)
        if not s:
            return f"Error: Unknown skill '{name}'."
        return f"<skill name=\"{name}\">\n{s['body']}\n</skill>"

TOOL_HANDLERS = {
    "load_skill": lambda **kw: SKILLS.load(kw["name"]),
}

SYSTEM = f"""You are a coding agent at {WORKDIR}.
Skills available:
{SKILLS.descriptions()}"""
```

---

### s06：Context Compact（三层压缩：微观占位 -> 阈值摘要 -> 手动压缩）
核心架构（每轮前处理 + 必要时摘要）：
```text
每轮：micro_compact（旧 tool_result 用占位符）
超阈值：auto_compact（落盘 transcripts + LLM 摘要替换 messages）
手动：compact tool -> auto_compact
```
关键代码（micro_compact 思路）：
```python
def microcompact(messages: list):
    indices = []
    for i, msg in enumerate(messages):
        if msg["role"] == "user" and isinstance(msg.get("content"), list):
            for part in msg["content"]:
                if isinstance(part, dict) and part.get("type") == "tool_result":
                    indices.append(part)
    if len(indices) <= 3:
        return
    for part in indices[:-3]:
        if isinstance(part.get("content"), str) and len(part["content"]) > 100:
            part["content"] = "[cleared]"
```
关键代码（auto_compact：落盘 + 摘要替换）：
```python
def auto_compact(messages: list) -> list:
    transcript_path = TRANSCRIPT_DIR / f"transcript_{int(time.time())}.jsonl"
    with open(transcript_path, "w") as f:
        for msg in messages:
            f.write(json.dumps(msg, default=str) + "\n")
    conv_text = json.dumps(messages, default=str)[:80000]
    resp = client.messages.create(
        model=MODEL,
        messages=[{"role": "user", "content": f"Summarize for continuity:\n{conv_text}"}],
        max_tokens=2000,
    )
    summary = resp.content[0].text
    return [
        {"role": "user", "content": f"[Compressed. Transcript: {transcript_path}]\n{summary}"},
        {"role": "assistant", "content": "Understood. Continuing with summary context."},
    ]
```

---

### s07：Task System（控制面：持久化 DAG + 状态驱动协作）
核心架构（任务图 DAG + 解锁机制）：
```text
.tasks/
  task_1.json status=completed
  task_2.json blockedBy=[1] status=pending
任务完成 -> 自动把完成的 id 从 blockedBy 移除 -> 解锁后续
```
关键代码（创建任务 + 完成时清理依赖）：
```python
class TaskManager:
    def __init__(self, tasks_dir: Path):
        self.dir = tasks_dir
        self.dir.mkdir(exist_ok=True)
        self._next_id = self._max_id() + 1

    def create(self, subject, description=""):
        task = {"id": self._next_id, "subject": subject,
                "status": "pending", "blockedBy": [],
                "blocks": [], "owner": ""}
        self._save(task)
        self._next_id += 1
        return json.dumps(task, indent=2)

def _clear_dependency(self, completed_id):
    for f in self.dir.glob("task_*.json"):
        task = json.loads(f.read_text())
        if completed_id in task.get("blockedBy", []):
            task["blockedBy"].remove(completed_id)
            self._save(task)

def update(self, task_id, status=None,
           add_blocked_by=None, add_blocks=None):
    task = self._load(task_id)
    if status:
        task["status"] = status
        if status == "completed":
            self._clear_dependency(task_id)
    self._save(task)
```
关键代码（任务工具：create/update/list/get 加到 dispatch map）：
```python
TOOL_HANDLERS = {
    "task_create": lambda **kw: TASKS.create(kw["subject"]),
    "task_update": lambda **kw: TASKS.update(kw["task_id"], kw.get("status")),
    "task_list":   lambda **kw: TASKS.list_all(),
    "task_get":    lambda **kw: TASKS.get(kw["task_id"]),
}
```

---

### s08：Background Tasks（执行并发：后台跑命令 + 下一轮注入结果）
核心架构（主循环不阻塞）：
```text
主线程：LLM 思考每轮
后台线程：subprocess.run(command)
每次 LLM 调用前：drain notifications -> 注入 <background-results> -> 再调用 LLM
```
关键代码（后台 run + 通知队列）：
```python
def run(self, command: str) -> str:
    task_id = str(uuid.uuid4())[:8]
    self.tasks[task_id] = {"status": "running", "command": command}
    thread = threading.Thread(
        target=self._execute, args=(task_id, command), daemon=True)
    thread.start()
    return f"Background task {task_id} started"

def _execute(self, task_id, command):
    r = subprocess.run(command, shell=True, cwd=WORKDIR,
        capture_output=True, text=True, timeout=300)
    output = (r.stdout + r.stderr).strip()[:50000]
    with self._lock:
        self._notification_queue.append({
            "task_id": task_id, "result": output[:500]})

def agent_loop(messages: list):
    while True:
        notifs = BG.drain_notifications()
        if notifs:
            notif_text = "\n".join(
                f"[bg:{n['task_id']}] {n['result']}" for n in notifs)
            messages.append({"role": "user",
                "content": f"<background-results>\n{notif_text}\n</background-results>"})
            messages.append({"role": "assistant", "content": "Noted background results."})
        response = client.messages.create(
            model=MODEL, system=SYSTEM, messages=messages,
            tools=TOOLS, max_tokens=8000,
        )
        messages.append({"role": "assistant", "content": response.content})
        if response.stop_reason != "tool_use":
            return
        results = []
        for block in response.content:
            if block.type == "tool_use":
                handler = TOOL_HANDLERS.get(block.name)
                try:
                    output = handler(**block.input) if handler else f"Unknown tool: {block.name}"
                except Exception as e:
                    output = f"Error: {e}"
                print(f"> {block.name}: {str(output)[:200]}")
                results.append({"type": "tool_result", "tool_use_id": block.id, "content": str(output)})
        messages.append({"role": "user", "content": results})
```

---

### s09：Agent Teams（协作面：持久队友 + JSONL 邮箱）
核心架构（队友生命周期 + 邮箱通信）：
```text
team/
  config.json       # roster + status
  inbox/alice.jsonl # append-only -> drain-on-read
每个队友：每轮前 read_inbox(name) -> 注入 messages -> 再跑 LLM
```
关键代码（send 追加 JSONL；read_inbox drain）：
```python
def send(self, sender, to, content, msg_type="message", extra=None):
    msg = {"type": msg_type, "from": sender, "content": content,
           "timestamp": time.time()}
    if extra:
        msg.update(extra)
    with open(self.dir / f"{to}.jsonl", "a") as f:
        f.write(json.dumps(msg) + "\n")

def read_inbox(self, name):
    path = self.dir / f"{name}.jsonl"
    if not path.exists(): return "[]"
    msgs = [json.loads(l) for l in path.read_text().strip().splitlines() if l]
    path.write_text("")  # drain
    return json.dumps(msgs, indent=2)
```

---

### s10：Team Protocols（协议化协商：request_id + FSM）
核心架构（两个用途复用同一握手模式）：
```text
Lead -> request_id 发起请求
Teammate -> 引用同 request_id 返回 approve/reject
统一 FSM：pending -> approved | rejected
```
关键代码（shutdown_request 示例）：
```python
def handle_shutdown_request(teammate: str) -> str:
    req_id = str(uuid.uuid4())[:8]
    shutdown_requests[req_id] = {"target": teammate, "status": "pending"}
    BUS.send("lead", teammate, "Please shut down.", "shutdown_request", {"request_id": req_id})
    return f"Shutdown request {req_id} sent to '{teammate}'"

def handle_plan_review(request_id: str, approve: bool, feedback: str = "") -> str:
    req = plan_requests.get(request_id)
    if not req: return f"Error: Unknown plan request_id '{request_id}'"
    req["status"] = "approved" if approve else "rejected"
    BUS.send("lead", req["from"], feedback, "plan_approval_response",
             {"request_id": request_id, "approve": approve, "feedback": feedback})
    return f"Plan {req['status']} for '{req['from']}'"
```

---

### s11：Autonomous Agents（自治：IDLE 轮询 + claim + 身份重注入）
核心架构（WORK/IDLE 两阶段）：
```text
WORK：LLM 工作阶段（tool_use 停止/或 idle 工具触发）
IDLE：每 5s 检查 inbox + 扫描未认领 tasks -> claim -> 返回 WORK
压缩后消息过短：插入 identity block，避免忘记身份
```
关键代码（扫描未认领任务）：
```python
def scan_unclaimed_tasks() -> list:
    unclaimed = []
    for f in sorted(TASKS_DIR.glob("task_*.json")):
        task = json.loads(f.read_text())
        if (task.get("status") == "pending"
                and not task.get("owner")
                and not task.get("blockedBy")):
            unclaimed.append(task)
    return unclaimed
```
关键代码（身份重注入）：
```python
if len(messages) <= 3:
    messages.insert(0, {"role": "user",
        "content": f"<identity>You are '{name}', role: {role}, "
                   f"team: {team_name}. Continue your work.</identity>"})
    messages.insert(1, {"role": "assistant", "content": f"I am {name}. Continuing."})
```

---

### s12：Worktree + Task Isolation（执行隔离：每任务独立目录 + 绑定关系）
核心架构（控制面 .tasks / 执行面 .worktrees）：
```text
.tasks/task_1.json            .worktrees/auth-refactor/
  status / worktree <------>   branch: wt/auth-refactor
  task_id: 1                  task_id: 1
```
关键代码（创建 worktree 并绑定 task_id）：
```python
def create(self, name: str, task_id: int = None, base_ref: str = "HEAD") -> str:
    self._validate_name(name)
    if self._find(name):
        raise ValueError(f"Worktree '{name}' already exists in index")
    if task_id is not None and not self.tasks.exists(task_id):
        raise ValueError(f"Task {task_id} not found")

    path = self.dir / name
    branch = f"wt/{name}"

    self.events.emit(
        "worktree.create.before",
        task={"id": task_id} if task_id is not None else {},
        worktree={"name": name, "base_ref": base_ref},
    )
    try:
        self._run_git(["worktree", "add", "-b", branch, str(path), base_ref])

        entry = {
            "name": name,
            "path": str(path),
            "branch": branch,
            "task_id": task_id,
            "status": "active",
            "created_at": time.time(),
        }

        idx = self._load_index()
        idx["worktrees"].append(entry)
        self._save_index(idx)

        if task_id is not None:
            self.tasks.bind_worktree(task_id, name)

        self.events.emit(
            "worktree.create.after",
            task={"id": task_id} if task_id is not None else {},
            worktree={
                "name": name,
                "path": str(path),
                "branch": branch,
                "status": "active",
            },
        )
        return json.dumps(entry, indent=2)
    except Exception as e:
        self.events.emit(
            "worktree.create.failed",
            task={"id": task_id} if task_id is not None else {},
            worktree={"name": name, "base_ref": base_ref},
            error=str(e),
        )
        raise
```
关键代码（在隔离目录执行：cwd=worktree path）：
```python
def run(self, name: str, command: str) -> str:
    wt = self._find(name)
    path = Path(wt["path"])
    r = subprocess.run(
        command,
        shell=True,
        cwd=path,
        capture_output=True,
        text=True,
        timeout=300,
    )
```
关键代码（remove + complete_task：拆除 + 标记任务完成）：
```python
def remove(self, name: str, force: bool = False, complete_task: bool = False) -> str:
    wt = self._find(name)
    if not wt:
        return f"Error: Unknown worktree '{name}'"

    self.events.emit(
        "worktree.remove.before",
        task={"id": wt.get("task_id")} if wt.get("task_id") is not None else {},
        worktree={"name": name, "path": wt.get("path")},
    )
    try:
        args = ["worktree", "remove"]
        if force:
            args.append("--force")
        args.append(wt["path"])
        self._run_git(args)

        if complete_task and wt.get("task_id") is not None:
            task_id = wt["task_id"]
            before = json.loads(self.tasks.get(task_id))
            self.tasks.update(task_id, status="completed")
            self.tasks.unbind_worktree(task_id)
            self.events.emit(
                "task.completed",
                task={
                    "id": task_id,
                    "subject": before.get("subject", ""),
                    "status": "completed",
                },
                worktree={"name": name},
            )

        idx = self._load_index()
        for item in idx.get("worktrees", []):
            if item.get("name") == name:
                item["status"] = "removed"
                item["removed_at"] = time.time()
        self._save_index(idx)

        self.events.emit(
            "worktree.remove.after",
            task={"id": wt.get("task_id")} if wt.get("task_id") is not None else {},
            worktree={"name": name, "path": wt.get("path"), "status": "removed"},
        )
        return f"Removed worktree '{name}'"
    except Exception as e:
        self.events.emit(
            "worktree.remove.failed",
            task={"id": wt.get("task_id")} if wt.get("task_id") is not None else {},
            worktree={"name": name, "path": wt.get("path")},
            error=str(e),
        )
        raise
```

---

### s_full：完整合体 cockpit（工程化拼装示意）
核心架构图（教学版注释里的“cockpit”）：
```text
System prompt (skills, task-first + todo nag)
Before each LLM call:
  microcompact / auto_compact
  drain background notifications
  check inbox
LLM call (tools)
Tool dispatch handlers
  subagent spawn (s04)
  background (s08)
  teams + protocols (s09/s10/s11)
```
关键代码（s_full 的工具分发与每轮管线）：
```python
SYSTEM = f"""You are a coding agent at {WORKDIR}. Use tools to solve tasks.
Prefer task_create/task_update/task_list for multi-step work. Use TodoWrite for short checklists.
Use task for subagent delegation. Use load_skill for specialized knowledge.
Skills: {SKILLS.descriptions()}"""

TOOL_HANDLERS = {
    "bash":             lambda **kw: run_bash(kw["command"]),
    "read_file":        lambda **kw: run_read(kw["path"], kw.get("limit")),
    "write_file":       lambda **kw: run_write(kw["path"], kw["content"]),
    "edit_file":        lambda **kw: run_edit(kw["path"], kw["old_text"], kw["new_text"]),
    "TodoWrite":        lambda **kw: TODO.update(kw["items"]),
    "task":             lambda **kw: run_subagent(kw["prompt"], kw.get("agent_type", "Explore")),
    "load_skill":       lambda **kw: SKILLS.load(kw["name"]),
    "compress":         lambda **kw: "Compressing...",
    "background_run":   lambda **kw: BG.run(kw["command"], kw.get("timeout", 120)),
    "check_background": lambda **kw: BG.check(kw.get("task_id")),
    "task_create":      lambda **kw: TASK_MGR.create(kw["subject"], kw.get("description", "")),
    "task_get":         lambda **kw: TASK_MGR.get(kw["task_id"]),
    "task_update":      lambda **kw: TASK_MGR.update(kw["task_id"], kw.get("status"), kw.get("add_blocked_by"), kw.get("add_blocks")),
    "task_list":        lambda **kw: TASK_MGR.list_all(),
    "spawn_teammate":   lambda **kw: TEAM.spawn(kw["name"], kw["role"], kw["prompt"]),
    "list_teammates":   lambda **kw: TEAM.list_all(),
    "send_message":     lambda **kw: BUS.send("lead", kw["to"], kw["content"], kw.get("msg_type", "message")),
    "read_inbox":       lambda **kw: json.dumps(BUS.read_inbox("lead"), indent=2),
    "broadcast":        lambda **kw: BUS.broadcast("lead", kw["content"], TEAM.member_names()),
    "shutdown_request": lambda **kw: handle_shutdown_request(kw["teammate"]),
    "plan_approval":    lambda **kw: handle_plan_review(kw["request_id"], kw["approve"], kw.get("feedback", "")),
    "idle":             lambda **kw: "Lead does not idle.",
    "claim_task":       lambda **kw: TASK_MGR.claim(kw["task_id"], "lead"),
}

def agent_loop(messages: list):
    rounds_without_todo = 0
    while True:
        microcompact(messages)
        if estimate_tokens(messages) > TOKEN_THRESHOLD:
            messages[:] = auto_compact(messages)
        notifs = BG.drain()
        if notifs:
            txt = "\n".join(f"[bg:{n['task_id']}] {n['status']}: {n['result']}" for n in notifs)
            messages.append({"role": "user", "content": f"<background-results>\n{txt}\n</background-results>"})
            messages.append({"role": "assistant", "content": "Noted background results."})
        inbox = BUS.read_inbox("lead")
        if inbox:
            messages.append({"role": "user", "content": f"<inbox>{json.dumps(inbox, indent=2)}</inbox>"})
            messages.append({"role": "assistant", "content": "Noted inbox messages."})

        response = client.messages.create(
            model=MODEL, system=SYSTEM, messages=messages,
            tools=TOOLS, max_tokens=8000,
        )

        messages.append({"role": "assistant", "content": response.content})
        if response.stop_reason != "tool_use":
            return

        results = []
        used_todo = False
        manual_compress = False
        for block in response.content:
            if block.type == "tool_use":
                if block.name == "compress":
                    manual_compress = True
                handler = TOOL_HANDLERS.get(block.name)
                try:
                    output = handler(**block.input) if handler else f"Unknown tool: {block.name}"
                except Exception as e:
                    output = f"Error: {e}"
                print(f"> {block.name}: {str(output)[:200]}")
                results.append(
                    {"type": "tool_result", "tool_use_id": block.id, "content": str(output)}
                )
                if block.name == "TodoWrite":
                    used_todo = True

        # s03: nag reminder (only when todo workflow is active)
        rounds_without_todo = 0 if used_todo else rounds_without_todo + 1
        if TODO.has_open_items() and rounds_without_todo >= 3:
            results.insert(0, {"type": "text", "text": "<reminder>Update your todos.</reminder>"})
        messages.append({"role": "user", "content": results})

        # s06: manual compress
        if manual_compress:
            print("[manual compact]")
            messages[:] = auto_compact(messages)
```


