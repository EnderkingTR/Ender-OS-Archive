#!/usr/bin/env bash
# Ender-OS — Kalıcı ext4 Sanal Disk (data.img) Oluşturma Betiği

set -euo pipefail

BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"
OUT_DIR="$BUILD_ROOT/out"
DISK_IMG="$OUT_DIR/data.img"
DISK_SIZE_MB="${ENDER_DISK_MB:-16}"

mkdir -p "$OUT_DIR"

if [[ -f "$DISK_IMG" && "${FORCE:-0}" != "1" ]]; then
    echo "Kalıcı disk imajı zaten mevcut: $DISK_IMG"
    ls -lh "$DISK_IMG"
    exit 0
fi

echo "Kalıcı ext4 disk imajı oluşturuluyor (${DISK_SIZE_MB}M): $DISK_IMG"
rm -f "$DISK_IMG"
truncate -s "${DISK_SIZE_MB}M" "$DISK_IMG"
/usr/sbin/mkfs.ext4 -q -F -L ender-data "$DISK_IMG"

ls -lh "$DISK_IMG"
echo "Disk imajı hazır: $DISK_IMG"
