#!/usr/bin/env bash
# MultiClaw 多实例 + 多实体验证脚本
set -e

# 确保使用 ~/.multiclaw 作为集群根（避免 MULTICLAW_CLUSTER_ROOT 指向临时目录）
export MULTICLAW_CLUSTER_ROOT="${HOME}/.multiclaw"
unset MULTICLAW_WORKSPACE
unset MULTICLAW_INSTANCE

MULTICLAW_ROOT="${HOME}/.multiclaw"
MULTICLAW_BIN="./target/release/multiclaw"
API_KEY="sk-sp-1609be3c50c34c16829b554fa6390255"
PROVIDER="qwen-coding-plan"
MODEL="qwen3.5-plus"

cd "$(dirname "$0")/.."

echo "=== 1. 编译项目 ==="
cargo build --release 2>&1 | tail -5

echo ""
echo "=== 2. 停止已有服务并清理 ~/.multiclaw ==="
pkill -f "multiclaw daemon" 2>/dev/null || true
pkill -f "multiclaw gateway" 2>/dev/null || true
sleep 1
rm -rf "$MULTICLAW_ROOT"
echo "已清理 $MULTICLAW_ROOT"

echo ""
echo "=== 3. 初始化集群（创建 admin 实例） ==="
mkdir -p "$MULTICLAW_ROOT/instances/admin/workspace"
cat > "$MULTICLAW_ROOT/instances.json" << 'REGISTRY'
{
  "version": 1,
  "instances": [
    {
      "id": "admin",
      "role": "admin",
      "status": "created",
      "config_path": null,
      "workspace_path": null,
      "gateway_port": 42617,
      "created_at": null,
      "constraints": null,
      "preset": null
    }
  ]
}
REGISTRY

# 管理员实例完整配置：无配对码、不加密密钥、模型信息
cat > "$MULTICLAW_ROOT/instances/admin/config.toml" << CONFIG
default_provider = "$PROVIDER"
default_model = "$MODEL"
default_temperature = 0.7
api_key = "$API_KEY"

[gateway]
port = 42617
host = "127.0.0.1"
require_pairing = false

[secrets]
encrypt = false

[channels_config]
cli = true

[cron]
enabled = true
CONFIG

# 更新 instances.json 中的路径
ADMIN_DIR="$MULTICLAW_ROOT/instances/admin"
CREATED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
cat > "$MULTICLAW_ROOT/instances.json" << REGISTRY2
{
  "version": 1,
  "instances": [
    {
      "id": "admin",
      "role": "admin",
      "status": "created",
      "config_path": "${ADMIN_DIR}/config.toml",
      "workspace_path": "${ADMIN_DIR}/workspace",
      "gateway_port": 42617,
      "created_at": "${CREATED_AT}",
      "constraints": null,
      "preset": null
    }
  ]
}
REGISTRY2

echo "Admin 实例已创建: $MULTICLAW_ROOT/instances/admin/"
echo "  端口: 42617"
echo "  require_pairing = false (无配对码)"
echo "  secrets.encrypt = false (密钥明文，可复制)"
echo "  provider: $PROVIDER, model: $MODEL"

echo ""
echo "=== 4. 启动管理员实例网关（后台） ==="
$MULTICLAW_BIN --instance admin gateway &
GATEWAY_PID=$!
sleep 3
if kill -0 $GATEWAY_PID 2>/dev/null; then
  echo "网关已启动 (PID: $GATEWAY_PID)，端口 42617"
  echo "require_pairing=false 时无配对码，可直接访问"
else
  echo "警告: 网关可能启动失败，请检查"
fi

echo ""
echo "=== 重启网关（无配对码） ==="
echo "  各实例 config.toml 中 [gateway] require_pairing = false 时，重启后无需配对码即可访问管理页。"
echo "  重启命令: $MULTICLAW_BIN --instance <id> gateway  或  $MULTICLAW_BIN --instance <id> daemon"
echo ""

echo ""
echo "=== 验证命令示例 ==="
echo "  # 与管理员实例对话:"
echo "  $MULTICLAW_BIN --instance admin agent -m '1+1'"
echo ""
echo "  # 创建初创公司实例:"
echo "  $MULTICLAW_BIN --instance admin instance create startup1 --preset startup"
echo ""
echo "  # 与指定实体(CEO)对话:"
echo "  $MULTICLAW_BIN --instance startup1 agent --target-entity ceo -m '你好'"
echo ""
echo "验证脚本完成。请手动执行后续测试。"
