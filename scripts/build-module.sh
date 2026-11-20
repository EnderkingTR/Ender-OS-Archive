#!/usr/bin/env bash
# Ender-OS — Rust-for-Linux Çekirdek Modülü (ender_dev.ko) Derleme Betiği

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:$PATH"

KERNEL_BUILD="$BUILD_ROOT/kernel/build"
MOD_SRC="$REPO_DIR/kernel/modules/ender_dev"
MOD_BUILD="$BUILD_ROOT/modules/ender_dev"
OUT_DIR="$BUILD_ROOT/out"

if [[ ! -f "$KERNEL_BUILD/Module.symvers" ]]; then
    echo "HATA: Çekirdek derleme çıktıları bulunamadı ($KERNEL_BUILD/Module.symvers)." >&2
    echo "Önce build-kernel.sh betiğini çalıştırın." >&2
    exit 1
fi

mkdir -p "$BUILD_ROOT/modules" "$OUT_DIR"

echo "=========================================="
echo "  Ender-OS Çekirdek Modülü Derleniyor (ender_dev.ko)"
echo "=========================================="

rm -rf "$MOD_BUILD"
cp -a "$MOD_SRC" "$MOD_BUILD"

make -C "$KERNEL_BUILD" M="$MOD_BUILD" ARCH=x86_64 LLVM=1 modules

if [[ ! -f "$MOD_BUILD/ender_dev.ko" ]]; then
    echo "HATA: ender_dev.ko oluşturulamadı." >&2
    exit 1
fi

cp "$MOD_BUILD/ender_dev.ko" "$OUT_DIR/ender_dev.ko"
file "$OUT_DIR/ender_dev.ko"
ls -lh "$OUT_DIR/ender_dev.ko"
strings "$OUT_DIR/ender_dev.ko" | grep -E "^(license|description|vermagic|name)=" || true
echo "Modül başarıyla derlendi: $OUT_DIR/ender_dev.ko"
