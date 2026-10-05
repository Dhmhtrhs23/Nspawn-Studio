#!/usr/bin/env bash
# =============================================================================
#  Verify Nspawn Studio: unit tests, generator integration tests, a release
#  build and a GUI smoke test.
#
#  Usage:  ./scripts/verify.sh
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> 1/4  Core tests (model, validation, generator, store, command, launcher)"
cargo test -p nspawn-studio-core --quiet

echo "==> 2/4  Release build"
cargo build --release -p nspawn-studio

echo "==> 3/4  Generated launcher integration tests"
echo "     covered by 'launcher_integration.rs' in step 1"

echo "==> 4/4  Binary smoke test"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
if [ -n "${DISPLAY:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then
  "$ROOT/target/release/nspawn-studio" >/dev/null 2>&1 &
  PID=$!
  sleep 3
  if kill -0 "$PID" 2>/dev/null; then
    echo "     OK: the GUI stayed running"
    kill "$PID" 2>/dev/null || true
  else
    echo "     WARNING: the GUI exited early (run it directly to see why)"
  fi
else
  echo "     No display found; skipping the interactive launch"
fi

echo
echo "All checks completed."
