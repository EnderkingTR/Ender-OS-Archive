#!/usr/bin/env bash
# Ender-OS — Linux Çekirdeği (bzImage + modül altyapısı) Derleme Betiği
# Kaynak ve derleme dosyaları hız ile Kbuild uyumluluğu için $HOME/ender-os-build altında tutulur.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:$PATH"

for tool in clang lld bindgen rustc; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "HATA: Rust-for-Linux çekirdeği için gerekli araç eksik: $tool" >&2
        echo "Kurulum: sudo apt-get install -y clang lld llvm bindgen libclang-dev && rustup component add rust-src" >&2
        exit 1
    fi
done

KERNEL_VERSION="${KERNEL_VERSION:-6.18.64}"
KERNEL_MAJOR="${KERNEL_VERSION%%.*}"
TARBALL_URL="https://cdn.kernel.org/pub/linux/kernel/v${KERNEL_MAJOR}.x/linux-${KERNEL_VERSION}.tar.xz"

KERNEL_DIR="$BUILD_ROOT/kernel"
TARBALL="$KERNEL_DIR/linux-${KERNEL_VERSION}.tar.xz"
KERNEL_SRC="$KERNEL_DIR/linux-${KERNEL_VERSION}"
KERNEL_BUILD="$KERNEL_DIR/build"
CONFIG_SRC="$REPO_DIR/kernel/ender_x86_64.config"
CONFIG_LOCAL="$KERNEL_DIR/ender_x86_64.config"
OUT_DIR="$BUILD_ROOT/out"

mkdir -p "$KERNEL_DIR" "$OUT_DIR"

# GCC'den LLVM=1 derlemesine geçişte veya CLEAN=1 verildiğinde eski build dizinini temizle
if [[ "${CLEAN:-0}" == "1" ]] || { [[ -f "$KERNEL_BUILD/.config" ]] && ! grep -q "^CONFIG_CC_IS_CLANG=y$" "$KERNEL_BUILD/.config"; }; then
    echo "Bilgi: Eski çekirdek derleme dizini temizleniyor (LLVM=1 geçişi)..."
    rm -rf "$KERNEL_BUILD"
fi
mkdir -p "$KERNEL_BUILD"

echo "=========================================="
echo "  Ender-OS Çekirdek Derleme (Linux $KERNEL_VERSION, LLVM=1 + Rust)"
echo "=========================================="

# 1. Çekirdek arşivini indir (zaten varsa atla)
if [[ ! -f "$TARBALL" ]]; then
    echo "[1/5] Linux $KERNEL_VERSION kaynak arşivi indiriliyor..."
    curl -fL --retry 3 -o "${TARBALL}.part" "$TARBALL_URL"
    mv "${TARBALL}.part" "$TARBALL"
else
    echo "[1/5] Çekirdek arşivi zaten önbellekte: $TARBALL"
fi

# 2. Arşivi ext4 üzerine aç (zaten açılmışsa atla)
if [[ ! -d "$KERNEL_SRC" ]]; then
    echo "[2/5] Çekirdek kaynak kodu açılıyor..."
    tar -xf "$TARBALL" -C "$KERNEL_DIR"
else
    echo "[2/5] Çekirdek kaynak kodu zaten hazır: $KERNEL_SRC"
fi

# 3. Yapılandırmayı boşluk içermeyen yola kopyala ve allnoconfig üzerine birleştir
echo "[3/5] Çekirdek yapılandırması uygulanıyor..."
cp "$CONFIG_SRC" "$CONFIG_LOCAL"

make -C "$KERNEL_SRC" O="$KERNEL_BUILD" ARCH=x86_64 LLVM=1 allnoconfig
ARCH=x86_64 LLVM=1 "$KERNEL_SRC/scripts/kconfig/merge_config.sh" \
    -m -O "$KERNEL_BUILD" \
    "$KERNEL_BUILD/.config" \
    "$CONFIG_LOCAL"
make -C "$KERNEL_SRC" O="$KERNEL_BUILD" ARCH=x86_64 LLVM=1 olddefconfig

# Kritik seçeneklerin etkin olduğunu doğrula
REQUIRED_OPTS=(
    CONFIG_64BIT
    CONFIG_BINFMT_ELF
    CONFIG_BLK_DEV_INITRD
    CONFIG_RD_GZIP
    CONFIG_PROC_FS
    CONFIG_SYSFS
    CONFIG_TMPFS
    CONFIG_DEVTMPFS
    CONFIG_DEVTMPFS_MOUNT
    CONFIG_TTY
    CONFIG_SERIAL_8250
    CONFIG_SERIAL_8250_CONSOLE
    CONFIG_PCI
    CONFIG_ACPI
    CONFIG_MODULES
    CONFIG_MODULE_UNLOAD
    CONFIG_RUST
    CONFIG_NET
    CONFIG_INET
    CONFIG_VIRTIO_PCI
    CONFIG_VIRTIO_NET
    CONFIG_BLOCK
    CONFIG_VIRTIO_BLK
    CONFIG_EXT4_FS
    CONFIG_EFI
    CONFIG_VT
    CONFIG_VT_CONSOLE
    CONFIG_FB
    CONFIG_FB_DEVICE
    CONFIG_FB_SIMPLE
    CONFIG_FRAMEBUFFER_CONSOLE
    CONFIG_INPUT_KEYBOARD
    CONFIG_KEYBOARD_ATKBD
    CONFIG_DRM
    CONFIG_DRM_VIRTIO_GPU
    CONFIG_INPUT_EVDEV
    CONFIG_SND_HDA_INTEL
    CONFIG_SND_HDA_GENERIC
    CONFIG_CGROUPS
    CONFIG_USER_NS
    CONFIG_OVERLAY_FS
)

for opt in "${REQUIRED_OPTS[@]}"; do
    if ! grep -q "^${opt}=y$" "$KERNEL_BUILD/.config"; then
        echo "HATA: Gerekli çekirdek seçeneği eksik: ${opt}=y" >&2
        exit 1
    fi
done

# 4. Çekirdeği ve modül altyapısını derle (bzImage + Module.symvers + libkernel.rlib)
echo "[4/5] Linux çekirdeği ve Rust altyapısı derleniyor (LLVM=1 -j$(nproc))..."
make -C "$KERNEL_SRC" O="$KERNEL_BUILD" ARCH=x86_64 LLVM=1 -j"$(nproc)" bzImage modules

# 5. Çıktıyı kopyala ve boyutunu yazdır
echo "[5/5] bzImage çıktı dizinine kopyalanıyor..."
cp "$KERNEL_BUILD/arch/x86/boot/bzImage" "$OUT_DIR/bzImage"
ls -lh "$OUT_DIR/bzImage"

echo "Çekirdek derlemesi tamamlandı: $OUT_DIR/bzImage"
