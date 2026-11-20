# Ender-OS

Akademik ve sektörel gelişim amacıyla **Linux çekirdeği (kernel)** üzerine **Rust** kullanıcı alanı (*userspace*) ve ileri aşamalarda **Rust-for-Linux** çekirdek modülleri inşa edilerek geliştirilen öğrenme odaklı işletim sistemi projesi.

## Mimari Genel Bakış

```text
+-------------------------------------------------------+
|  Rust Userspace: Ender-Shell (ender-sh) & Coreutils   |
+-------------------------------------------------------+
|  Rust PID 1: ender-init (/init, mount, süreç yönetimi) |
+-------------------------------------------------------+
|  Linux Çekirdeği (LTS) (+ Faz 3: Rust Kernel Modülü)  |
+-------------------------------------------------------+
|  QEMU x86_64 Sanal Donanım / Gerçek Donanım           |
+-------------------------------------------------------+
```

## Planlanan Dizin Yapısı

```text
Ender-OS/
├── docs/          # Kurulum, mimari ve faz bazlı teknik dokümanlar
├── scripts/       # WSL2 ortam kurulumu, çekirdek derleme ve QEMU çalıştırma betikleri
├── kernel/        # Özel Linux çekirdek yapılandırmaları ve (Faz 3) LKM sürücüleri
├── userspace/     # Rust ile yazılan PID 1 init, shell (ender-sh) ve temel araçlar
└── build/         # Yerel çıktı artefaktları (.gitignore ile hariç tutulur)
```

## Yol Haritası

- **Faz 0 — Geliştirme Ortamı:** WSL2 Ubuntu, Linux çekirdeği derleme araçları, statik Rust (`x86_64-unknown-linux-musl`) ve QEMU kurulumu.
- **Faz 1 (`v0.1`) — İlk Boot:** Linux çekirdeğini kaynak koddan derleme, Rust ile ilk statik PID 1 (`/init`) programını yazma ve QEMU üzerinde ilk açılışı alma.
- **Faz 2 (`v0.2`) — Rust Userspace & Kabuk:** Rust ile kendi terminal kabuğumuzu (`ender-sh`), temel sistem araçlarını (`ls`, `cat`, `ps`, `meminfo`, `uptime`) yazma ve `/proc`, `/sys` dosya sistemleriyle etkileşim kurma.
- **Faz 3 (`v0.3`) — Çekirdek Modülü (LKM):** Linux çekirdeği için özel karakter aygıtı sürücüsü / sistem bilgisi modülü geliştirme (Rust-for-Linux / C karşılaştırmalı).
- **Faz 4 (`v0.4+`) — İleri Seviye:** Önyüklenebilir `.iso` imajı, basit servis yöneticisi (*mini-systemd*), ağ (networking) veya framebuffer tabanlı grafik arayüz.

## Hızlı Başlangıç

Ortam kurulumunu tamamlamak için [Faz 0: Ortam Kurulumu Rehberi](docs/00-ortam-kurulumu.md) dosyasındaki adımları izleyin.
