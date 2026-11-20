# Faz 1: İlk Boot (`Ender-OS v0.1`)

Bu fazda Linux 6.18 LTS çekirdeğini kaynak koddan derleyip, Rust ile yazdığımız ilk statik **PID 1 (`/init`)** programını `initramfs` içine paketleyerek **QEMU** sanal makinesinde ilk açılışı gerçekleştiriyoruz.

## Linux Açılış Akışı

```text
QEMU (x86_64)
  └─> bzImage (Linux 6.18.64-ender çekirdeği)
        └─> initramfs.cpio.gz belleğe açılır (rootfs)
              └─> Çekirdek /init sürecini PID 1 olarak başlatır (ender-init)
```

## `ender-init` Neden Özeldir?

1. **PID 1 Sorumluluğu:** Linux'ta PID 1 sistemin ilk ve ana sürecidir. `/init` herhangi bir sebeple `main()` fonksiyonundan çıkarsa çekirdek hemen `Kernel panic - not syncing: Attempted to kill init!` hatası verip durur. Bu nedenle `ender-init`, `EOF` durumunda dahi sonsuz döngüde bekler ve yalnızca `reboot`/`poweroff` sistem çağrılarıyla kapanır.
2. **Sanal Dosya Sistemlerini Bağlama (`mount`):** Çekirdek açıldığında donanım ve süreç bilgileri henüz kullanıcı alanına sunulmamıştır. `ender-init`, `libc::mount` çağrısı üzerinden `/proc` (`proc`), `/sys` (`sysfs`), `/dev` (`devtmpfs`) ve `/tmp` (`tmpfs`) dosya sistemlerini bağlar.
3. **Çekirdek Verisi Okuma (`/proc`):** `info` ve `mem` komutları, standart Linux dosya okuma API'siyle (`std::fs::read_to_string`) doğrudan çekirdeğin ürettiği `/proc/version`, `/proc/uptime` ve `/proc/meminfo` sanal dosyalarını okur.

## Kodda Öne Çıkan Rust Kavramları (`userspace/ender-init/src/main.rs`)

- **`Result` ve `?` operatörü:** Hata yönetimi istisna (*exception*) fırlatmak yerine dönüş değerleriyle yapılır.
- **`CString`:** Rust metinleri (`&str`) null (`\0`) ile sonlanmaz ve UTF-8'dir. C tabanlı Linux sistem çağrılarına (`libc::mount`) metin geçirirken `CString` ile güvenli null-sonlandırmalı tampon oluşturulur.
- **`unsafe` ve `// SAFETY:` sınırlandırması:** Rust'ta ham işaretçi alan veya doğrudan çekirdek ABI'sine giren `libc::mount` / `libc::reboot` çağrıları `unsafe` blok içine hapsedilir ve etrafına güvenli bir Rust fonksiyonu (`mount_fs`, `system_reboot`) sarılır.
- **Kapsamlı `match` kontrolü:** Komut ayrıştırma ve girdi okuma (`Ok(0)`, `Ok(_)`, `Err(_)`) tüm olası durumları zorunlu olarak ele alır.

## Derleme ve Çalıştırma Adımları (WSL2 Ubuntu)

1. **Çekirdeği derle (`bzImage`):**
   ```bash
   bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-kernel.sh"
   ```
2. **Rust `/init` programını derle ve `initramfs.cpio.gz` oluştur:**
   ```bash
   bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-initramfs.sh"
   ```
3. **Ender-OS v0.1'i QEMU'da başlat:**
   ```bash
   bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/run-qemu.sh"
   ```

QEMU açıldığında `ender-init# ` isteminde `help`, `info`, `mem`, `reboot` ve `poweroff` komutlarını deneyebilirsiniz.
