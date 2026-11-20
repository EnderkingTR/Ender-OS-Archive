#!/usr/bin/env bash
# Ender-OS — Gerçek glibc + pacman + systemd Kalıcı Kök Disk (root.img) Oluşturma Betiği

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_ROOT="${ENDER_BUILD_ROOT:-$HOME/ender-os-build}"

BOOTSTRAP_URL="https://geo.mirror.pkgbuild.com/iso/latest/archlinux-bootstrap-x86_64.tar.zst"
ROOTFS_CACHE="$BUILD_ROOT/rootfs"
TARBALL="$ROOTFS_CACHE/archlinux-bootstrap-x86_64.tar.zst"
ROOTFS_DIR="$ROOTFS_CACHE/root.x86_64"
OUT_DIR="$BUILD_ROOT/out"
ROOT_IMG="$OUT_DIR/root.img"
DISK_SIZE_MB="${ENDER_ROOT_MB:-12288}"

mkdir -p "$ROOTFS_CACHE" "$OUT_DIR"

echo "=========================================="
echo "  Ender-OS v0.6 Rootfs (glibc + pacman) Hazırlanıyor"
echo "=========================================="

# 1. Resmi Arch Linux bootstrap arşivini indir (önbellekte varsa atla)
if [[ ! -f "$TARBALL" ]]; then
    echo "[1/4] Arch Linux bootstrap arşivi indiriliyor (~127 MB)..."
    curl -fL --retry 3 -o "${TARBALL}.part" "$BOOTSTRAP_URL"
    mv "${TARBALL}.part" "$TARBALL"
else
    echo "[1/4] Bootstrap arşivi zaten önbellekte: $TARBALL"
fi

# 2. Kök dizini temizleyip çıkar (sudo gerekmeden --no-same-owner + --delay-directory-restore ile)
echo "[2/4] Bootstrap arşivi açılıyor..."
if [[ -d "$ROOTFS_DIR" ]]; then
    chmod -R u+rwX "$ROOTFS_DIR" 2>/dev/null || true
    rm -rf "$ROOTFS_DIR"
fi
tar --no-same-owner --delay-directory-restore -I zstd -xf "$TARBALL" -C "$ROOTFS_CACHE"
chmod -R u+rwX "$ROOTFS_DIR"

# 3. Ender-OS yapılandırması, ağ, autologin ve özel araçları yerleştir
echo "[3/4] Ender-OS sistem yapılandırması uygulanıyor..."
mkdir -p \
    "$ROOTFS_DIR/etc/systemd/network" \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants" \
    "$ROOTFS_DIR/etc/systemd/system/sockets.target.wants" \
    "$ROOTFS_DIR/etc/systemd/system/serial-getty@ttyS0.service.d" \
    "$ROOTFS_DIR/etc/systemd/system/getty@tty1.service.d" \
    "$ROOTFS_DIR/usr/local/bin" \
    "$ROOTFS_DIR/lib/modules" \
    "$ROOTFS_DIR/data"

echo "ender-os" > "$ROOTFS_DIR/etc/hostname"
: > "$ROOTFS_DIR/etc/machine-id"
ln -sf /usr/share/zoneinfo/Europe/Istanbul "$ROOTFS_DIR/etc/localtime"
echo "LANG=C.UTF-8" > "$ROOTFS_DIR/etc/locale.conf"
echo "KEYMAP=trq" > "$ROOTFS_DIR/etc/vconsole.conf"

cat > "$ROOTFS_DIR/etc/os-release" <<'EOF'
NAME="Ender-OS"
PRETTY_NAME="Ender-OS v0.6 (Gaming & Dev Edition)"
ID=enderos
ID_LIKE=arch
VERSION_ID="0.6"
EOF

cat > "$ROOTFS_DIR/etc/hosts" <<'EOF'
127.0.0.1   localhost ender-os
::1         localhost ender-os
EOF

cat > "$ROOTFS_DIR/etc/resolv.conf" <<'EOF'
nameserver 10.0.2.3
nameserver 1.1.1.1
EOF

# Hızlı pacman yansısı ve Gaming (32-bit multilib) deposunu aç
cat > "$ROOTFS_DIR/etc/pacman.d/mirrorlist" <<'EOF'
Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch
EOF

if grep -q "^#\[multilib\]" "$ROOTFS_DIR/etc/pacman.conf"; then
    sed -i '/^#\[multilib\]/,/^#Include =/{s/^#//}' "$ROOTFS_DIR/etc/pacman.conf"
fi

if grep -q "^#DisableSandbox" "$ROOTFS_DIR/etc/pacman.conf"; then
    sed -i 's/^#DisableSandbox/DisableSandbox/' "$ROOTFS_DIR/etc/pacman.conf"
elif ! grep -q "^DisableSandbox" "$ROOTFS_DIR/etc/pacman.conf"; then
    sed -i '/^\[options\]/a DisableSandbox' "$ROOTFS_DIR/etc/pacman.conf"
fi

# QEMU SLIRP VirtIO-Net için systemd-networkd ayarı
cat > "$ROOTFS_DIR/etc/systemd/network/20-virtio.network" <<'EOF'
[Match]
Name=en* eth*

[Network]
Address=10.0.2.15/24
Gateway=10.0.2.2
DNS=10.0.2.3
EOF

ln -sf /usr/lib/systemd/system/systemd-networkd.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/systemd-networkd.service"

# Geliştirme kolaylığı için tty1 ve ttyS0 üzerinde otomatik root oturumu
cat > "$ROOTFS_DIR/etc/systemd/system/serial-getty@ttyS0.service.d/autologin.conf" <<'EOF'
[Service]
ExecStart=
ExecStart=-/sbin/agetty -o '-p -f -- \\u' --keep-baud 115200,57600,38400,9600 --autologin root %I $TERM
EOF

cat > "$ROOTFS_DIR/etc/systemd/system/getty@tty1.service.d/autologin.conf" <<'EOF'
[Service]
ExecStart=
ExecStart=-/sbin/agetty -o '-p -f -- \\u' --noclear --autologin root %I $TERM
EOF

# Ender-OS özel Rust araçlarını ve çekirdek modülünü yerleştir
RELEASE_DIR="$BUILD_ROOT/cargo-target/x86_64-unknown-linux-musl/release"
for bin in ender-httpd ender-utils ender-fb ender-hud; do
    if [[ -f "$RELEASE_DIR/$bin" ]]; then
        cp "$RELEASE_DIR/$bin" "$ROOTFS_DIR/usr/local/bin/$bin"
        chmod 0755 "$ROOTFS_DIR/usr/local/bin/$bin"
    fi
done

cat > "$ROOTFS_DIR/usr/local/bin/ender-setup-desktop" <<'SETUP_EOF'
#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "  Ender-OS v0.6 Masaüstü Kurulumu (Wayland + Mesa + PipeWire)"
echo "=========================================="

pacman-key --init
pacman-key --populate archlinux

pacman -Syu --noconfirm --needed \
    sudo git neovim btop \
    mesa vulkan-virtio vulkan-icd-loader \
    wayland xorg-xwayland seatd \
    labwc waybar foot wofi swaybg \
    pipewire pipewire-pulse wireplumber \
    ttf-dejavu

if ! id ender >/dev/null 2>&1; then
    useradd -m -G wheel,video,render,audio,input,seat -s /bin/bash ender
    passwd -d ender
fi

echo "%wheel ALL=(ALL:ALL) NOPASSWD: ALL" > /etc/sudoers.d/10-ender-wheel
chmod 0440 /etc/sudoers.d/10-ender-wheel

systemctl enable seatd.service

mkdir -p /home/ender/.config/{labwc,waybar,foot}

cat > /home/ender/.config/labwc/autostart <<'AUTOSTART'
swaybg -c '#0b1020' >/dev/null 2>&1 &
waybar >/dev/null 2>&1 &
foot >/dev/null 2>&1 &
AUTOSTART

cat > /home/ender/.config/labwc/rc.xml <<'RCXML'
<?xml version="1.0"?>
<labwc_config>
  <theme>
    <cornerRadius>8</cornerRadius>
  </theme>
  <keyboard>
    <default />
    <keybind key="A-Return">
      <action name="Execute" command="foot" />
    </keybind>
    <keybind key="A-d">
      <action name="Execute" command="wofi --show drun" />
    </keybind>
  </keyboard>
</labwc_config>
RCXML

cat > /home/ender/.config/waybar/config.jsonc <<'WAYBAR_CFG'
{
  "layer": "top",
  "height": 34,
  "modules-left": ["custom/ender-hud"],
  "modules-center": ["clock"],
  "modules-right": ["custom/gamemode"],
  "custom/ender-hud": {
    "exec": "/usr/local/bin/ender-hud waybar",
    "return-type": "json",
    "interval": 2
  },
  "custom/gamemode": {
    "format": "GAME MODE",
    "on-click": "/usr/local/bin/ender-hud gamemode toggle"
  },
  "clock": {
    "format": "{:%H:%M  %d.%m.%Y}"
  }
}
WAYBAR_CFG

cat > /home/ender/.config/waybar/style.css <<'WAYBAR_CSS'
* { font-family: "DejaVu Sans", sans-serif; font-size: 13px; }
window#waybar { background: #111827; color: #e5ecff; border-bottom: 2px solid #38bdf8; }
#custom-ender-hud { padding: 0 14px; color: #38bdf8; font-weight: bold; }
#custom-ender-hud.gamemode { color: #22c55e; }
#custom-gamemode { padding: 0 12px; margin: 4px 8px; background: #1e293b; border-radius: 6px; color: #f59e0b; font-weight: bold; }
#clock { color: #cbd5e1; }
WAYBAR_CSS

cat > /home/ender/.config/foot/foot.ini <<'FOOT_INI'
[main]
font=DejaVu Sans Mono:size=11
[colors]
background=0b1020
foreground=e5ecff
FOOT_INI

cat > /home/ender/.bash_profile <<'BASH_PROF'
if [ -z "${WAYLAND_DISPLAY:-}" ] && [ "$(tty)" = "/dev/tty1" ]; then
    exec labwc
fi
BASH_PROF

chown -R ender:ender /home/ender

mkdir -p /etc/systemd/system/getty@tty1.service.d
cat > /etc/systemd/system/getty@tty1.service.d/autologin.conf <<'TTY1_AUTO'
[Service]
ExecStart=
ExecStart=-/sbin/agetty -o '-p -f -- \\u' --noclear --autologin ender %I $TERM
TTY1_AUTO

echo "Masaüstü kurulumu tamamlandı! 'GUI=1 bash scripts/run-desktop-qemu.sh' ile grafik masaüstünü açabilirsiniz."
SETUP_EOF
chmod 0755 "$ROOTFS_DIR/usr/local/bin/ender-setup-desktop"

if [[ -f "$OUT_DIR/ender_dev.ko" ]]; then
    cp "$OUT_DIR/ender_dev.ko" "$ROOTFS_DIR/lib/modules/ender_dev.ko"
fi

cat > "$ROOTFS_DIR/etc/systemd/system/ender-httpd.service" <<'EOF'
[Unit]
Description=Ender-OS Rust Web Dashboard
After=network.target

[Service]
ExecStart=/usr/local/bin/ender-httpd
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
EOF

ln -sf /etc/systemd/system/ender-httpd.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/ender-httpd.service"

# 4. Root sahipliğinde sparse ext4 disk imajı oluştur
echo "[4/4] Kalıcı ext4 root disk imajı oluşturuluyor (${DISK_SIZE_MB}M): $ROOT_IMG"
rm -f "$ROOT_IMG"
/usr/sbin/mkfs.ext4 -q -F -L ender-root -E root_owner=0:0 -d "$ROOTFS_DIR" "$ROOT_IMG" "${DISK_SIZE_MB}M"

ls -lh "$ROOT_IMG"
echo "Rootfs disk imajı hazır: $ROOT_IMG"
