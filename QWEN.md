# MultiClaw 项目上下文

## 项目概述

**MultiClaw** 是一个用 Rust 构建的高性能、低开销的多实例 AI 助手运行时系统。它被设计为"代理工作流运行时操作系统"，通过抽象模型、工具、内存和执行层，使代理能够"一次构建，随处运行"。

### 核心特性

- **极致轻量**: 发布版本二进制约 8.8MB，运行时内存 <5MB，启动时间 <10ms
- **Trait 驱动架构**: 所有子系统（提供商、渠道、工具、内存等）都是可插拔的 trait 实现
- **安全优先**: 默认安全配置，包括配对认证、沙箱隔离、显式允许列表、文件系统范围限制
- **完全可交换**: 支持通过配置切换 AI 提供商、消息渠道、工具、存储后端
- **无供应商锁定**: 支持 OpenAI 兼容接口 + 可插拔自定义端点
- **跨平台**: 支持 ARM、x86、RISC-V 架构，Linux/macOS/Windows 系统

### 支持的集成

| 子系统 | 内置支持 |
|--------|----------|
| **AI 提供商** | OpenRouter、Anthropic、OpenAI、Gemini、Groq、Mistral、DeepSeek、xAI、Z.AI(GLM) 等 20+ 提供商 |
| **消息渠道** | CLI、Telegram、Discord、Slack、Mattermost、iMessage、Matrix、Signal、WhatsApp、Email、IRC、Lark、钉钉、QQ、Nostr、Webhook |
| **存储后端** | SQLite（混合搜索）、PostgreSQL、Lucid 桥接、Markdown 文件 |
| **运行时** | Native（原生）、Docker（沙箱） |
| **硬件** | STM32、树莓派 GPIO（可选） |

## 项目结构

```
MultiClaw/
├── src/                      # 主要源代码
│   ├── agent/                # 代理编排循环
│   ├── providers/            # AI 模型提供商实现
│   ├── channels/             # 消息渠道实现
│   ├── tools/                # 工具执行系统
│   ├── memory/               # 记忆/持久化后端
│   ├── security/             # 安全策略和沙箱
│   ├── gateway/              # Webhook/网关服务器
│   ├── runtime/              # 运行时适配器
│   ├── config/               # 配置 schema 和加载
│   ├── observability/        # 日志和指标
│   ├── peripherals/          # 硬件外设（STM32、RPi GPIO）
│   └── main.rs               # CLI 入口点
├── crates/
│   └── robot-kit/            # 机器人工具包子 crate
├── docs/                     # 文档系统
├── scripts/                  # 构建和安装脚本
├── tests/                    # 集成测试
├── dev/                      # 开发工具
└── .github/                  # CI/CD 工作流
```

## 构建和运行

### 前置要求

- **Rust**: 1.87+ (`rustup install stable`)
- **构建工具**:
  - Linux: `build-essential pkg-config`
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio Build Tools + "Desktop development with C++" workload

### 快速开始

```bash
# 克隆仓库
git clone https://github.com/Civigenesis/MultiClaw.git
cd multiclaw

# 启用 Git hooks（预推送检查）
git config core.hooksPath .githooks

# 构建（调试模式）
cargo build

# 构建（发布模式，优化体积）
cargo build --release --locked

# 运行测试
cargo test --locked

# 格式化和 lint 检查
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# 安装到系统
cargo install --path . --force --locked

# 一键安装（包含依赖和 Rust 工具链）
./bootstrap.sh --install-system-deps --install-rust
```

### 常用命令

```bash
# 初始化配置（交互式）
multiclaw onboard --interactive

# 初始化配置（非交互式）
multiclaw onboard --api-key "sk-..." --provider openrouter --model "openrouter/auto"

# 运行代理
multiclaw agent -m "Hello, MultiClaw!"
multiclaw agent  # 交互模式

# 启动守护进程（自主运行时）
multiclaw daemon

# 启动网关（webhook 服务器）
multiclaw gateway                # 默认：127.0.0.1:42617
multiclaw gateway --port 0       # 随机端口

# 检查状态
multiclaw status
multiclaw auth status

# 生成 Shell 补全
source <(multiclaw completions bash)
multiclaw completions zsh > ~/.zfunc/_multiclaw

# 运行系统诊断
multiclaw doctor

# 检查渠道健康
multiclaw channel doctor

# 管理服务
multiclaw service install
multiclaw service status
multiclaw service restart
```

### 开发模式运行

无需全局安装，使用 `cargo run --release --` 前缀：

```bash
cargo run --release -- status
cargo run --release -- agent -m "test"
```

### 性能优化构建

```bash
# 标准发布构建（优化体积，适合低内存设备）
cargo build --release

# 快速发布构建（优化编译速度，需要 16GB+ RAM）
cargo build --profile release-fast
```

## 开发约定

### 代码风格

- **最小依赖**: 每个 crate 都会增加二进制体积
- **内联测试**: 每个文件底部使用 `#[cfg(test)] mod tests {}`
- **Trait 优先**: 先定义 trait，再实现
- **默认安全**: 沙箱隔离、允许列表、永不默认阻止
- **生产代码不使用 unwrap**: 使用 `?`、`anyhow` 或 `thiserror`

### 命名约定

| 类型 | 命名风格 | 示例 |
|------|----------|------|
| 模块/文件 | `snake_case` | `discord_channel.rs` |
| 类型/Trait/枚举 | `PascalCase` | `DiscordChannel`, `SecurityPolicy` |
| 函数/变量 | `snake_case` | `send_message`, `channel_allowlist` |
| 常量/Static | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| Trait 实现者 | `<Domain><Type>` | `*Provider`, `*Channel`, `*Tool`, `*Memory` |
| 工厂键 | 小写稳定 | `"openai"`, `"discord"`, `"shell"` |
| 测试 | `<主题>_<预期行为>` | `allowlist_denies_unknown_user` |

### 架构边界规则

1. **通过 trait 实现 + 工厂注册扩展功能**，避免大规模重构
2. **依赖方向面向契约**: 具体集成依赖于 trait/config/util，而非其他具体集成
3. **避免跨子系统耦合**: 提供商代码不应导入渠道内部，工具代码不应直接修改网关/安全内部
4. **单一职责模块**: `agent` 编排、`channels` 传输、`providers` 模型 I/O、`security` 策略、`tools` 执行、`memory` 持久化
5. **共享抽象需三次重复**: 只有在稳定重复使用后才提取共享抽象
6. **配置键是公共契约**: `src/config/schema.rs` 的更改需要文档化默认值、兼容性影响、迁移步骤和回滚路径

### 提交约定

使用 [Conventional Commits](https://www.conventionalcommits.org/)：

```
feat: add Anthropic provider
feat(provider): add Anthropic provider
fix: path traversal edge case with symlinks
docs: update contributing guide
test: add heartbeat unicode parsing tests
refactor: extract common security checks
chore: bump tokio to 1.43
```

推荐的范围键：`provider`, `channel`, `memory`, `security`, `runtime`, `ci`, `docs`, `tests`

### 测试实践

```bash
# 运行所有测试
cargo test --locked

# 运行特定测试
cargo test -- test_name

# 预推送验证（启用 hooks 后自动运行）
./scripts/ci/rust_quality_gate.sh
```

### 秘密管理

**永远不要提交**：
- `.env` 文件（仅提交 `.env.example`）
- API 密钥、令牌、密码或凭证
- OAuth 令牌或会话标识符
- Webhook 签名秘密
- `~/.multiclaw/.secret_key` 或类似密钥文件
- 测试/夹具/示例中的个人标识符或真实用户数据

**本地开发**：
1. 复制 `.env.example` 到 `.env` 并填写值
2. `.env` 文件已被 Git 忽略，应保持在本地
3. 或使用 `multiclaw onboard` 进行交互式配置

## 添加新集成

### 添加提供商

1. 在 `src/providers/` 中实现 `Provider` trait
2. 在 `src/providers/mod.rs` 工厂中注册
3. 添加工厂配置和错误路径的测试

```rust
use async_trait::async_trait;
use anyhow::Result;
use crate::providers::traits::Provider;

pub struct YourProvider { /* ... */ }

#[async_trait]
impl Provider for YourProvider {
    async fn chat(&self, message: &str, model: &str, temperature: f64) -> Result<String> {
        // 实现
    }
}
```

### 添加渠道

1. 在 `src/channels/` 中实现 `Channel` trait
2. 保持一致的 `send`、`listen`、`health_check` 语义
3. 覆盖认证/允许列表/健康行为的测试

### 添加工具

1. 在 `src/tools/` 中实现 `Tool` trait，带有严格的参数 schema
2. 验证和清理所有输入
3. 返回结构化的 `ToolResult`；运行时路径中避免 panic

### 添加外设

1. 在 `src/peripherals/` 中实现 `Peripheral` trait
2. 外设暴露 `tools()` — 每个工具委托给硬件（GPIO、传感器等）
3. 如需在配置 schema 中添加板类型，参见 `docs/hardware-peripherals-design.md`

## 验证命令

### 本地预 PR 检查

```bash
# 格式化和 lint
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# 测试
cargo test

# 或使用 CI 脚本（推荐）
./dev/ci.sh all
```

### 可选严格检查

```bash
# 完整仓库严格 lint（发布前硬化）
./scripts/ci/rust_quality_gate.sh --strict

# 仅检查变更的 Rust 行（增量债务控制）
./scripts/ci/rust_strict_delta_gate.sh

# 文档质量检查（变更行 markdown 检查）
./scripts/ci/docs_quality_gate.sh

# 文档链接检查（仅检查新增链接）
./scripts/ci/docs_links_gate.sh
```

## 风险分层

PR 应根据风险级别映射到审查深度：

| Track | 典型范围 | 所需审查深度 |
|-------|----------|--------------|
| **A (低风险)** | 文档/测试/事务性更改，无安全/运行时/CI 影响 | 1 名维护者审查 + CI 通过 |
| **B (中风险)** | 提供商/渠道/内存/工具行为更改 | 1 名子系统感知审查 + 明确验证证据 |
| **C (高风险)** | `src/security/**`、`src/runtime/**`、`src/gateway/**`、`.github/workflows/**`、访问控制边界 | 双重审查（快速分类 + 深度风险审查），需要回滚计划 |

## 文档系统

### 入口点

- 根 README: `README.md`, `README.zh-CN.md`, `README.ja.md`, `README.ru.md`, `README.fr.md`, `README.vi.md`
- 文档中心: `docs/README.md`
- 统一目录: `docs/SUMMARY.md`

### 支持的区域设置

当前合同：`en`, `zh-CN`, `ja`, `ru`, `fr`, `vi`

### 运行时契约参考（必须跟踪行为更改）

- `docs/commands-reference.md`
- `docs/providers-reference.md`
- `docs/channels-reference.md`
- `docs/config-reference.md`
- `docs/operations-runbook.md`
- `docs/troubleshooting.md`
- `docs/one-click-bootstrap.md`

## 关键资源

- **项目仓库**: <https://github.com/Civigenesis/MultiClaw>
- **官方文档**: <https://multiclawlabs.ai>
- **文档中心**: [`docs/README.md`](docs/README.md)
- **贡献指南**: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- **代理工程协议**: [`AGENTS.md`](AGENTS.md)
- **故障排除**: [`docs/troubleshooting.md`](docs/troubleshooting.md)
- **操作手册**: [`docs/operations-runbook.md`](docs/operations-runbook.md)

## 许可证

MultiClaw 采用双许可：**MIT** 或 **Apache-2.0**（由用户选择）。
