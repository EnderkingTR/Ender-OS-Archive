#!/usr/bin/env bash
# Ender-OS — Önyüklenebilir .iso İmajını QEMU'da Çalıştırma Betiği

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"
ISO_PATH="${ENDER_ISO_PATH:-$BUILD_ROOT/out/Ender-OS-v0.5.iso}"
DATA_IMG="$BUILD_ROOT/out/data.img"
HTTP_PORT="${ENDER_HTTP_PORT:-8080}"

if [[ ! -f "$ISO_PATH" ]]; then
    echo "HATA: ISO imajı bulunamadı: $ISO_PATH" >&2
    echo "Önce build-iso.sh betiğini çalıştırın." >&2
    exit 1
fi

DISK_ARGS=()
if [[ "${NO_DISK:-0}" != "1" ]]; then
    if [[ ! -f "$DATA_IMG" ]]; then
        bash "$SCRIPT_DIR/build-disk.sh"
    fi
    DISK_ARGS=(-drive "file=$DATA_IMG,format=raw,if=virtio")
fi

NET_ARGS=(
    -netdev "user,id=net0,hostfwd=tcp::${HTTP_PORT}-:8080"
    -device virtio-net-pci,netdev=net0
)

KVM_ARGS=()
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    KVM_ARGS=(-enable-kvm -cpu host)
fi

DISPLAY_ARGS=(-nographic)
if [[ "${GUI:-0}" == "1" ]]; then
    DISPLAY_ARGS=(-serial stdio)
fi

echo "Ender-OS ISO QEMU'da başlatılıyor: $ISO_PATH"
echo "Web Dashboard : http://localhost:${HTTP_PORT}"
if [[ "${GUI:-0}" == "1" ]]; then
    echo "Mod           : Grafik Pencere + Seri Konsol (GUI=1)"
else
    echo "Mod           : Seri Konsol (-nographic, GRUB 2. giriş)"
fi
echo ""

exec qemu-system-x86_64 \
    "${KVM_ARGS[@]}" \
    "${DISK_ARGS[@]}" \
    "${NET_ARGS[@]}" \
    -cdrom "$ISO_PATH" \
    -boot d \
    -vga std \
    -m "${QEMU_MEM:-512M}" \
    "${DISPLAY_ARGS[@]}"
