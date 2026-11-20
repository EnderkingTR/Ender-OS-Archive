# Faz 6B: Wayland Grafik Masaüstü, Mesa 3D, PipeWire ve `ender-hud`

Bu fazda Ender-OS `root.img` üzerine **Mesa (OpenGL + Vulkan)**, **Wayland (`labwc`)**, **Waybar**, **Foot Terminal**, **Wofi Uygulama Başlatıcı**, **PipeWire Ses** ve senin kendi Rust telemetri aracın **`ender-hud`** kurulur.

## Kurulum (Tek Komut)

`run-desktop-qemu.sh` içinden (veya aşağıdaki otomatik betikle) `ender-setup-desktop` çalıştırılır:

```bash
/usr/local/bin/ender-setup-desktop
```

## Grafik Masaüstünü Açma

```bash
GUI=1 bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/run-desktop-qemu.sh"
```

- `tty1` üzerinde otomatik `ender` kullanıcısıyla `labwc` Wayland masaüstü açılır.
- Üst çubukta (`waybar`) senin Rust **`ender-hud`** aracın canlı CPU/RAM ve `GAME`/`DEV` modunu gösterir.
- Kısayollar: **`Alt + Enter`** → Terminal (`foot`), **`Alt + D`** → Uygulama Menüsü (`wofi`).
