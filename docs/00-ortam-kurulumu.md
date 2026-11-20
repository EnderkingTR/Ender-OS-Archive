# Faz 0: Geliştirme Ortamı Kurulumu (WSL2 + Rust + QEMU)

Bu rehber, Windows 11 üzerinde **Ender-OS** geliştirme ortamını sıfırdan kurmak için izlenecek adımları içerir.

## 1. WSL2 ve Ubuntu Kurulumu (Windows Tarafı)

1. Başlat menüsünden **PowerShell**'i sağ tıklayıp **Yönetici olarak çalıştır** (*Run as Administrator*) seçeneğiyle açın.
2. Aşağıdaki komutu çalıştırın:

```powershell
wsl --install -d Ubuntu
```

3. Kurulum tamamlandıktan sonra **bilgisayarınızı yeniden başlatın**.

## 2. Ubuntu İlk Açılış ve Kullanıcı Oluşturma

1. Bilgisayar açıldıktan sonra Başlat menüsünden **Ubuntu** uygulamasını başlatın.
2. Kurulum tamamlandığında sizden yeni bir **Linux kullanıcı adı** ve **parola** belirlemeniz istenecektir (parolayı yazarken ekranda karakter görünmez, normaldir).

## 3. Otomatik Kurulum Betiğini Çalıştırma

Ubuntu terminali açıldıktan sonra aşağıdaki komutu kopyalayıp çalıştırın. Betik; Linux çekirdeği derleme araçlarını, `musl-tools` paketini, `qemu-system-x86` emülatörünü, Rust (`rustup`) kurulumunu ve `x86_64-unknown-linux-musl` statik hedefini otomatik olarak kuracaktır:

```bash
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/setup-wsl.sh"
```

Kurulum bittikten sonra `rustc` ve `cargo` komutlarının mevcut oturumda aktif olması için şu komutu çalıştırın (veya Ubuntu terminalini kapatıp yeniden açın):

```bash
source "$HOME/.cargo/env"
```

## 4. Kurulumu Doğrulama

Ubuntu terminalinde aşağıdaki komutların hepsinin sürüm bilgisi verdiğini doğrulayın:

```bash
gcc --version | head -n1
rustc --version
cargo --version
rustup target list --installed
qemu-system-x86_64 --version | head -n1
```

`rustup target list --installed` çıktısında `x86_64-unknown-linux-musl` hedefi görünmelidir.

---

## Mühendislik Notları

### Neden WSL2 Kullanıyoruz?
Linux çekirdeği (Kbuild) ve `initramfs` paketleme araçları POSIX dosya izinlerine, sembolik linklere ve Linux sistem başlıklarına ihtiyaç duyar. WSL2, Windows içinde gerçek bir Linux çekirdeği ve `ext4` dosya sistemi sunduğu için gömülü Linux / OS geliştirme iş akışını birebir destekler.

### Neden Ağır Derlemeler `~/ender-os-build` Altında Yapılıyor?
Proje kaynak dosyaları Windows diskinde (`/mnt/c/...`) durur; ancak yol üzerinde **boşluk ve Türkçe karakterler** bulunmaktadır ve disk **NTFS** dosya sistemidir:
- Linux çekirdeği derleme sistemi (**Kbuild**), boşluk içeren dosya yollarında hata verir.
- NTFS büyük/küçük harf duyarsızdır (*case-insensitive*); Linux çekirdeği kaynak kodunda aynı dizinde yalnızca harf büyüklüğü farklı dosyalar (ör. `xt_TCPMSS.c` ve `xt_tcpmss.c`) bulunduğundan NTFS üzerinde çakışma yaşanır.
- WSL2'nin Windows dosya sistemine erişimi (9P protokolü) binlerce küçük dosyayı derlerken `ext4`'e kıyasla çok daha yavaştır.

Bu nedenle kaynak kodlarımızı bu repoda saklayıp temiz Git geçmişi tutacağız; çekirdek kaynak kodu, Cargo derleme önbelleği ve çıktı imajları ise WSL2'nin yerel `ext4` diskindeki `~/ender-os-build` klasöründe işlenecektir.

### Neden `x86_64-unknown-linux-musl` Hedefi?
Normal Linux programları çalışırken sistemde dinamik olarak paylaşılan `glibc` kütüphanelerine (`/lib/ld-linux-x86-64.so.2`, `libc.so.6` vb.) ihtiyaç duyar. Bizim ilk aşamada oluşturacağımız minimal `initramfs` içinde hiçbir hazır dağıtım kütüphanesi olmayacaktır. `x86_64-unknown-linux-musl` hedefi, yazdığımız Rust `init` ve sistem araçlarını **tamamen statik (zero-dependency)** tek bir ikili dosya (binary) olarak derler; böylece Linux çekirdeği doğrudan bizim Rust programımızı PID 1 olarak çalıştırabilir.
