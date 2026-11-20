#!/usr/bin/env bash
# Ender-OS v0.6 — Gerçek glibc + pacman + systemd Kalıcı Sistemini QEMU'da Başlatma

set -euo pipefail

BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"
BZIMAGE="$BUILD_ROOT/out/bzImage"
INITRAMFS="$BUILD_ROOT/out/initramfs.cpio.gz"
ROOT_IMG="$BUILD_ROOT/out/root.img"
HTTP_PORT="${ENDER_HTTP_PORT:-8080}"

for f in "$BZIMAGE" "$INITRAMFS" "$ROOT_IMG"; do
    if [[ ! -f "$f" ]]; then
        echo "HATA: Gerekli imaj bulunamadı: $f" >&2
        exit 1
    fi
done

KVM_ARGS=()
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    KVM_ARGS=(-enable-kvm -cpu host)
fi

if [[ "${GUI:-0}" == "1" ]]; then
    DISPLAY_ARGS=(-serial stdio)
    if [[ "${AUDIO:-0}" == "1" ]]; then
        AUDIO_ARGS=(-device ich9-intel-hda -device hda-duplex)
    else
        AUDIO_ARGS=(-audiodev none,id=snd0 -device ich9-intel-hda -device hda-duplex,audiodev=snd0)
    fi
    CMDLINE="console=ttyS0 console=tty0 root=/dev/vda rw loglevel=3"
else
    DISPLAY_ARGS=(-nographic)
    AUDIO_ARGS=(-audiodev none,id=snd0 -device ich9-intel-hda -device hda-duplex,audiodev=snd0)
    CMDLINE="console=tty0 console=ttyS0 root=/dev/vda rw quiet loglevel=3"
fi

echo "Ender-OS v0.6 (Desktop/Gaming Tabanı) QEMU'da başlatılıyor..."
echo "Web Dashboard : http://localhost:${HTTP_PORT}"
echo "Kalıcı Kök    : $ROOT_IMG (/dev/vda -> /)"
echo ""

exec qemu-system-x86_64 \
    "${KVM_ARGS[@]}" \
    -drive "file=$ROOT_IMG,format=raw,if=virtio" \
    -netdev "user,id=net0,hostfwd=tcp::${HTTP_PORT}-:8080" \
    -device virtio-net-pci,netdev=net0 \
    -vga virtio \
    -device virtio-tablet-pci \
    "${AUDIO_ARGS[@]}" \
    -kernel "$BZIMAGE" \
    -initrd "$INITRAMFS" \
    -m "${QEMU_MEM:-2048M}" \
    "${DISPLAY_ARGS[@]}" \
    -append "$CMDLINE"
