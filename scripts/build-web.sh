#!/usr/bin/env bash
# Build the Vite dashboard into web/dist/ for rust-embed (see src/gateway/static_files.rs).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/web"
if [[ ! -f package-lock.json ]]; then
  echo "❌ web/package-lock.json not found" >&2
  exit 1
fi
npm ci
npm run build
echo "✅ web/dist ready — run: cargo build --release"
