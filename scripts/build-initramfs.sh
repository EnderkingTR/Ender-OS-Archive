#!/usr/bin/env bash
# Ender-OS — Rust Userspace Workspace Derleme ve initramfs.cpio.gz Paketleme Betiği

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:$PATH"

USERSPACE_SRC="$BUILD_ROOT/userspace-src"
CARGO_TARGET="$BUILD_ROOT/cargo-target"
ROOTFS="$BUILD_ROOT/initramfs/root"
OUT_DIR="$BUILD_ROOT/out"
RELEASE_DIR="$CARGO_TARGET/x86_64-unknown-linux-musl/release"

mkdir -p "$CARGO_TARGET" "$OUT_DIR"

echo "=========================================="
echo "  Ender-OS v0.4 initramfs Oluşturuluyor"
echo "=========================================="

# 1. Kaynak dosyaları WSL2 yerel ext4 dizinine eşitle (Cargo.lock önbelleğini korur)
echo "[1/4] Rust userspace kaynakları hazırlanıyor..."
if [[ -f "$USERSPACE_SRC/Cargo.lock" ]]; then
    cp "$USERSPACE_SRC/Cargo.lock" "$BUILD_ROOT/Cargo.lock.bak"
fi
rm -rf "$USERSPACE_SRC"
cp -a "$REPO_DIR/userspace" "$USERSPACE_SRC"
if [[ -f "$BUILD_ROOT/Cargo.lock.bak" ]]; then
    mv "$BUILD_ROOT/Cargo.lock.bak" "$USERSPACE_SRC/Cargo.lock"
fi

# 2. Tüm workspace paketlerini statik musl ikili dosyaları olarak derle
echo "[2/4] Rust workspace derleniyor (x86_64-unknown-linux-musl)..."
CARGO_TARGET_DIR="$CARGO_TARGET" cargo build \
    --manifest-path "$USERSPACE_SRC/Cargo.toml" \
    --workspace \
    --release \
    --target x86_64-unknown-linux-musl

BINARIES=(ender-init ender-sh ender-utils ender-httpd)
for bin in "${BINARIES[@]}"; do
    BIN_PATH="$RELEASE_DIR/$bin"
    if [[ ! -f "$BIN_PATH" ]]; then
        echo "HATA: Derlenen binary bulunamadı: $BIN_PATH" >&2
        exit 1
    fi
    file "$BIN_PATH"
    ls -lh "$BIN_PATH"
done

# 3. FHS uyumlu minimal initramfs kök dosya sistemini hazırla
echo "[3/4] initramfs kök dosya sistemi ve sembolik linkler oluşturuluyor..."
rm -rf "$ROOTFS"
mkdir -p "$ROOTFS"/{bin,sbin,etc,root,proc,sys,dev,tmp,run,var/log,data,lib/modules}

cp "$RELEASE_DIR/ender-init" "$ROOTFS/init"
cp "$RELEASE_DIR/ender-sh" "$ROOTFS/bin/ender-sh"
cp "$RELEASE_DIR/ender-utils" "$ROOTFS/bin/ender-utils"
cp "$RELEASE_DIR/ender-httpd" "$ROOTFS/bin/ender-httpd"
chmod 0755 \
    "$ROOTFS/init" \
    "$ROOTFS/bin/ender-sh" \
    "$ROOTFS/bin/ender-utils" \
    "$ROOTFS/bin/ender-httpd"

ln -s ender-sh "$ROOTFS/bin/sh"

BIN_APPLETS=(
    ls cat ps uname uptime meminfo echo mkdir rm touch head grep clear pwd
    lsmod dmesg enderctl ip fetch svc df
)
for app in "${BIN_APPLETS[@]}"; do
    ln -s ender-utils "$ROOTFS/bin/$app"
done

SBIN_APPLETS=(poweroff reboot insmod rmmod)
for app in "${SBIN_APPLETS[@]}"; do
    ln -s ../bin/ender-utils "$ROOTFS/sbin/$app"
done

if [[ -f "$OUT_DIR/ender_dev.ko" ]]; then
    cp "$OUT_DIR/ender_dev.ko" "$ROOTFS/lib/modules/ender_dev.ko"
    ls -lh "$ROOTFS/lib/modules/ender_dev.ko"
else
    echo "UYARI: $OUT_DIR/ender_dev.ko bulunamadı; initramfs modülsüz paketleniyor."
fi

cat > "$ROOTFS/etc/os-release" <<'EOF'
NAME="Ender-OS"
VERSION="0.4"
ID=enderos
PRETTY_NAME="Ender-OS v0.4"
EOF

# 4. newc cpio + gzip arşivi olarak paketle
echo "[4/4] initramfs.cpio.gz paketleniyor..."
(
    cd "$ROOTFS"
    find . -print0 | cpio --null -ov --format=newc --owner=0:0 2>/dev/null | gzip -9 > "$OUT_DIR/initramfs.cpio.gz"
)

ls -lh "$OUT_DIR/initramfs.cpio.gz"
echo "initramfs başarıyla oluşturuldu: $OUT_DIR/initramfs.cpio.gz"
