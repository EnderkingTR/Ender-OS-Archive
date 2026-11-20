# Faz 6A: Gerçek `glibc + pacman + systemd` Tabanı ve `switch_root` (`Ender-OS v0.6`)

Bu fazda Ender-OS, oyun (Steam/Proton) ve günlük yazılım geliştirme (VS Code, Rust, Python, tarayıcı) yazılımlarını çalıştırabilecek gerçek bir **`glibc + pacman + systemd`** kök dosya sistemine (`root.img`) kavuşur.

## `switch_root` Nasıl Çalışır?

1. Çekirdek önce RAM'deki küçük `initramfs` içindeki `ender-init` programını başlatır.
2. `ender-init`, çekirdek komut satırında `root=/dev/vda` parametresini görürse kalıcı `ext4` diskini `/newroot` klasörüne bağlar.
3. `/proc`, `/sys`, `/dev` ve `/run` bağlamalarını `/newroot` içine taşır (`MS_MOVE`), `chroot` ile yeni köke geçer ve kontrolü kalıcı diskteki `/sbin/init` (`systemd`) sürecine devreder.
4. `root=` parametresi verilmezse (`run-qemu.sh` veya `.iso` kurtarma modu) `ender-init` eskisi gibi hafif RAM modunda açılır.

## Derleme ve Başlatma (WSL2 Ubuntu)

```bash
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-kernel.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-module.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-initramfs.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-rootfs.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/run-desktop-qemu.sh"
```

Sistem açıldığında artık doğrudan `pacman` ile paket kurabilirsiniz:

```bash
pacman-key --init && pacman-key --populate archlinux
pacman -Sy
```
