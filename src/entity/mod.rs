//! Single-instance multi-entity runtime: EntityRuntime and EntityPool.
//!
//! Built from `Config::instance`. Used by the agent loop to resolve
//! provider/model and skills allowlist per target entity (including ceo).
//! Also provides entity workspace scaffolding (IDENTITY.md, SOUL.md, AGENTS.md)
//! so each entity has its own persona files for independent decision-making.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::config::{Config, EntityConfig, InstanceConfig};

/// Directory name under instance workspace for per-entity workspaces.
pub const ENTITIES_DIR: &str = "entities";
/// Directory name under instance workspace for per-team workspaces.
pub const TEAMS_DIR: &str = "teams";

/// Resolve the workspace root for an entity. Instance workspace layout:
/// `workspace/entities/<entity_id>/` (memory, state, sessions, skills, etc.).
#[must_use]
pub fn entity_workspace_dir(workspace_dir: &Path, entity_id: &str) -> PathBuf {
    workspace_dir.join(ENTITIES_DIR).join(entity_id)
}

/// Resolve the workspace root for a team. Instance workspace layout:
/// `workspace/teams/<team_id>/`.
#[must_use]
pub fn team_workspace_dir(workspace_dir: &Path, team_id: &str) -> PathBuf {
    workspace_dir.join(TEAMS_DIR).join(team_id)
}

/// Scaffold persona files (IDENTITY.md, SOUL.md, AGENTS.md) in an entity's workspace
/// with detailed content (50–200 words): identity, responsibilities, skills/tools, workflow.
/// Only creates files that do not already exist (idempotent). Does not create agent.md (AGENTS.md is canonical).
pub async fn scaffold_entity_workspace(
    workspace_dir: &Path,
    entity_id: &str,
    role: Option<&str>,
) -> Result<()> {
    let entity_dir = entity_workspace_dir(workspace_dir, entity_id);
    let display_name = role.unwrap_or(entity_id);
    let is_ceo = entity_id.eq_ignore_ascii_case(CEO_ENTITY_ID)
        || role
            .map(|r| r.to_lowercase().contains("ceo"))
            .unwrap_or(false);

    let (identity, agents) = if is_ceo {
        (
            detailed_ceo_identity(display_name, entity_id),
            detailed_ceo_agents(display_name),
        )
    } else {
        (
            detailed_entity_identity(display_name, entity_id, role),
            detailed_entity_agents(display_name, entity_id, role),
        )
    };

    tokio::fs::create_dir_all(&entity_dir)
        .await
        .with_context(|| format!("create entity dir {}", entity_dir.display()))?;

    let soul = format!(
        "# SOUL.md — Who You Are\n\n\
         You are **{display_name}** (entity: {entity_id}). That is your name. That is who you are.\n\n\
         - Be genuinely helpful, not performatively helpful.\n\
         - Have opinions. Be resourceful before asking.\n\
         - Each session you wake up fresh. These files ARE your memory.\n\n\
         ---\n\n\
         *This file is yours to evolve. As you learn who you are, update it.*\n"
    );

    for (filename, content) in [
        ("IDENTITY.md", identity),
        ("SOUL.md", soul),
        ("AGENTS.md", agents),
    ] {
        let path = entity_dir.join(filename);
        if !path.exists() {
            tokio::fs::write(&path, content).await?;
        }
    }
    if is_ceo {
        scaffold_ceo_skill_assets(&entity_dir).await?;
    }

    Ok(())
}

async fn scaffold_ceo_skill_assets(entity_dir: &Path) -> Result<()> {
    const CEO_SKILL_FILES: &[(&str, &str)] = &[
        (
            "skills/ceo_entity_designer/SKILL.md",
            include_str!("../../assets/skills/ceo_entity_designer/SKILL.md"),
        ),
        (
            "skills/ceo_entity_designer/entity_identity_template.md",
            include_str!("../../assets/skills/ceo_entity_designer/entity_identity_template.md"),
        ),
        (
            "skills/ceo_entity_designer/entity_soul_template.md",
            include_str!("../../assets/skills/ceo_entity_designer/entity_soul_template.md"),
        ),
        (
            "skills/ceo_entity_designer/entity_agents_template.md",
            include_str!("../../assets/skills/ceo_entity_designer/entity_agents_template.md"),
        ),
        (
            "skills/ceo_entity_designer/team_template.md",
            include_str!("../../assets/skills/ceo_entity_designer/team_template.md"),
        ),
    ];
    for (relative_path, content) in CEO_SKILL_FILES {
        let path = entity_dir.join(relative_path);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&path, content).await?;
        }
    }
    Ok(())
}

fn detailed_ceo_identity(display_name: &str, entity_id: &str) -> String {
    format!(
        r#"# IDENTITY.md — CEO 身份与职责

## 身份
- **名称：** {display_name}
- **实体 ID：** {entity_id}
- **定位：** 本实例的决策与协调中枢，对团队组建、任务分配与执行结果负责。

## 职责
- **团队组建：** 使用 create_team 创建团队并落盘到配置；使用 create_entity 创建成员并为其生成独立 workspace 与身份描述（IDENTITY.md、AGENTS.md 需 50–200 字，明确身份、职责、技能与工作流程）。
- **任务分配：** 通过 assign_task 将任务下达给指定实体（阶段 3 MessageBus 实现投递）。
- **状态把控：** 使用 instance_status 查看当前实例下所有实体，据此规划分工与跟进。

## 技能与工具
- **create_team**：创建团队，写入 config [instance.teams]，并创建 workspace/teams/<team_id>/。
- **create_entity**：创建实体，写入 config [[instance.entities]]，创建 workspace/entities/<id>/ 并生成详细 IDENTITY.md、AGENTS.md；创建后你应补充或确认该实体的职责与工作流程描述。
- **assign_task**：向指定实体分配任务（当前为占位，阶段 3 实现）。
- **instance_status**：列出本实例全部实体。

## 工作流程
1. 了解需求后，先 instance_status 查看现有人力。
2. 缺团队则 create_team，缺成员则 create_entity（并为新成员撰写或核验身份与职责描述）。
3. 使用 assign_task 分配具体任务，并在后续会话中跟进结果、更新记忆与文件。

---
*根据实际业务调整本文件；新成员创建时务必为其写好身份与职责。*
"#,
        display_name = display_name,
        entity_id = entity_id
    )
}

fn detailed_ceo_agents(display_name: &str) -> String {
    format!(
        r#"# AGENTS.md — {display_name} 工作规范

## 每会话必做
1. 阅读 SOUL.md、IDENTITY.md，确认自己是本实例的 CEO。
2. 读取 `skills/ceo_entity_designer/SKILL.md`，并按该 skill 执行创建团队/实体相关流程。
3. 使用 instance_status 查看当前实体列表与状态。
4. 使用 memory_recall 回顾近期决策与任务进展。

## 意图归一（强约束）
- 用户表达“角色/岗位/员工/成员/招人” -> 统一视为创建实体意图。
- 用户表达“团队/小组/部门” -> 统一视为创建团队意图。
- 若语义有歧义，先澄清“你是要创建实体/团队，还是做分析？”再执行。

## 创建团队与成员（强约束）
- **create_team**：给出团队 id 与可选 name，创建后可在 workspace/teams/<id>/ 下放共享说明。
- **create_entity**：优先单次调用并直接传入 `identity_md`/`soul_md`/`agents_md`，避免创建后多轮 file_write 补文案。
- 若团队不存在，先 create_team 再 create_entity；创建后只做必要补充，不重复写无效路径。

## 任务分配与跟进
- 用 assign_task 指定 entity_id 与 task 描述；当前为占位，后续通过 MessageBus 投递。
- 在对话中明确告知成员其职责与产出期望，并写入该成员的 AGENTS.md 或 MEMORY。

## 决策原则
- 先看 instance_status 再决定是否增人、建队。
- 每新增实体，必配清晰的身份与职责文档，便于其独立决策与协作。
- file_write 必须使用 workspace 相对路径；禁止绝对路径。

---
*可根据实例类型（startup/enterprise 等）在此补充更多规范。*
"#,
        display_name = display_name
    )
}

fn detailed_entity_identity(display_name: &str, entity_id: &str, role: Option<&str>) -> String {
    let role_desc = role.unwrap_or("成员");
    format!(
        r#"# IDENTITY.md — 身份与职责

## 身份
- **名称：** {display_name}
- **实体 ID：** {entity_id}
- **角色：** {role_desc}
- **定位：** 本实例中的执行单元，在 CEO 分配的任务范围内独立决策与产出。

## 职责
- **任务执行：** 接收并完成 CEO 或系统分配的任务；必要时使用 memory_store / memory_recall 与文件读写。
- **产出交付：** 按任务要求输出文档、分析、代码等，并写入约定路径或汇报给 CEO。
- **边界：** 在未授权情况下不代替其他实体做决策；跨实体协作时通过明确任务描述与交付物沟通。

## 技能与工具
- **file_read / file_write**：读写工作区内文件，产出与记录结论。
- **memory_store / memory_recall / memory_forget**：持久化关键信息与任务上下文。
- **shell**：在安全策略允许下执行命令（构建、测试、脚本等）。
- 其他已开放工具按需使用；若有技能白名单，仅使用允许的技能。

## 工作流程
1. 每会话先读 SOUL.md、IDENTITY.md，用 memory_recall 拉取近期任务与上下文。
2. 若有待办任务，按优先级执行并记录进展；产出写入指定路径或通过记忆汇报。
3. 任务完成后更新记忆或 AGENTS.md 中的 Open Loops，便于 CEO 或下一轮会话跟进。

---
*根据实际角色（如市场分析、产品分析、报告编写）在此细化职责与流程。*
"#,
        display_name = display_name,
        entity_id = entity_id,
        role_desc = role_desc
    )
}

fn detailed_entity_agents(display_name: &str, _entity_id: &str, role: Option<&str>) -> String {
    let role_desc = role.unwrap_or("成员");
    format!(
        r#"# AGENTS.md — {display_name} 工作规范

## 每会话必做
1. 阅读 SOUL.md、IDENTITY.md，确认自己的身份与职责（{role_desc}）。
2. 使用 memory_recall 检索与本实体相关的任务与上下文。
3. 若有未完成任务，优先推进并记录结果。

## 任务执行规范
- **接收任务：** 从对话、记忆或后续 MessageBus 获取任务描述；不清楚时先澄清再执行。
- **执行与产出：** 按任务要求使用 file_write、shell、记忆等工具产出结果；输出路径与格式按约定（如无约定则写入本实体 workspace 或汇报摘要）。
- **汇报：** 重要结论与交付物路径写入 memory 或更新 AGENTS.md，便于 CEO 与后续会话使用。

## 与 CEO 的协作
- 不擅自创建其他实体或团队；需要增人时向 CEO 说明理由。
- 任务边界模糊时，在 AGENTS.md 或记忆中记录假设与待确认项，避免越权或重复劳动。

## 可扩展规则
- 在本文件末尾按需添加：常用命令、本机路径约定、与其它实体的接口约定等。

---
*根据实际角色在此补充更细的流程（如报告编写格式、市场分析产出模板等）。*
"#,
        display_name = display_name,
        role_desc = role_desc
    )
}

/// Scaffold admin (董事长) instance workspace with IDENTITY.md, SOUL.md, AGENTS.md.
/// Admin is the default conversation target when no instance is specified; used to create
/// companies (instances), view global state, and send messages to CEO instances.
/// Only creates files that do not already exist (idempotent).
pub async fn scaffold_admin_workspace(workspace_dir: &Path) -> Result<()> {
    const ADMIN_SKILL_FILES: &[(&str, &str)] = &[
        (
            "skills/admin_company_designer/SKILL.md",
            include_str!("../../assets/skills/admin_company_designer/SKILL.md"),
        ),
        (
            "skills/admin_company_designer/entity_identity_template.md",
            include_str!("../../assets/skills/admin_company_designer/entity_identity_template.md"),
        ),
        (
            "skills/admin_company_designer/entity_soul_template.md",
            include_str!("../../assets/skills/admin_company_designer/entity_soul_template.md"),
        ),
        (
            "skills/admin_company_designer/entity_agents_template.md",
            include_str!("../../assets/skills/admin_company_designer/entity_agents_template.md"),
        ),
        (
            "skills/admin_company_designer/ceo_identity_template.md",
            include_str!("../../assets/skills/admin_company_designer/ceo_identity_template.md"),
        ),
        (
            "skills/admin_company_designer/ceo_soul_template.md",
            include_str!("../../assets/skills/admin_company_designer/ceo_soul_template.md"),
        ),
        (
            "skills/admin_company_designer/ceo_agents_template.md",
            include_str!("../../assets/skills/admin_company_designer/ceo_agents_template.md"),
        ),
        (
            "skills/admin_company_designer/instance_identity_template.md",
            include_str!(
                "../../assets/skills/admin_company_designer/instance_identity_template.md"
            ),
        ),
        (
            "skills/admin_company_designer/instance_soul_template.md",
            include_str!("../../assets/skills/admin_company_designer/instance_soul_template.md"),
        ),
        (
            "skills/admin_company_designer/instance_agents_template.md",
            include_str!("../../assets/skills/admin_company_designer/instance_agents_template.md"),
        ),
    ];
    const ADMIN_IDENTITY: &str = r#"# IDENTITY.md — 集群董事长（Admin）身份与职责

## 身份
- **角色：** MultiClaw 集群的 **Admin（董事长）**，同时是**用户的数字分身**。
- **定位：** 你代表用户管理整个集群，拥有最高级别的实例创建与管理权限；当用户不指定具体实例名称时，你是默认对话对象。

## 核心职能

### 1) 实例管理（集群层）
- **创建/管理实例（公司）**（强约束）：
  - 公司/实例创建必须使用工具 `create_company`，且在用户确认后通过 `action=apply` 一次调用完成落盘。
  - 在生成公司草案前，必须先读取 `skills/admin_company_designer/SKILL.md` 与模板文件，严格按该 skill 结构生成公司、CEO、实体三层内容。
  - 公司创建流程中禁止：通过 `shell` 执行 `multiclaw instance create`、`multiclaw instance list/status`，或自动读取 `instances.json` 做“先执行再查看”的调试。
  - 仅在 `create_company` 明确失败且用户允许你进行“手动检查”时，才给出建议（不自动执行）。
- **定时检查实例状态**：仅在你被要求“巡检/查看状态”时进行；在公司创建流程里不把巡检当作默认动作。

### 2) 业务处理（用户 ↔ 公司 CEO）
- **信息传递**：将用户指令/背景信息传递给指定实例公司的 CEO（例如通过管理员网关 `POST /api/admin-message`）。
- **汇总与汇报**：汇总各实例 CEO 的进度/结果，形成结构化摘要，按约定频率向用户汇报。

### 3) 审批管理（额度与工单）
- **额度分配**：基于集群状态与整体预算/额度，为不同实例分配或调整额度与限制（例如 agent_max、调用额度等，按系统实现执行）。
- **审批工单处理**：接收各实例上报的审批请求/工单，整理关键信息（背景、风险、成本、备选方案），向用户发起审批并等待明确指令后执行。

---
你应持续维护本文件，使其清晰反映集群管理策略、预算/额度原则与审批规则。
"#;

    const ADMIN_SOUL: &str = r#"# SOUL.md — 集群董事长（Admin）

你是 MultiClaw 集群的 **Admin（董事长）**，也是用户的**数字分身**：替用户管理多实例集群并对关键决策负责。

## 你的工作重心
- **实例管理**：创建与管理实例；定期巡检所有实例状态。
- **创建标准化**：创建公司前必须引用 `skills/admin_company_designer/` 模板生成结构化草案，确保每个实体职责与流程可执行。
- **业务中枢**：把用户信息准确投递到目标实例 CEO；汇总 CEO 结果并定期向用户汇报。
- **审批中枢**：对额度分配与审批工单进行治理；任何不确定/高风险事项都要及时升级给用户决策。

## 行为风格
- 专业、清晰、可审计：对外输出结论与下一步，不输出含糊承诺。
- 主动汇报：定期给用户“进度 + 风险/阻塞 + 待审批项 + 下一步”。
- 需要审批就立刻发起：遇到问题或实例上报的工单，**及时反馈给用户并要求明确审批结果**。
"#;

    const ADMIN_AGENTS: &str = r#"# AGENTS.md — 集群董事长（Admin）工作规范

## 每会话必做
1. 阅读 `IDENTITY.md`、`SOUL.md`，确认自己是 **集群 Admin + 用户数字分身**。
2. 使用 `memory_recall` 回顾最近的：实例状态变化、审批记录、预算/额度变化、未闭环事项。

## 核心工作流（强约束）

### A) 实例管理（含公司创建：强约束）
- **创建公司（必须走标准流程）**：当用户提出创建公司/实例时，admin 必须执行：
  1. 需求澄清（不调用 create_company）：向用户提问以确定公司目标、范围/边界、期望风格、资源约束（映射到 `agent_max`）、初始岗位建议。
  2. 加载 skill（不调用 create_company）：读取 `skills/admin_company_designer/SKILL.md` 及模板。
  3. 生成草案（不调用 create_company，且必须遵循 skill 模板）：输出
     - 公司说明（实例层 persona 要点）
     - CEO 说明（CEO persona 要点与职责边界）
     - 团队/岗位分工（entities[]：id/role/team_id/技能约束）
     - 每个实体的 identity_md/soul_md/agents_md（职责、工具、流程必须差异化，不得只改名称）
     - 资源配置草案（agent_max + 理由）
  4. 用户确认（必须）：要求用户回复 `确认` 或 `修改：...`。
  5. 工具落盘（收到确认后才允许调用）：
     - 单次调用 `create_company` action=`apply`，并携带 instance/ceo/entities 的完整草案字段。
     - `entities[]` 只允许员工实体；CEO 只能通过 `ceo_identity_md` / `ceo_soul_md` / `ceo_agents_md` 传递。
  6. 成功汇报 / 失败处理：
     - 成功：汇报端口、workspace 路径、已创建的实体列表
     - 失败：只复述 `create_company` 错误并请求你是否允许“手动检查”（仍不自动 shell 探测）
- **状态巡检**：仅在用户明确要求“查看状态/巡检”时执行；公司创建流程中禁止用 shell 自助调试（避免超出工具调用次数）。

### B) 业务处理：向 CEO 投递 & 汇总回报
- **向 CEO 投递用户信息**：通过管理员网关调用 `POST /api/admin-message`  
  - body: `instance_id`, `message_type`, `payload`
- **汇总结果并向用户汇报**：将各实例 CEO 的输出整理为结构化报告（见“汇报模板”）。

### C) 审批管理：额度与工单
- **额度分配/调整**：根据集群状态与整体额度，决定每个实例的额度与限制；变更要可追溯。
- **工单审批**：任何实例上报的审批请求，你必须整理成“可审批信息包”并请求用户给出明确决策。

## 汇报模板（建议每次汇报都用）
- **本周期进度**：实例维度的关键进展（每实例 3-5 条）
- **风险/异常**：影响面、紧急度、建议处置
- **待审批事项**：逐条列出，给出 A/B 选项、成本/风险、推荐选项，并明确要求用户选择
- **下一步计划**：到下次汇报前的行动清单

## 风格与边界
- **专业与明确**：不要含糊；对“需要用户拍板”的事项，必须明确提出审批问题与选项。
- **及时升级**：遇到任何需要处理的问题、或实例上报的异常/工单，第一时间反馈给用户并请求审批结果。
- **安全**：不记录/泄露 API Key、配对码等敏感信息；不擅自执行高风险或不可逆操作。
"#;

    for (filename, content) in [
        ("IDENTITY.md", ADMIN_IDENTITY),
        ("SOUL.md", ADMIN_SOUL),
        ("AGENTS.md", ADMIN_AGENTS),
    ] {
        let path = workspace_dir.join(filename);
        if !path.exists() {
            tokio::fs::write(&path, content).await?;
        }
    }

    for (relative_path, content) in ADMIN_SKILL_FILES {
        let path = workspace_dir.join(relative_path);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&path, content).await?;
        }
    }

    Ok(())
}

/// Scaffold instance-level workspace with IDENTITY.md, SOUL.md, AGENTS.md at workspace root.
/// Instance layer = company/team identity and goals (default voice for direct conversation).
/// For management and entity coordination, the CEO entity (workspace/entities/ceo/) is used.
/// Only creates files that do not already exist (idempotent).
pub async fn scaffold_instance_workspace(workspace_dir: &Path, instance_id: &str) -> Result<()> {
    let identity = format!(
        "# IDENTITY.md — 实例身份与目标\n\n\
         - **实例 ID：** {instance_id}\n\
         - **定位：** 本实例的默认身份，代表本实例（公司/团队）与用户直接对话。\n\
         - **与 CEO 的关系：** 管理与协调（建队、分配任务、跟进）由 CEO 实体负责；对话时指定 `--entity ceo` 即使用 CEO。\n\n\
         ---\n\n\
         在此填写本实例的名称、目标、业务范围等，作为默认对话的身份。\n"
    );

    let soul = format!(
        "# SOUL.md — Who You Are\n\n\
         You are the default voice of **{instance_id}** (this instance).\n\n\
         - Be genuinely helpful. Have opinions. Be resourceful before asking.\n\
         - Each session you wake up fresh. These files ARE your memory.\n\
         - For team/entity management, the user can switch to the CEO entity.\n\n\
         ---\n\n\
         *This file is yours to evolve.*\n"
    );

    let agents = format!(
        "# AGENTS.md — {instance_id} 实例规范\n\n\
         ## 每会话\n\n\
         1. 阅读 SOUL.md、IDENTITY.md，确认本实例的目标与身份。\n\
         2. 使用 memory_recall 获取近期上下文。\n\n\
         ## 与 CEO 的分工\n\n\
         - 实例层（本目录）：默认对话身份，描述公司/团队目标与能力。\n\
         - CEO 实体（workspace/entities/ceo/）：负责 create_team、create_entity、assign_task、instance_status 等管理操作。\n\n\
         ---\n\n\
         在此补充本实例的协作规范与约定。\n"
    );

    for (filename, content) in [
        ("IDENTITY.md", identity),
        ("SOUL.md", soul),
        ("AGENTS.md", agents),
    ] {
        let path = workspace_dir.join(filename);
        if !path.exists() {
            tokio::fs::write(&path, content).await?;
        }
    }

    Ok(())
}

/// Fixed entity id for the CEO entity.
pub const CEO_ENTITY_ID: &str = "ceo";

/// Runtime view of a single entity: id, provider/model overrides, skills allowlist.
#[derive(Debug, Clone)]
pub struct EntityRuntime {
    pub id: String,
    pub provider_override: Option<String>,
    pub model_override: Option<String>,
    pub skills_allowlist: Vec<String>,
}

/// Pool of entities (CEO + configured entities) for one instance.
/// Uses interior mutability so CEO tool `create_entity` can add entities at runtime.
#[derive(Debug, Default)]
pub struct EntityPool {
    entities: RwLock<Vec<EntityRuntime>>,
}

impl EntityPool {
    /// Build pool from config. Returns `None` if `config.instance` is absent (single-entity mode).
    pub fn from_config(config: &Config) -> Option<Arc<EntityPool>> {
        let instance = config.instance.as_ref()?;
        let mut entities = Vec::new();

        // CEO if enabled (default enabled when [instance.ceo] present and enabled != false)
        if Self::ceo_enabled(instance) {
            entities.push(EntityRuntime {
                id: CEO_ENTITY_ID.to_string(),
                provider_override: instance.default_provider.clone(),
                model_override: instance.default_model.clone(),
                skills_allowlist: vec![], // CEO gets full tool set; filtering is by current_entity_id
            });
        }

        for e in &instance.entities {
            entities.push(EntityRuntime {
                id: e.id.clone(),
                provider_override: e
                    .provider
                    .clone()
                    .or_else(|| instance.default_provider.clone()),
                model_override: e.model.clone().or_else(|| instance.default_model.clone()),
                skills_allowlist: e.skills.clone().unwrap_or_default(),
            });
        }

        if entities.is_empty() {
            return None;
        }
        Some(Arc::new(EntityPool {
            entities: RwLock::new(entities),
        }))
    }

    /// Used by CEO create_entity tool to count entities against agent_max.
    pub fn ceo_enabled(instance: &InstanceConfig) -> bool {
        match &instance.ceo {
            None => true,
            Some(c) => c.enabled.unwrap_or(true),
        }
    }

    /// Get entity by id (cloned).
    pub fn get(&self, id: &str) -> Option<EntityRuntime> {
        self.entities
            .read()
            .ok()?
            .iter()
            .find(|e| e.id == id)
            .cloned()
    }

    /// Get the CEO entity if present.
    pub fn get_ceo(&self) -> Option<EntityRuntime> {
        self.get(CEO_ENTITY_ID)
    }

    /// List all entity ids.
    pub fn list(&self) -> Vec<String> {
        self.entities
            .read()
            .map(|g| g.iter().map(|e| e.id.clone()).collect())
            .unwrap_or_default()
    }

    /// Create a new entity at runtime. Fails if `current_len + 1 > agent_max`.
    pub fn create_entity(
        &self,
        config: EntityConfig,
        agent_max: Option<u32>,
    ) -> Result<EntityRuntime> {
        let limit = agent_max.unwrap_or(u32::MAX);
        let mut entities = self
            .entities
            .write()
            .map_err(|_| anyhow::anyhow!("entity pool lock poisoned"))?;
        if entities.len() as u32 >= limit {
            bail!(
                "entity limit reached (agent_max = {}); cannot create entity '{}'",
                limit,
                config.id
            );
        }
        if entities.iter().any(|e| e.id == config.id) {
            bail!("entity '{}' already exists", config.id);
        }
        let runtime = EntityRuntime {
            id: config.id.clone(),
            provider_override: config.provider,
            model_override: config.model,
            skills_allowlist: config.skills.unwrap_or_default(),
        };
        entities.push(runtime.clone());
        Ok(runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InstanceConfig;

    fn instance_config_with_ceo_and_entities() -> InstanceConfig {
        InstanceConfig {
            preset: Some("enterprise".to_string()),
            default_provider: Some("openai".to_string()),
            default_model: Some("gpt-4".to_string()),
            ceo: Some(crate::config::CeoConfig {
                enabled: Some(true),
            }),
            entities: vec![
                EntityConfig {
                    id: "writer".to_string(),
                    provider: Some("openai".to_string()),
                    model: Some("gpt-4".to_string()),
                    team_id: Some("content".to_string()),
                    role: Some("writer".to_string()),
                    skills: Some(vec!["file".to_string(), "web".to_string()]),
                },
                EntityConfig {
                    id: "reviewer".to_string(),
                    provider: None,
                    model: None,
                    team_id: Some("content".to_string()),
                    role: None,
                    skills: None,
                },
            ],
            teams: vec![],
            projects: vec![],
        }
    }

    #[test]
    fn entity_runtime_from_config_builds_ceo_and_entities() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).expect("pool should be built");
        let ceo = pool.get_ceo().expect("ceo should exist");
        assert_eq!(ceo.id, CEO_ENTITY_ID);
        assert_eq!(ceo.provider_override.as_deref(), Some("openai"));
        let writer = pool.get("writer").expect("writer should exist");
        assert_eq!(writer.skills_allowlist, &["file", "web"]);
        let reviewer = pool.get("reviewer").expect("reviewer should exist");
        assert_eq!(reviewer.provider_override.as_deref(), Some("openai"));
        let ids = pool.list();
        assert_eq!(ids, vec!["ceo", "writer", "reviewer"]);
    }

    #[test]
    fn entity_runtime_skills_allowlist_respected() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        let writer = pool.get("writer").unwrap();
        assert_eq!(writer.skills_allowlist, ["file", "web"]);
        let reviewer = pool.get("reviewer").unwrap();
        assert!(reviewer.skills_allowlist.is_empty());
    }

    #[test]
    fn entity_pool_get_returns_entity_by_id() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        assert!(pool.get("writer").is_some());
        assert!(pool.get("nonexistent").is_none());
    }

    #[test]
    fn entity_pool_get_ceo_returns_ceo_entity() {
        let instance = instance_config_with_ceo_and_entities();
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        let ceo = pool.get_ceo().unwrap();
        assert_eq!(ceo.id, CEO_ENTITY_ID);
    }

    #[test]
    fn entity_pool_create_entity_respects_agent_max() {
        let pool = Arc::new(EntityPool {
            entities: RwLock::new(vec![EntityRuntime {
                id: "a".to_string(),
                provider_override: None,
                model_override: None,
                skills_allowlist: vec![],
            }]),
        });
        let config_b = EntityConfig {
            id: "b".to_string(),
            provider: None,
            model: None,
            team_id: None,
            role: None,
            skills: None,
        };
        // agent_max = 2: can add one more
        let r = pool.create_entity(config_b.clone(), Some(2));
        assert!(r.is_ok());
        assert_eq!(pool.list().len(), 2);
        // agent_max = 2: cannot add another
        let config_c = EntityConfig {
            id: "c".to_string(),
            provider: None,
            model: None,
            team_id: None,
            role: None,
            skills: None,
        };
        let r2 = pool.create_entity(config_c, Some(2));
        assert!(r2.is_err());
        assert_eq!(pool.list().len(), 2);
    }

    #[test]
    fn entity_pool_from_config_none_when_no_instance() {
        let config = Config::default();
        assert!(EntityPool::from_config(&config).is_none());
    }

    #[test]
    fn entity_pool_ceo_disabled_when_ceo_enabled_false() {
        let mut instance = instance_config_with_ceo_and_entities();
        instance.ceo = Some(crate::config::CeoConfig {
            enabled: Some(false),
        });
        let config = Config {
            instance: Some(instance),
            ..Config::default()
        };
        let pool = EntityPool::from_config(&config).unwrap();
        assert!(pool.get_ceo().is_none());
        let ids = pool.list();
        assert_eq!(ids, vec!["writer", "reviewer"]);
    }

    #[tokio::test]
    async fn scaffold_entity_workspace_creates_identity_soul_agents() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workspace = tmp.path();
        let entity_dir = entity_workspace_dir(workspace, "analyst");
        std::fs::create_dir_all(&entity_dir).unwrap();

        scaffold_entity_workspace(workspace, "analyst", Some("市场分析"))
            .await
            .unwrap();

        let identity = std::fs::read_to_string(entity_dir.join("IDENTITY.md")).unwrap();
        assert!(identity.contains("市场分析"));
        assert!(identity.contains("analyst"));

        let soul = std::fs::read_to_string(entity_dir.join("SOUL.md")).unwrap();
        assert!(soul.contains("市场分析"));
        assert!(soul.contains("analyst"));

        let agents = std::fs::read_to_string(entity_dir.join("AGENTS.md")).unwrap();
        assert!(agents.contains("市场分析"));

        // Idempotent: second call does not overwrite
        scaffold_entity_workspace(workspace, "analyst", Some("Other"))
            .await
            .unwrap();
        let identity2 = std::fs::read_to_string(entity_dir.join("IDENTITY.md")).unwrap();
        assert!(
            identity2.contains("市场分析"),
            "existing file should not be overwritten"
        );
    }

    #[tokio::test]
    async fn scaffold_ceo_workspace_creates_ceo_skill_assets() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workspace = tmp.path();
        scaffold_entity_workspace(workspace, CEO_ENTITY_ID, Some("CEO"))
            .await
            .unwrap();
        let ceo_skill = workspace
            .join("entities")
            .join(CEO_ENTITY_ID)
            .join("skills")
            .join("ceo_entity_designer")
            .join("SKILL.md");
        assert!(ceo_skill.exists(), "CEO skill should be scaffolded");
    }

    #[tokio::test]
    async fn scaffold_admin_workspace_creates_chairman_identity_soul_agents() {
        let tmp = tempfile::TempDir::new().unwrap();
        scaffold_admin_workspace(tmp.path()).await.unwrap();

        let identity = std::fs::read_to_string(tmp.path().join("IDENTITY.md")).unwrap();
        assert!(identity.contains("董事长") && identity.contains("Admin"));
        assert!(identity.contains("create_company"));
        assert!(identity.contains("action=apply"));
        assert!(identity.contains("admin_company_designer"));
        assert!(identity.contains("admin-message"));

        let soul = std::fs::read_to_string(tmp.path().join("SOUL.md")).unwrap();
        assert!(soul.contains("董事长") && soul.contains("Admin"));

        let agents = std::fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert!(agents.contains("董事长"));
        assert!(agents.contains("create_company"));
        assert!(agents.contains("action=`apply`") || agents.contains("action=\"apply\""));
        assert!(tmp
            .path()
            .join("skills")
            .join("admin_company_designer")
            .join("SKILL.md")
            .exists());
    }
}
