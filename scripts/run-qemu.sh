#!/usr/bin/env bash
# Ender-OS — QEMU Sanal Makinesinde Çalıştırma Betiği (VirtIO-Net + VirtIO-Blk + Web Panel)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"
BZIMAGE="$BUILD_ROOT/out/bzImage"
INITRAMFS="$BUILD_ROOT/out/initramfs.cpio.gz"
DATA_IMG="$BUILD_ROOT/out/data.img"
HTTP_PORT="${ENDER_HTTP_PORT:-8080}"

if [[ ! -f "$BZIMAGE" ]]; then
    echo "HATA: Çekirdek imajı bulunamadı: $BZIMAGE" >&2
    echo "Önce build-kernel.sh betiğini çalıştırın." >&2
    exit 1
fi

if [[ ! -f "$INITRAMFS" ]]; then
    echo "HATA: initramfs imajı bulunamadı: $INITRAMFS" >&2
    echo "Önce build-initramfs.sh betiğini çalıştırın." >&2
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

KERNEL_CMDLINE="${KERNEL_CMDLINE:-console=ttyS0 quiet loglevel=3}"

echo "Ender-OS QEMU'da başlatılıyor..."
echo "Web Dashboard : http://localhost:${HTTP_PORT}"
if [[ "${NO_DISK:-0}" != "1" ]]; then
    echo "Kalıcı Disk   : $DATA_IMG -> /data (ext4)"
fi
echo "Çıkmak için 'poweroff' yazın veya Ctrl-A ardından X tuşlarına basın."
echo ""

exec qemu-system-x86_64 \
    "${KVM_ARGS[@]}" \
    "${DISK_ARGS[@]}" \
    "${NET_ARGS[@]}" \
    -kernel "$BZIMAGE" \
    -initrd "$INITRAMFS" \
    -m "${QEMU_MEM:-256M}" \
    -nographic \
    -append "$KERNEL_CMDLINE"
