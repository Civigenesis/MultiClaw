# MultiClaw / Civigenesis 落地方案（完整版）

> 整合《执行方案 2.0》与《执行方案 3.0》，形成可直接用于实施的一体化落地方案。**覆盖多实例、单实例多实体、技能、Agent 主动性、对话/群聊、通信、可观测、记忆、故障恢复等全部设计，按阶段划分交付物与里程碑。**

---

## 一、文档定位与目标

### 1.1 定位

- **落地方案**：面向实施的一体化设计文档，供开发、测试、运维按阶段执行。
- **范围**：从 Zeroclaw 现状出发，到 MultiClaw 完整能力的实现路径；不含具体代码，但明确模块、接口、数据流与阶段边界。
- **前置文档**：《概念设计 2.0》《执行方案 2.0》《执行方案 3.0》。

### 1.2 目标能力一览

| 能力域 | 核心内容 |
|--------|----------|
| **实例管理** | 管理员实例 + 多普通实例；集群根、实例注册表、端口分配、启动机制 |
| **单实例多实体** | Entity、CEO、EntityPool；混合 Provider 配置；五类预设组织 |
| **技能** | OpenClaw 兼容、ClawHub 自主安装、作用域（实例/全局）、平台扫描 + CEO 评估 |
| **主动性** | CEO 周期、实体任务/巡逻、ProactiveScheduler |
| **对话/群聊** | task / conversation / broadcast；1:1、群聊、频道、随机社交 |
| **通信** | 汇报、资源申请；管理员→CEO；实例→全局共享（管理员审批） |
| **可观测** | 五层视图、事件采集、Web 仪表盘、甘特图/看板 |
| **记忆** | 作用域、压缩、长期存储 |
| **故障恢复** | 业务级检查点、运行中记忆/任务/对话恢复、无缝衔接 |

---

## 二、Zeroclaw 现状与改造基线

### 2.1 现状摘要

| 模块 | 当前行为 |
|------|----------|
| Config | 单份 config.toml；无 instance_id |
| Onboard | 写单份 config；无 instances/、instances.json |
| main | 无 --instance；Config::load_or_init() 加载单 config |
| Daemon | 单 config 克隆给 gateway、channels、heartbeat、scheduler |
| Gateway | 单 config，无按实例路由 |
| Agent | 被动；单次 turn 或 interactive stdin |
| Cron | JobType::Shell / Agent；单 config |
| Skills | workspace/skills/<name>/；SKILL.toml 或 SKILL.md；无实例/实体作用域 |
| Memory | 单 backend；无 scope 划分 |
| Service | 启动 multiclaw daemon（单实例） |

### 2.2 向后兼容约定

- 无 `instances.json` 且无 `instances/` 目录 → **单实例模式**：config.toml、workspace 直接在集群根；行为与当前 Multiclaw 一致。
- 有 `instances/` 与 `instances.json` → **集群模式**：多实例、管理员、注册表生效。

---

## 三、基础设施与目录设计

### 3.1 集群根与目录布局

```
<cluster_root>/                      # ~/.multiclaw 或 MULTICLAW_CLUSTER_ROOT
├── instances.json                   # 实例注册表
├── global/                          # 全局共享（3.0 新增）
│   └── knowledge/                   # 审批通过的全局知识
├── shared/                          # 全局技能（可选）
│   └── skills/<name>/
├── instances/
│   ├── admin/                       # 管理员实例
│   │   ├── config.toml
│   │   ├── daemon_state.json
│   │   ├── checkpoints/             # 业务级检查点（3.0）
│   │   └── workspace/
│   │       ├── sessions/ memory/ state/ cron/ skills/
│   └── <instance_id>/               # 普通实例（单实例多实体）
│       ├── config.toml
│       ├── daemon_state.json
│       ├── checkpoints/
│       └── workspace/
│           ├── sessions/ memory/ state/ cron/ skills/   # 实例级（或 ceo 使用）
│           ├── entities/            # 各实体独立 workspace（阶段 2）
│           │   └── <entity_id>/
│           │       ├── memory/ state/ sessions/ skills/
│           └── teams/               # 团队共享目录（可选）
│               └── <team_id>/
```

### 3.2 实例注册表结构

| 字段 | 类型 | 说明 |
|------|------|------|
| version | u32 | 格式版本 |
| instances[].id | string | 实例 id |
| instances[].role | enum | admin \| normal |
| instances[].status | enum | created \| running \| stopped \| deleted |
| instances[].config_path | string | config.toml 路径 |
| instances[].workspace_path | string | workspace 路径 |
| instances[].gateway_port | u16 | Gateway 端口 |
| instances[].created_at | string | ISO8601 |
| instances[].constraints | object | agent_max、call_quota 等 |
| instances[].preset | string | startup \| enterprise \| brainstorm \| freeform \| project（3.0） |

### 3.3 端口分配

- 管理员：config.gateway.port（默认 42617）
- 普通实例：端口池 42618，创建时分配，写入注册表

### 3.4 启动与 Config 加载

- CLI：`--instance <id>`；无则根据 MULTICLAW_INSTANCE 或单实例模式
- Config 路径：`cluster_root/instances/<id>/config.toml`
- Daemon：`multiclaw daemon --instance admin` 或 `--instance <id>`
- Service：ExecStart 为 `multiclaw daemon --instance admin`

---

## 四、单实例多实体设计

### 4.1 实体与 CEO 配置

- **实例段**：`[instance]` 含 `preset`、`default_provider`、`default_model`、`ceo`、`entities`、`teams`、`projects`（按 preset）
- **Provider 混合策略**（3.0）：实例级默认 + 实体可覆盖 `provider`、`model`；API Key 统一存 config
- **CEO**：`entity_id=ceo`；工具：create_team、create_entity、assign_task、instance_status 等；无 instances_list 等实例管理能力
- **约束校验**：create_entity 时检查 constraints.agent_max

### 4.2 五类预设组织（3.0）

| preset | 结构 | 通信特点 | 配置要点 |
|--------|------|----------|----------|
| startup | CEO 直管 | 任务直接分配；可选 1:1/群聊 | 无 teams |
| enterprise | CEO→TeamLeader→Member | 任务可分配 Team；Team 内群聊 | teams、role、team_id |
| brainstorm | 讨论组 | conversation 为主；任务后验提炼 | 无层级；conversation 权重高 |
| freeform | 无预设；可有机组队 | 随机社交；1:1/群聊自由 | random_social、allow_organic_teams |
| project | 项目为中心 | 任务绑定 project；项目看板 | projects、project_ids |

### 4.3 实体/团队 workspace 管理（阶段 2）

- **实体独立 workspace**：每个实体在实例下拥有独立目录 `instances/<id>/workspace/entities/<entity_id>/`，可含 `memory/`、`state/`、`sessions/`、`skills/` 等，用于隔离该实体的记忆、状态与产出。
- **团队目录**：`workspace/teams/<team_id>/` 用于团队共享；create_team 持久化到 config 后创建该目录。
- **create_team / create_entity**：必须写入 config.toml（追加 `[instance.teams]` / `[[instance.entities]]`），并创建对应 workspace 子目录；不得仅为占位实现。

### 4.4 每实体的个性化与描述文件（独立决策）

- **目标**：每个实体（admin/CEO/工作节点）除在 config 中注册外，拥有自己的**描述文件**（如 IDENTITY.md、SOUL.md、AGENTS.md、可选 agent.md），用于构建 system prompt，实现独立身份与决策风格。
- **当前实现**：实例级 workspace 下有 OpenClaw 风格文件 + config `[identity]`（AIEOS 等），在 `build_system_prompt` 时从单一 `workspace_dir` 注入。
- **扩展**：以某实体身份运行时，**prompt 根目录** = `workspace/entities/<entity_id>/`；从该目录加载 SOUL.md、IDENTITY.md 等；缺失时可回退实例级。**实体创建时**在其实体 workspace 内执行 scaffold（默认 IDENTITY/SOUL/AGENTS），**实例创建时**可对实例级 workspace 做最小 scaffold。

### 4.5 记忆与技能作用域

- **记忆**：key 前缀 `entity:<id>:`、`team:<id>:`、`instance:`；实体专属数据落在 `workspace/entities/<entity_id>/`。
- **技能**：实例级 `workspace/skills/`；实体级 `workspace/entities/<entity_id>/skills/` 或白名单；全局 `shared/skills/` 需管理员审批

---

## 五、技能与 ClawHub

### 5.1 加载路径与优先级

| 来源 | 路径 | 优先级 |
|------|------|--------|
| 实例 workspace | instances/<id>/workspace/skills/<name>/ | 1 |
| open-skills | open_skills_dir | 2 |
| 全局 | cluster_root/shared/skills/<name>/ | 3 |

### 5.2 ClawHub 自主安装（3.0）

- **工具**：clawhub_search、clawhub_browse、clawhub_install
- **作用域**：`instance` 直接安装；`global` 需管理员审批
- **安全扫描**：平台强制扫描（路径穿越、危险命令等）；CEO 可选业务风险评估
- **管理员工具**：skill_approve_global、skill_reject_global

### 5.3 OpenClaw 兼容与迁移

- 格式：SKILL.toml 优先，否则 SKILL.md
- 迁移：`multiclaw migrate openclaw --source ~/.openclaw`

---

## 六、Agent 主动性与对话/群聊

### 6.1 CEO 主动循环

- 定时：ceo_interval_secs（默认 300）
- 事件：实体汇报触发
- 与 Cron：JobType::Agent target=ceo

### 6.2 实体主动循环

- 任务驱动：收件箱取任务执行
- 巡逻：idle_interval_secs 自检/巡逻 prompt
- 调度：ProactiveScheduler 协调 CEO、实体、Cron、Heartbeat

### 6.3 对话与群聊（3.0）

- **消息类型**：task、conversation、broadcast
- **通道**：1:1、群聊（Group）、频道（Channel）、随机社交
- **ConversationBus**：create_conversation、invite、post、leave
- **配置**：conversation.enabled、initiate_interval_secs、random_social_interval_secs、max_participants
- **调度**：poll_idle 时检查被邀请对话、可发起随机社交

### 6.4 主动性配置

| 配置项 | 默认 |
|--------|------|
| proactive.ceo_interval_secs | 300 |
| proactive.entity_idle_interval_secs | 60 |
| proactive.ceo_enabled | true |
| proactive.entity_autonomous | true |
| proactive.max_concurrent_turns | 3 |

---

## 七、通信机制

### 7.1 实例内通信

- **MessageBus**：TaskMessage；inbox_send、inbox_recv
- **ConversationBus**：conversation、broadcast
- **路由**：to=entity_id | team:id | ceo

### 7.2 实例→管理员

| 类型 | 用途 | 端点/传输 |
|------|------|-----------|
| report (progress/alert) | 汇报 | POST /api/report |
| resource_request | 资源申请（3.0） | 同汇报通道 |

### 7.3 管理员→实例（3.0）

- **AdminToCeoMessage**：instruction、approval、rejection、info
- **传输**：POST /api/admin-message 或共享存储

### 7.4 实例→全局（3.0）

- **GlobalShareRequest**：info、discussion、knowledge；target=all_instances|specific
- **流程**：request_global_share → 管理员审批 → 写入 global/knowledge/
- **审批**：approve_global_share、reject_global_share

---

## 八、可观测性与 Web

### 8.1 架构

```
Data Sources → Aggregation → Store → API → Web UI
```

### 8.2 各角色视图与数据

| 角色 | 主视图 | 数据 | 图表 |
|------|--------|------|------|
| 用户/管理员 | 全局仪表盘 | 资源、实例列表、待审批、汇报 | 资源图、实例卡片、审批列表、汇报时间线 |
| CEO | 实例运营台 | 资源、团队、任务、实体 | 甘特图、团队看板、实体表 |
| 团队 | 任务看板 | 本团队任务、进度、阻塞 | 看板、甘特图、燃尽图 |
| 实体 | 执行台 | 当前任务、执行记录 | 任务详情、执行时间线 |

### 8.3 Web 页面

| 路由 | 内容 |
|------|------|
| /login | 认证 |
| /dashboard | 用户/管理员仪表盘 |
| /instances/:id | 实例详情 |
| /instances/:id/ceo | CEO 视图 |
| /instances/:id/teams/:tid | 团队看板 |
| /instances/:id/entities/:eid | 实体视图 |
| /approvals | 审批中心 |

### 8.4 事件与存储

- **事件**：task_created、task_assigned、task_completed、turn_start、turn_end、tool_call、report、approval_request、health_update
- **维度**：instance_id、entity_id、team_id、timestamp
- **保留**：7 天原始、30 天聚合（可配置）

---

## 九、记忆与压缩

### 9.1 作用域

- Key 前缀：entity:、team:、instance:
- 访问控制：读/写时校验 scope

### 9.2 压缩

- 触发：history 长度/token 超过阈值（如 max_history_messages 的 80%）
- 流程：选取历史 → LLM 摘要 → 写入 category=compressed
- 检索：按需查 compressed 与 scope

---

## 十、故障恢复

### 10.1 业务级检查点（3.0 扩展）

**持久化内容**：

| 状态 | 说明 |
|------|------|
| 任务队列 | 待分配、进行中、阻塞 |
| 任务执行进度 | 当前 turn、工具调用中、部分结果 |
| 收件箱 | 未读消息 |
| 对话缓冲区 | 进行中的 1:1/群聊 |
| 记忆上下文 | 当前加载摘要 |
| 实体运行态 | 当前 prompt、history 快照 |
| CEO 循环状态 | 上次分析时间、待处理决策 |

### 10.2 检查点策略

- **全量**：周期可配置（如 5 分钟）
- **增量**：自上次全量后的变更
- **关键点**：任务完成、对话轮次结束

### 10.3 恢复流程

1. 启动时检测 checkpoint
2. 加载最近全量 + 增量
3. 恢复 EntityPool、MessageBus、ConversationBus、任务队列、收件箱
4. 恢复实体当前 turn（工具执行中可重试或跳过）
5. 恢复 CEO 循环、Cron、Heartbeat
6. 继续运行；记录恢复事件

### 10.4 恢复策略

- 重启：instance_restart
- 回滚：加载检查点
- 重建：instance_create 重建

---

## 十一、资源与配额

### 11.1 校验点

| 时机 | 动作 |
|------|------|
| instance_create | 全局实例数、端口池 |
| create_entity | constraints.agent_max |
| 每轮 turn | call_quota、cost_per_day |
| 工具执行 | max_actions_per_hour |

### 11.2 超限

- 拒绝操作；可选汇报 resource_request
- 主动循环暂停或降频

---

## 十二、阶段与交付物

### 12.1 阶段总览

| 阶段 | 目标 | 关键交付 |
|------|------|----------|
| **0** | 引导与管理员实例 | 集群根、instances/admin、instances.json、--instance、Onboard 创建 admin |
| **1** | 实例抽象与管理 | 实例 CRUD、端口分配、管理员工具、汇报通道、管理员→CEO 消息 |
| **2** | 单实例多实体 | Entity、CEO、EntityPool、混合 Provider、五类 preset、约束校验 |
| **2.5** | 技能与 ClawHub | 作用域、ClawHub 工具、平台扫描、审批流程 |
| **3** | 主动性与对话 | ProactiveScheduler、CEO/实体循环、ConversationBus、随机社交 |
| **4** | 通信扩展 | 资源申请、全局共享、审批 API |
| **5** | 可观测与 Web | 事件采集、聚合、API、Web 仪表盘、甘特图/看板 |
| **6** | 业务级恢复 | 扩展检查点、状态恢复、无缝衔接 |

### 12.2 模块与阶段对照

| 模块 | 0 | 1 | 2 | 2.5 | 3 | 4 | 5 | 6 |
|------|---|---|---|-----|---|---|---|---|
| main.rs | --instance | | | | | | | |
| config/schema | instance 解析 | 约束 | entities、ceo、preset、Provider | | proactive、conversation | | | |
| Config::load_or_init | 按 instance 解析 | | | | | | | |
| onboard | admin 与注册表 | | | | | | | |
| instance_registry | 读写 | 管理能力 | | | | | 健康 | |
| instance_manager | | CRUD、端口 | | | | | 编排 | |
| daemon | | | | | ProactiveScheduler | | | |
| agent | | | EntityRuntime | | 主动 turn | | | |
| skills | | | | ClawHub、扫描、审批 | | | | |
| memory | | | scope 设计 | | | | 作用域实现 | |
| message/conversation | | | | | MessageBus、ConversationBus | | | |
| gateway | | 汇报 | | | | admin→CEO、global_share | | |
| cron | | | | | target: ceo/entity | | | |
| observability | | | | | | | 事件、API、Web | |
| checkpoint | | | | | | | | 业务状态 |

### 12.3 里程碑

| 里程碑 | 条件 |
|--------|------|
| M0 | 阶段 0 完成；multiclaw daemon --instance admin 可启动 |
| M1 | 阶段 1 完成；管理员可 CRUD 实例；汇报与 admin→CEO 可用 |
| M2 | 阶段 2 完成；单实例多实体、CEO、五类 preset 可用 |
| M2.5 | 阶段 2.5 完成；ClawHub 安装、平台扫描、审批生效 |
| M3 | 阶段 3 完成；CEO/实体主动循环、对话/群聊可用 |
| M4 | 阶段 4 完成；资源申请、全局共享与审批闭环 |
| M5 | 阶段 5 完成；Web 仪表盘、甘特图/看板、审批中心可用 |
| M6 | 阶段 6 完成；业务级检查点与恢复，重启无缝衔接 |

---

## 十三、实施建议

### 13.1 依赖顺序

- 0 → 1 → 2 为基线；2.5、3 可与 2 部分并行；4 依赖 1；5 依赖 2、3；6 依赖 3、5。
- 建议优先路径：0 → 1 → 2 → 3 → 4 → 2.5 → 5 → 6。

### 13.2 风险与缓解

| 风险 | 缓解 |
|------|------|
| 检查点与 Memory 一致性 | checkpoint 记录引用；恢复时校验 |
| 对话/随机社交成本 | 可配置间隔；超限时暂停 |
| 平台扫描漏检 | 沙箱+静态分析；CEO 二次确认 |
| 多实例编排 | 推荐外部编排器；文档提供 systemd/K8s 示例 |

### 13.3 可讨论点

- 技能扫描：建议平台强制扫描 + CEO 可选评估；若仅 CEO 扫描，需接受误判风险。
- 全局共享存储：可先 `cluster_root/global/` 文件；后续扩展数据库。
- Web 技术栈：可复用 Multiclaw 现有 web；图表可用 ECharts、Gantt 库。

---

*文档版本：落地方案（完整版）*  
*整合自：《执行方案 2.0》《执行方案 3.0》*  
*前置：《概念设计 2.0》*  
*适用范围：MultiClaw / Civigenesis 整体优化方案*
