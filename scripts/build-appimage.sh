#!/usr/bin/env bash
# =============================================================================
#  Build an AppImage for Nspawn Studio.
#
#  Usage:
#    ./scripts/build-appimage.sh                          # host architecture
#    ARCH=aarch64 ./scripts/build-appimage.sh             # explicit arch
#    TARGET=aarch64-unknown-linux-gnu ARCH=aarch64 ./scripts/build-appimage.sh
#
#  The optional TARGET variable selects a Rust cross-compilation target (used
#  by build-arm64.sh). Without it the host target is built, as before.
#
#  The resulting file is written to dist/. The AppImage uses the GTK4 and
#  libadwaita libraries provided by the host (see README).
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP_NAME="nspawn-studio"
VERSION="$(grep -m1 '^version' gui/Cargo.toml | cut -d'"' -f2)"

# Map the host machine to the AppImage architecture name.
host_arch() {
  case "$(uname -m)" in
    x86_64) echo "x86_64" ;;
    aarch64 | arm64) echo "aarch64" ;;
    *) uname -m ;;
  esac
}

ARCH="${ARCH:-$(host_arch)}"
TARGET="${TARGET:-}"

BUILD_DIR="$ROOT/target/appimage"
APPDIR="$BUILD_DIR/$APP_NAME.AppDir"
OUT_DIR="$ROOT/dist"
OUT="$OUT_DIR/$APP_NAME-$VERSION-$ARCH.AppImage"
TOOLS_DIR="$ROOT/target/appimage-tools"
HOST_ARCH="$(host_arch)"
TOOL="$TOOLS_DIR/appimagetool-$HOST_ARCH.AppImage"
TOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$HOST_ARCH.AppImage"
TARGET_RUNTIME="$TOOLS_DIR/runtime-$ARCH"
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-$ARCH"
ICON_DIR="$APPDIR/usr/share/icons/hicolor/scalable/apps"

if [ -n "$TARGET" ]; then
  echo "==> Building the release binary for $TARGET"
  cargo build --release --target "$TARGET" -p "$APP_NAME"
  BIN="target/$TARGET/release/$APP_NAME"
else
  echo "==> Building the release binary"
  cargo build --release -p "$APP_NAME"
  BIN="target/release/$APP_NAME"
fi

if [ ! -x "$BIN" ]; then
  echo "error: $BIN was not produced" >&2
  exit 1
fi

echo "==> Assembling the AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$ICON_DIR"

install -m 0755 "$BIN" "$APPDIR/usr/bin/$APP_NAME"
install -m 0644 "data/$APP_NAME.desktop" "$APPDIR/usr/share/applications/$APP_NAME.desktop"
install -m 0644 "data/$APP_NAME.desktop" "$APPDIR/$APP_NAME.desktop"
install -m 0644 "data/$APP_NAME.svg" "$ICON_DIR/$APP_NAME.svg"
install -m 0644 "data/$APP_NAME.svg" "$APPDIR/$APP_NAME.svg"

cat > "$APPDIR/AppRun" <<'APPRUN_EOF'
#!/bin/sh
# Nspawn Studio AppImage entry point.
HERE="$(dirname "$(readlink -f "$0")")"
# Prefer Wayland when available, fall back to X11.
if [ -z "${GDK_BACKEND:-}" ]; then
  if [ -n "${WAYLAND_DISPLAY:-}" ]; then
    export GDK_BACKEND="wayland,x11"
  else
    export GDK_BACKEND="x11,wayland"
  fi
fi
exec "$HERE/usr/bin/nspawn-studio" "$@"
APPRUN_EOF
chmod 0755 "$APPDIR/AppRun"

echo "==> Fetching appimagetool for $HOST_ARCH"
mkdir -p "$TOOLS_DIR"
if [ ! -x "$TOOL" ]; then
  if command -v wget >/dev/null 2>&1; then
    wget -q -O "$TOOL" "$TOOL_URL"
  else
    curl -fL -o "$TOOL" "$TOOL_URL"
  fi
  chmod 0755 "$TOOL"
fi

# --appimage-extract-and-run avoids depending on FUSE. When cross packaging we
# embed the target architecture's AppImage runtime instead of running a
# foreign appimagetool.
CMD=("$TOOL" --appimage-extract-and-run --no-appstream)
if [ "$ARCH" != "$HOST_ARCH" ]; then
  echo "==> Fetching the $ARCH AppImage runtime"
  if [ ! -f "$TARGET_RUNTIME" ]; then
    if command -v wget >/dev/null 2>&1; then
      wget -q -O "$TARGET_RUNTIME" "$RUNTIME_URL"
    else
      curl -fL -o "$TARGET_RUNTIME" "$RUNTIME_URL"
    fi
    chmod 0755 "$TARGET_RUNTIME"
  fi
  CMD+=(--runtime-file "$TARGET_RUNTIME")
fi

echo "==> Building the AppImage"
mkdir -p "$OUT_DIR"
export ARCH
"${CMD[@]}" "$APPDIR" "$OUT"

echo
echo "==> AppImage written to: $OUT"
