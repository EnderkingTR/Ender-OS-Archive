# Ender-OS

![License](https://img.shields.io/badge/License-MIT-blue.svg)
![Linux Kernel](https://img.shields.io/badge/Linux_Kernel-6.18.64_LTS-green.svg)
![Rust](https://img.shields.io/badge/Rust-2024_%2B_Rust--for--Linux-orange.svg)
![Architecture](https://img.shields.io/badge/Arch-x86__64-lightgrey.svg)

**Ender-OS**, akademik ve sektörel sistem programlama gelişimi için **Linux `6.18.64` LTS** çekirdeği üzerine sıfırdan inşa edilmiş; **Rust kullanıcı alanı (*userspace*)**, **Rust-for-Linux çekirdek modülü**, **mini-systemd servis denetleyicisi**, **TCP/IP ağ yığını**, **sıfır bağımlılıklı HTTP/1.1 Web Kontrol Paneli** ve **kalıcı `ext4` disk** desteğine sahip hafif bir işletim sistemidir.

---

## Mimari Genel Bakış (`v0.4`)

```text
Host Tarayıcı (http://localhost:8080)
  └─ QEMU SLIRP Port Yönlendirme (8080 -> eth0:8080)
       └─ Linux 6.18.64-ender (LLVM=1 + CONFIG_RUST=y + VirtIO-Net + VirtIO-Blk + ext4)
            ├─ Rust-for-Linux LKM: ender_dev.ko (/dev/ender karakter aygıtı)
            └─ PID 1: /init (ender-init — mini-systemd denetleyicisi)
                 ├─ /dev/vda -> /data (kalıcı ext4 dosya sistemi)
                 ├─ eth0 -> 10.0.2.15/24 (gw 10.0.2.2) (socket ioctl ile)
                 ├─ PID 2: /bin/ender-httpd (0.0.0.0:8080 Web Dashboard & JSON API)
                 └─ PID 3: /bin/ender-sh (boru hatlı etkileşimli kabuk + ender-utils)
```

---

## Öne Çıkan Özellikler

- **`ender-init` (PID 1 Mini-Systemd Denetleyicisi):**
  - Sanal dosya sistemlerini (`/proc`, `/sys`, `/dev`, `/tmp`, `/run`) ve kalıcı `/dev/vda` (`ext4` → `/data`) diskini bağlar.
  - Harici `ip`/`ifconfig` aracı olmadan doğrudan `libc::socket` + `SYS_ioctl` (`SIOCSIFADDR`, `SIOCSIFNETMASK`, `SIOCSIFFLAGS`, `SIOCADDRT`) ile `lo` (`127.0.0.1/8`) ve `eth0` (`10.0.2.15/24`) ağını yapılandırır.
  - Seri konsolda `IUTF8` TTY bayrağını etkinleştirir, `ender_dev.ko` modülünü otomatik yükler, `/run/services/*.status` altında servis durumunu yayınlar, `waitpid(-1)` ile yetim zombileri toplar ve çöken servisleri otomatik yeniden başlatır.
- **`ender-sh` (Saf Rust Etkileşimli Kabuk):**
  - Tırnak (`"..."`, `'...'`), `#` yorumları, boru hatları (`|`), çıktı yönlendirmeleri (`>`, `>>`), yerleşik komutlar (`cd`, `export`, `env`, `help`, `exit`) ve ham bayt tabanlı UTF-8 dayanıklı girdi okuyucusu içerir.
- **`ender-utils` (BusyBox Tarzı Statik Çoklu-Komut İkili Dosyası — 25 Komut):**
  - **Dosya & Metin:** `ls`, `cat`, `mkdir`, `rm`, `touch`, `pwd`, `echo`, `head`, `grep`, `clear`
  - **Süreç & Çekirdek:** `ps`, `uname`, `uptime`, `meminfo`, `poweroff`, `reboot`
  - **Modül & IOCTL:** `insmod`, `rmmod`, `lsmod`, `dmesg`, `enderctl`
  - **Ağ, Servis & Disk:** `ip`, `fetch`, `svc`, `df`
- **`ender_dev.ko` (Rust-for-Linux Çekirdek Modülü):**
  - `MiscDevice` soyutlaması ve `GlobalLock<Mutex<DeviceState>>` kullanarak `/dev/ender` karakter aygıtını kaydeder; `read_iter`, `write_iter`, `IovIter`/`UserSlice` bellek sınırları ve özel `ioctl` (`stats` / `reset`) komutlarını uygular.
- **`ender-httpd` (Sıfır Bağımlılıklı Rust Web Dashboard):**
  - Yalnızca `std::net::TcpListener` ve `std::thread` ile çalışan çok iş parçacıklı HTTP/1.1 sunucusu; canlı RAM/servis/modül/disk arayüzü (`/`), JSON API (`/api/status`), `/dev/ender` okuma/yazma uç noktası (`/api/ender-dev`) ve `/health` sunar.

---

## Dizin Yapısı

```text
Ender-OS/
├── docs/                           # Faz bazlı kurulum ve mimari dokümanları
├── kernel/
│   ├── ender_x86_64.config         # Minimal x86_64 Linux 6.18.64 LTS yapılandırma parçası
│   └── modules/ender_dev/          # Rust-for-Linux /dev/ender LKM sürücüsü (Kbuild, Makefile, .rs)
├── scripts/
│   ├── setup-wsl.sh                # WSL2 Ubuntu temel araç ve Rust (musl) kurulum betiği
│   ├── build-kernel.sh             # LLVM=1 + CONFIG_RUST=y Linux çekirdeği derleme betiği
│   ├── build-module.sh             # Out-of-tree ender_dev.ko derleme betiği
│   ├── build-disk.sh               # Kalıcı 16M ext4 sanal disk (data.img) oluşturma betiği
│   ├── build-initramfs.sh          # Statik musl workspace derleme ve initramfs.cpio.gz paketleme
│   └── run-qemu.sh                 # VirtIO-Net + VirtIO-Blk destekli QEMU başlatma betiği
├── userspace/
│   ├── Cargo.toml                  # Rust workspace tanımı (LTO + strip)
│   ├── ender-init/                 # PID 1 servis denetleyicisi ve ağ/disk başlatıcı
│   ├── ender-sh/                   # Etkileşimli kabuk (ayrıştırıcı + boru hattı çalıştırıcısı)
│   ├── ender-utils/                # 25 komutlu multi-call sistem araçları
│   └── ender-httpd/                # Sıfır bağımlılıklı HTTP/1.1 Web Dashboard sunucusu
├── LICENSE                         # MIT Lisansı
└── README.md
```

---

## Yol Haritası

- [x] **Faz 0 — Geliştirme Ortamı:** WSL2 Ubuntu, çekirdek derleme araçları, statik Rust (`x86_64-unknown-linux-musl`) ve QEMU kurulumu.
- [x] **Faz 1 (`v0.1`) — İlk Boot:** Linux `6.18.64` çekirdeğini kaynak koddan derleme, Rust ile statik PID 1 (`/init`) yazma ve QEMU'da ilk boot.
- [x] **Faz 2 (`v0.2`) — Rust Userspace & Kabuk:** `ender-sh` kabuğu (`|`, `>`, `>>`), `ender-utils` multi-call araçları, `/proc` & `/sys` entegrasyonu ve PID 1 zombi toplama.
- [x] **Faz 3 (`v0.3`) — Rust-for-Linux Çekirdek Modülü:** `LLVM=1` + `CONFIG_RUST=y` ile `/dev/ender` (`ender_dev.ko`) sürücüsü ve `insmod`/`rmmod`/`lsmod`/`dmesg`/`enderctl` araçları.
- [x] **Faz 4 (`v0.4`) — Ağ, Web Paneli, Servis Yöneticisi & Kalıcı Disk:** `VirtIO-Net` + soket `ioctl`, `ender-httpd` Web Dashboard (`:8080`), mini-systemd denetleyicisi ve `VirtIO-Blk` `ext4` (`/data`) kalıcılığı.
- [ ] **Faz 5 (`v0.5+`) — İleri Aşamalar:** VirtualBox/USB uyumlu önyüklenebilir GRUB `.iso` imajı ve isteğe bağlı framebuffer tabanlı grafik arayüz.

---

## Hızlı Başlangıç (Linux / WSL2 Ubuntu)

### 1. Depoyu Klonlayın ve Ortamı Kurun

```bash
git clone https://github.com/EnderkingTR/Ender-OS.git
cd Ender-OS
bash scripts/setup-wsl.sh
source "$HOME/.cargo/env"
```

Rust-for-Linux çekirdeği (`CONFIG_RUST=y`) derlemesi için ek LLVM/bindgen araçlarını da kurun:

```bash
sudo apt-get update && sudo apt-get install -y clang lld llvm bindgen libclang-dev
rustup component add rust-src
```

> **Not:** Windows üzerindeki boşluklu/NTFS dosya yollarında Kbuild uyumluluğu ve derleme hızı için tüm ağır derleme çıktıları WSL2 yerel `ext4` dizini olan `$HOME/ender-os-build` altında tutulur.

### 2. Derleyin ve QEMU'da Çalıştırın

```bash
bash scripts/build-kernel.sh
bash scripts/build-module.sh
bash scripts/build-disk.sh
bash scripts/build-initramfs.sh
bash scripts/run-qemu.sh
```

QEMU açıldığında tarayıcınızdan **`http://localhost:8080`** adresini ziyaret ederek canlı kontrol paneline ulaşabilir ve `/dev/ender` çekirdek modülüne doğrudan mesaj gönderebilirsiniz. Terminal içinden `ip`, `svc`, `df`, `lsmod`, `enderctl stats`, `fetch http://127.0.0.1:8080/api/status` ve `poweroff` komutlarını deneyebilirsiniz.

### 3. Birim Testlerini Çalıştırma

```bash
CARGO_TARGET_DIR="$HOME/ender-os-build/cargo-target" cargo test \
  --manifest-path "$HOME/ender-os-build/userspace-src/Cargo.toml" \
  --workspace
```

---

## Dokümantasyon

- [Faz 0 — Geliştirme Ortamı Kurulumu](docs/00-ortam-kurulumu.md)
- [Faz 1 — İlk Boot (`Ender-OS v0.1`)](docs/01-ilk-boot.md)
- [Faz 2 — Rust Kabuğu ve Userspace (`Ender-OS v0.2`)](docs/02-kabuk-ve-userspace.md)
- [Faz 3 — Rust-for-Linux Çekirdek Modülü (`Ender-OS v0.3`)](docs/03-cekirdek-modulu.md)
- [Faz 4 — Ağ, Web Paneli, Servisler ve Kalıcı Disk (`Ender-OS v0.4`)](docs/04-ag-servis-ve-disk.md)

---

## Lisans

Bu projenin kullanıcı alanı araçları, betikleri ve dokümantasyonu [MIT Lisansı](LICENSE) ile lisanslanmıştır.

> **Çekirdek Modülü Notu:** [kernel/modules/ender_dev/](kernel/modules/ender_dev/) altındaki Linux çekirdek modülü (`ender_dev.ko`), Linux çekirdeğinin GPL uyumluluk gereksinimleri (`EXPORT_SYMBOL_GPL`) nedeniyle **GPL-2.0** (`SPDX-License-Identifier: GPL-2.0`) lisansı altında sunulmaktadır.
