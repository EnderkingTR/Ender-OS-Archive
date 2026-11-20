# Faz 3: Rust-for-Linux Çekirdek Modülü (`/dev/ender`)

Bu fazda Linux çekirdeği **LLVM/Clang + `CONFIG_RUST=y`** desteğiyle derlenir ve ilk yüklenebilir **Rust çekirdek modülümüz** (`ender_dev.ko`) geliştirilir.

## Çekirdek Alanı ile Kullanıcı Alanı Arasındaki Sınır

- **Kullanıcı Alanı (`ender-sh`, `ender-utils`):** Hata yaparsa yalnızca ilgili süreç çöker; çekirdeğe `read`, `write`, `ioctl`, `init_module` gibi **sistem çağrıları (syscalls)** üzerinden erişir.
- **Çekirdek Alanı (`ender_dev.ko`):** Doğrudan ayrıcalıklı çekirdek bellek alanında çalışır (`no_std`, `kernel::prelude::*`). Kullanıcı belleğinden gelen tüm veriler `IovIter` ve `UserSlice` üzerinden güvenli kopyalanır; eşzamanlı erişim `GlobalLock<Mutex<DeviceState>>` ile korunur.

## `ender_dev.ko` Mimarisi

```text
Kullanıcı Alanı
  ├─ insmod /lib/modules/ender_dev.ko  ──(SYS_init_module)──> Çekirdek modülü yükler
  ├─ cat /dev/ender                    ──(VFS read_iter)───> DEVICE_STATE tamponunu okur
  ├─ echo "..." > /dev/ender           ──(VFS write_iter)──> DEVICE_STATE tamponuna yazar
  ├─ enderctl stats / reset            ──(SYS_ioctl)───────> Sayaçları okur / tamponu sıfırlar
  ├─ dmesg                             ──(SYS_syslog)──────> Çekirdek günlük mesajlarını okur
  └─ rmmod ender_dev                   ──(SYS_delete_module)> Modülü ve /dev/ender'ı kaldırır
```

## Derleme Sırası (WSL2 Ubuntu)

1. **Rust destekli çekirdeği derle (`LLVM=1`, `CONFIG_RUST=y`, `CONFIG_MODULES=y`):**
   ```bash
   bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-kernel.sh"
   ```
2. **Rust çekirdek modülünü derle (`ender_dev.ko`):**
   ```bash
   bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-module.sh"
   ```
3. **Kullanıcı alanı araçlarını ve modülü `initramfs` içine paketle:**
   ```bash
   bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-initramfs.sh"
   ```
4. **QEMU'da çalıştır ve modülü canlı test et:**
   ```bash
   bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/run-qemu.sh"
   ```

QEMU içinde deneyebileceğiniz komutlar:

```sh
insmod /lib/modules/ender_dev.ko
lsmod
ls -l /dev/ender
cat /dev/ender
echo "Merhaba Rust Kernel!" > /dev/ender
cat /dev/ender
enderctl stats
enderctl reset
cat /dev/ender
dmesg | grep ender_dev
rmmod ender_dev
poweroff
```
