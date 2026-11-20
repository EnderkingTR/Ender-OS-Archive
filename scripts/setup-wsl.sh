#!/usr/bin/env bash
# Ender-OS — WSL2 Ubuntu Geliştirme Ortamı Kurulum Betiği
# Linux çekirdeği derleme araçları, statik Rust (musl) hedefi ve QEMU kurulumunu yapar.

set -euo pipefail

echo "=========================================="
echo "  Ender-OS Ortam Kurulumu Başlıyor"
echo "=========================================="

# 1. Linux ortamında çalışıldığını doğrula
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "HATA: Bu betik yalnızca Linux / WSL2 Ubuntu içinde çalıştırılmalıdır." >&2
    exit 1
fi

# 2. Paket listesini güncelle ve çekirdek + QEMU bağımlılıklarını kur
echo "[1/4] Linux çekirdeği derleme araçları, musl ve QEMU kuruluyor..."
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    bc \
    bison \
    flex \
    libssl-dev \
    libelf-dev \
    libncurses-dev \
    dwarves \
    zstd \
    cpio \
    gzip \
    xz-utils \
    wget \
    curl \
    git \
    pkg-config \
    musl-tools \
    qemu-system-x86

# 3. Rust toolchain (rustup) kurulumu
RUST_JUST_INSTALLED=false
if ! command -v rustup >/dev/null 2>&1; then
    echo "[2/4] rustup bulunamadı; Rust kuruluyor..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    RUST_JUST_INSTALLED=true
else
    echo "[2/4] rustup zaten kurulu, atlanıyor."
fi

# PATH ayarını mevcut kabuğa yükle
if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

# 4. Statik Linux hedefini (musl) ekle
echo "[3/4] Statik Rust hedefi (x86_64-unknown-linux-musl) ekleniyor..."
rustup target add x86_64-unknown-linux-musl

# 5. WSL2 native ext4 derleme dizinlerini oluştur
# Not: Kaynak kod Windows NTFS (/mnt/c/...) üzerinde durur; ağır derlemeler hız ve
# büyük/küçük harf duyarlılığı (Linux Kbuild) için ext4 üzerindeki ~/ender-os-build altında yapılır.
echo "[4/4] Yerel ext4 derleme dizinleri hazırlanıyor: $HOME/ender-os-build"
mkdir -p "$HOME/ender-os-build"/{kernel,initramfs,cargo-target,out}

# 6. Ortam doğrulama özeti
echo ""
echo "=========================================="
echo "  Kurulum Özeti ve Doğrulama"
echo "=========================================="
echo "Çekirdek  : $(uname -r)"
echo "GCC       : $(gcc --version | head -n1)"
echo "Rustc     : $(rustc --version)"
echo "Cargo     : $(cargo --version)"
echo "Hedefler  : $(rustup target list --installed | tr '\n' ' ')"
echo "QEMU      : $(qemu-system-x86_64 --version | head -n1)"
echo "Derleme   : $HOME/ender-os-build"
echo "=========================================="

if [[ "$RUST_JUST_INSTALLED" == "true" ]]; then
    echo ""
    echo "ÖNEMLİ: Mevcut terminalde rustc/cargo komutlarının tanınması için şunu çalıştırın:"
    echo "  source \"\$HOME/.cargo/env\""
    echo "(veya Ubuntu terminalini kapatıp yeniden açın.)"
fi

echo ""
echo "Faz 0 kurulumu başarıyla tamamlandı!"
