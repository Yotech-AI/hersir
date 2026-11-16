#!/usr/bin/env bash
# Build Gizai (release) if needed and start it on dev data (.devdata/dev). Applies the Mesa EGL fix on machines
# without an NVIDIA GPU.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
[ -d node_modules ] || npm ci
# Rebuild when a binary is missing or any source file is newer than it. hersir-mcp (the helper Claude Code
# starts in a chat turn, built by the crate gizai-mcp) must sit next to hersir.
if [ ! -x target/release/hersir ] || [ -n "$(find src src-tauri/src src-tauri/tauri.conf.json src-tauri/capabilities crates index.html package.json -newer target/release/hersir -print -quit)" ]; then
  npm run tauri build -- --no-bundle
fi
if [ ! -x target/release/hersir-mcp ] || [ -n "$(find crates/gizai-mcp -newer target/release/hersir-mcp -print -quit)" ]; then
  cargo build --release -q -p gizai-mcp
fi
# The dev build runs on its own data, next to the Hersir you use (the app launcher one, on your real data), and never
# changes that one. Update it on purpose with ./install.sh --skip-build: that backs up your data first. The GIZAI_ names
# of these variables still work.
export HERSIR_DATA_DIR="${HERSIR_DATA_DIR:-${GIZAI_DATA_DIR:-$PWD/.devdata/dev}}"
# New dev data starts without the first start's onboarding; HERSIR_SKIP_ONBOARDING=0 shows it.
export HERSIR_SKIP_ONBOARDING="${HERSIR_SKIP_ONBOARDING-${GIZAI_SKIP_ONBOARDING-1}}"
echo "Dev build on the data in ${HERSIR_DATA_DIR/#$HOME/\~}. The Hersir in your app launcher is separate (update it: ./install.sh --skip-build)."
if ! lspci 2>/dev/null | grep -qi nvidia; then
  export __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json
fi
exec target/release/hersir "$@"
