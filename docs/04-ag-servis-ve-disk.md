# Faz 4: Ağ Yığını, Rust Web Dashboard, Servis Yöneticisi ve Kalıcı `ext4` Disk (`Ender-OS v0.4`)

Bu fazda Ender-OS; ağ kartı (`VirtIO-Net`), TCP/IP yığını, kalıcı disk (`VirtIO-Blk` + `ext4`), çoklu servis denetleyicisi (*mini-systemd*) ve tarayıcıdan erişilebilen kendi **Rust HTTP/1.1 Web Kontrol Paneli** (`ender-httpd`) kazanır.

## Mimari Genel Bakış

```text
Windows / WSL2 Tarayıcı (http://localhost:8080)
  └─ QEMU SLIRP Port Yönlendirme (8080 -> eth0:8080)
       └─ Linux 6.18.64-ender (TCP/IP + VirtIO-Net + VirtIO-Blk + ext4 + Rust LKM)
            └─ PID 1: /init (ender-init — mini-systemd denetleyicisi)
                 ├─ /dev/vda -> /data (kalıcı ext4 dosya sistemi)
                 ├─ eth0 -> 10.0.2.15/24 (gw 10.0.2.2) (socket ioctl ile)
                 ├─ /lib/modules/ender_dev.ko otomatik yüklenir
                 ├─ PID 2: /bin/ender-httpd (0.0.0.0:8080 Web Dashboard & JSON API)
                 └─ PID 3: /bin/ender-sh (etkileşimli kabuk)
```

## Öne Çıkan Sistem Bileşenleri

1. **Çekirdek İçi Ağ Yapılandırması (`userspace/ender-init/src/net.rs`):** Harici `ip`/`ifconfig` aracı olmadan doğrudan `libc::socket(AF_INET, SOCK_DGRAM, 0)` açıp Linux UAPI `SIOCSIFADDR`, `SIOCSIFNETMASK`, `SIOCSIFFLAGS` (`IFF_UP | IFF_RUNNING`) ve `SIOCADDRT` (`rtentry`) sistem çağrılarını kullanır.
2. **Mini-Systemd Denetleyicisi (`userspace/ender-init/src/supervisor.rs`):** Arka plan servisi (`ender-httpd`) ve etkileşimli kabuğu (`ender-sh`) birlikte yönetir, durumlarını `/run/services/*.status` altında yayınlar, yetim zombi süreçleri `waitpid(-1)` ile toplar ve çöken servisi 1 saniye içinde otomatik yeniden başlatır.
3. **Sıfır Bağımlılıklı Web Sunucusu (`userspace/ender-httpd`):** Yalnızca Rust `std::net::TcpListener` ve `std::thread` kullanarak canlı bellek, servis, modül ve `/data` durumunu gösteren kontrol paneli ile `/dev/ender` aygıtına tarayıcıdan veri yazabilen `POST /api/ender-dev` uç noktasını sunar.
4. **Kalıcı `ext4` Disk (`/data`):** `scripts/build-disk.sh` ile oluşturulan `data.img` imajı QEMU'ya VirtIO blok aygıtı (`/dev/vda`) olarak bağlanır; `initramfs` RAM'de çalışsa bile `/data` altındaki dosyalar kapanış ve yeniden başlatmalar arasında korunur.

## Yeni Kullanıcı Alanı Komutları

- `ip` — Ağ arayüzlerini, MAC/IP adreslerini ve çekirdek yönlendirme tablosunu gösterir.
- `svc` — `ender-init` tarafından yönetilen servislerin PID, durum ve restart sayısını listeler.
- `df` — Bağlı dosya sistemlerinin (`/`, `/tmp`, `/run`, `/data`) kapasite ve boş alan bilgisini gösterir.
- `fetch http://127.0.0.1:8080/api/status` — Yerleşik HTTP istemcisiyle uç noktalardan veri çeker.

## Derleme ve Çalıştırma Sırası (WSL2 Ubuntu)

```bash
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-kernel.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-module.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-disk.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-initramfs.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/run-qemu.sh"
```

QEMU çalışırken Windows tarayıcınızdan **http://localhost:8080** adresini açabilirsiniz.
