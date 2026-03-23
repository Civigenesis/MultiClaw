#!/usr/bin/env bash
# Build the Vite dashboard so rust-embed picks up a fresh web/dist at compile time.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT/web"

if ! command -v npm >/dev/null 2>&1; then
  echo "❌ npm not found — install Node.js 20+ to build the embedded dashboard." >&2
  exit 1
fi

if [[ ! -f package-lock.json ]]; then
  echo "❌ web/package-lock.json missing" >&2
  exit 1
fi

echo "==> web dashboard: npm ci && npm run build"
npm ci
npm run build

if [[ ! -f dist/index.html ]]; then
  echo "❌ web/dist/index.html not produced" >&2
  exit 1
fi
