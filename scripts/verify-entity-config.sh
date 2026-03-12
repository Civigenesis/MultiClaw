#!/usr/bin/env bash
# 验证多实体详细配置流程：编译、清空实例、重启、创建实例与 CEO、创建团队
# 用法: ./scripts/verify-entity-config.sh [--no-clean]
# 默认会删除 cluster 下的 instances 与 instances.json，便于干净验证。
# 使用 --no-clean 时仅编译并打印后续手动步骤。

set -e
CLUSTER_ROOT="${MULTICLAW_CLUSTER_ROOT:-$HOME/.multiclaw}"
DO_CLEAN=1
if [[ "${1:-}" == "--no-clean" ]]; then
  DO_CLEAN=0
fi

echo "=== 1. 重新编译 ==="
cargo build --release

echo ""
if [[ $DO_CLEAN -eq 1 ]]; then
  echo "=== 2. 删除已有实例配置（保留 cluster 根目录与 admin 配置）==="
  if [[ -f "$CLUSTER_ROOT/instances.json" ]]; then
    rm -f "$CLUSTER_ROOT/instances.json"
    echo "  已删除 $CLUSTER_ROOT/instances.json"
  fi
  if [[ -d "$CLUSTER_ROOT/instances" ]]; then
    rm -rf "$CLUSTER_ROOT/instances"
    echo "  已删除 $CLUSTER_ROOT/instances/"
  fi
  echo "  清理完成。请确保 admin 的 config 存在：$CLUSTER_ROOT/config.toml（或通过 --config 指定）"
else
  echo "=== 2. 跳过清理（--no-clean）==="
fi

echo ""
echo "=== 3. 后续手动步骤 ==="
echo "  3.1 正确配置 admin 的模型信息"
echo "      编辑 $CLUSTER_ROOT/config.toml，设置 default_provider / default_model 或 [providers]。"
echo ""
echo "  3.2 启动 daemon（若尚未运行）"
echo "      multiclaw daemon   # 或: MULTICLAW_CLUSTER_ROOT=$CLUSTER_ROOT multiclaw daemon"
echo ""
echo "  3.3 让 admin 创建带 CEO 的实例（--preset startup 会写入 [instance]+[instance.ceo] 并创建 workspace/entities/ceo/ 及详细 IDENTITY/AGENTS）"
echo "      multiclaw --instance admin instance create <实例id> --preset startup"
echo ""
echo "  3.4 连接该实例并让 CEO 创建团队与成员"
echo "      multiclaw --instance <实例id> cli   # 或通过 channel 与 CEO 对话"
echo "      在对话中让 CEO 使用 create_team / create_entity。"
echo "      创建成员后，CEO 会收到提示：为该成员撰写或更新 IDENTITY.md 与 AGENTS.md（50–200 字）。"
echo ""
echo "  3.5 验证"
echo "      检查各实体目录下的 IDENTITY.md、AGENTS.md、agent.md 是否为详细内容（非一句描述）："
echo "      ls -la $CLUSTER_ROOT/instances/<实例id>/workspace/entities/"
echo "      cat $CLUSTER_ROOT/instances/<实例id>/workspace/entities/ceo/IDENTITY.md"
echo "      cat $CLUSTER_ROOT/instances/<实例id>/workspace/entities/<成员id>/IDENTITY.md"
echo ""
