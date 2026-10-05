#!/usr/bin/env bash
# =============================================================================
#  Build an aarch64 (arm64) AppImage on a Debian/Ubuntu x86_64 host.
#
#  Usage:
#    sudo ./scripts/build-arm64.sh
#
#  This is a cross build: Rust compiles an aarch64 binary on the x86_64 host
#  using the Debian arm64 multiarch headers/libraries. qemu-user binfmt is
#  installed so the aarch64 appimagetool can run (and so the produced binary
#  can be smoke tested).
#
#  Requires network access and root (dpkg --add-architecture).
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ "$(id -u)" -ne 0 ]; then
  echo "build-arm64.sh: run this as root (it enables the arm64 architecture)" >&2
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive
export PATH="$HOME/.cargo/bin:$PATH"

echo "==> Enabling the arm64 architecture"
if ! dpkg --print-foreign-architectures | grep -qx arm64; then
  dpkg --add-architecture arm64
fi
apt-get update

echo "==> Installing the host cross toolchain and qemu"
apt-get install -y --no-install-recommends \
  gcc-aarch64-linux-gnu qemu-user-static binfmt-support \
  wget curl file patchelf squashfs-tools desktop-file-utils

echo "==> Installing the arm64 GTK4/libadwaita development libraries"
apt-get install -y --no-install-recommends \
  libgtk-4-dev:arm64 libadwaita-1-dev:arm64

echo "==> Ensuring a Rust toolchain with the aarch64 target"
if ! command -v rustup >/dev/null 2>&1; then
  wget -q -O /tmp/rustup-init \
    https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
  chmod +x /tmp/rustup-init
  RUSTUP_HOME="$HOME/.rustup" CARGO_HOME="$HOME/.cargo" \
    /tmp/rustup-init -y --profile minimal --default-toolchain stable
  . "$HOME/.cargo/env"
fi
rustup target add aarch64-unknown-linux-gnu

echo "==> Configuring cross pkg-config"
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR=/
export PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

echo "==> Compiling for aarch64"
cargo build --release --target aarch64-unknown-linux-gnu -p nspawn-studio

BIN="target/aarch64-unknown-linux-gnu/release/nspawn-studio"
file "$BIN"

echo "==> Smoke testing the aarch64 binary under qemu"
if command -v qemu-aarch64-static >/dev/null 2>&1; then
  if qemu-aarch64-static "$BIN" --version; then
    echo "     qemu smoke test: OK"
  else
    echo "     qemu smoke test: could not execute (continuing)"
  fi
fi

echo "==> Building the aarch64 AppImage"
ARCH=aarch64 TARGET=aarch64-unknown-linux-gnu ./scripts/build-appimage.sh

echo
echo "==> Done. The aarch64 AppImage is in dist/."
