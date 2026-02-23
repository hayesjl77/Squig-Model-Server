#!/usr/bin/env bash
set -euo pipefail

echo "═══════════════════════════════════════"
echo "  Squig Model Server - Build (Linux)"
echo "═══════════════════════════════════════"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# ─── Build UI ────────────────────────────────────────
echo ""
echo "→ Building dashboard UI..."
cd ui

if [ ! -d "node_modules" ]; then
    echo "  Installing npm dependencies..."
    npm install
fi

npm run build
echo "  ✓ UI built to ui/dist/"

cd "$PROJECT_DIR"

# ─── Build Rust server ───────────────────────────────
echo ""
echo "→ Building Rust server (release)..."
cargo build --release

echo ""
echo "✓ Build complete!"
echo "  Binary: target/release/squig-model-server"
echo ""
echo "To run:"
echo "  ./target/release/squig-model-server"
echo ""
echo "To run with custom config:"
echo "  ./target/release/squig-model-server -c /path/to/config.toml"
