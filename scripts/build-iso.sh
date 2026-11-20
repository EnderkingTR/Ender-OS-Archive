#!/usr/bin/env bash
# Ender-OS — Hibrit BIOS + UEFI Önyüklenebilir GRUB .iso İmajı Oluşturma Betiği

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"

for tool in grub-mkrescue xorriso mformat; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "HATA: .iso oluşturmak için gerekli araç eksik: $tool" >&2
        echo "Kurulum: sudo apt-get install -y grub-common grub-pc-bin grub-efi-amd64-bin xorriso mtools" >&2
        exit 1
    fi
done

OUT_DIR="$BUILD_ROOT/out"
BZIMAGE="$OUT_DIR/bzImage"
INITRAMFS="$OUT_DIR/initramfs.cpio.gz"
ISO_ROOT="$BUILD_ROOT/iso-root"
ISO_NAME="Ender-OS-v0.5.iso"
ISO_OUT="$OUT_DIR/$ISO_NAME"
REPO_OUT="$REPO_DIR/build/$ISO_NAME"

if [[ ! -f "$BZIMAGE" ]]; then
    echo "HATA: Çekirdek bulunamadı: $BZIMAGE (önce build-kernel.sh çalıştırın)" >&2
    exit 1
fi
if [[ ! -f "$INITRAMFS" ]]; then
    echo "HATA: initramfs bulunamadı: $INITRAMFS (önce build-initramfs.sh çalıştırın)" >&2
    exit 1
fi

echo "=========================================="
echo "  Ender-OS v0.5 ISO İmajı Oluşturuluyor"
echo "=========================================="

rm -rf "$ISO_ROOT"
mkdir -p "$ISO_ROOT/boot/grub" "$REPO_DIR/build"

cp "$BZIMAGE" "$ISO_ROOT/boot/bzImage"
cp "$INITRAMFS" "$ISO_ROOT/boot/initramfs.cpio.gz"
cp "$REPO_DIR/boot/grub/grub.cfg" "$ISO_ROOT/boot/grub/grub.cfg"

grub-mkrescue -o "$ISO_OUT" "$ISO_ROOT"

cp "$ISO_OUT" "$REPO_OUT"
file "$ISO_OUT"
ls -lh "$ISO_OUT" "$REPO_OUT"
echo "Önyüklenebilir ISO hazır: $REPO_OUT"
