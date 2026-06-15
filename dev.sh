#!/usr/bin/env bash
# Rebuild the octovia WASM package and launch the Vite dev server.
#
# Usage:
#   ./dev.sh              # release build + dev server
#   ./dev.sh --debug      # dev (unoptimised) build, faster compile
#
# Any extra arguments are forwarded to `npm run dev` after `--`.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="${ROOT_DIR}/rust"
SITE_DIR="${ROOT_DIR}/site"
PKG_DIR="${RUST_DIR}/pkg"
SITE_PKG_DIR="${SITE_DIR}/src/octovia"

PROFILE="--release"
EXTRA_ARGS=()

for arg in "$@"; do
    case "$arg" in
        --debug)  PROFILE="--dev" ;;
        *)        EXTRA_ARGS+=("$arg") ;;
    esac
done

if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "error: wasm-pack not found on PATH." >&2
    echo "install with: cargo install wasm-pack" >&2
    exit 1
fi

echo ">> Building octovia WASM (${PROFILE#--})..."
(cd "$RUST_DIR" && wasm-pack build --target web "$PROFILE" --out-dir pkg)

echo ">> Syncing pkg/ into site/src/octovia/..."
mkdir -p "$SITE_PKG_DIR"
cp \
    "$PKG_DIR/octovia.js" \
    "$PKG_DIR/octovia.d.ts" \
    "$PKG_DIR/octovia_bg.wasm" \
    "$PKG_DIR/octovia_bg.wasm.d.ts" \
    "$PKG_DIR/package.json" \
    "$SITE_PKG_DIR/"

echo ">> Launching Vite dev server..."
exec npm --prefix "$SITE_DIR" run dev -- --host "${EXTRA_ARGS[@]}"
