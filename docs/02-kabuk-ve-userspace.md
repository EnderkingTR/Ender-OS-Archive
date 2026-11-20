# Faz 2: Rust Kabuğu (`ender-sh`) ve Userspace Araçları (`Ender-OS v0.2`)

Bu fazda Ender-OS, tek bir etkileşimli PID 1 programından gerçek bir **çoklu süreçli Linux kullanıcı alanına (userspace)** dönüşür.

## Süreç Mimarisi

```text
PID 1: /init (ender-init — süreç denetleyicisi)
  │
  ├─ Sanal FS bağlar: /proc, /sys, /dev, /tmp
  ├─ Makine adını ayarlar: ender-os (libc::sethostname)
  ├─ waitpid(-1) ile yetim zombi süreçleri toplar
  └─ Başlatır ve izler:
       └─ PID 2: /bin/ender-sh (etkileşimli kabuk; çıkarsa yeniden başlatılır)
            └─ PID 3+: /bin/ls | /bin/grep | /bin/head ... (alt süreçler)
```

### PID 1 Denetleyicisi ve Zombi Toplama (`ender-init`)
Unix/Linux sistemlerinde bir alt süreç sonlandığında üst süreci (`parent`) onun çıkış durumunu `wait()`/`waitpid()` ile okuyana kadar süreç **zombi (`Z`)** durumunda kalır. Üst süreç altından önce ölürse, yetim kalan süreçler otomatik olarak **PID 1**'e devredilir. `ender-init`, `libc::waitpid(-1, &mut status, 0)` döngüsüyle hem kabuğu izler hem de sistemdeki tüm yetim süreçleri temizler. Kabuk `exit` ile kapanırsa 1 saniye sonra yeni bir kabuk başlatır.

## Çoklu-Komut (Multi-Call) İkili Dosya Deseni (`ender-utils`)
`BusyBox` tarafından popülerleştirilen bu mimaride 16 farklı sistem aracı (`ls`, `cat`, `ps`, `uname`, `uptime`, `meminfo`, `echo`, `mkdir`, `rm`, `touch`, `head`, `grep`, `clear`, `pwd`, `poweroff`, `reboot`) **tek bir statik Rust ikili dosyasında** (`/bin/ender-utils`) birleştirilir.

`/bin/ls`, `/bin/ps` veya `/sbin/poweroff` gibi komutlar aynı dosyaya işaret eden birer **sembolik linktir** (`symlink`). İşletim sistemi programı başlattığında `argv[0]` değerine hangi linkten çağrıldığı yazılır; `ender-utils` `argv[0]` adını okuyup ilgili alt komutu çalıştırır. Böylece tüm standart kütüphane yalnızca 1 kez paketlenir ve `initramfs` boyutu son derece küçük kalır.

## Boru Hattı (`|`) ve Yönlendirme (`>`, `>>`) Nasıl Çalışır?
`ender-sh` kabuğu komut satırını iki aşamada işler:
1. **Ayrıştırıcı (`parser.rs`):** Tırnakları (`"..."`, `'...'`), yorumları (`#`), yönlendirme operatörlerini (`>`, `>>`) ve boru ayırıcılarını (`|`) aşamalara böler.
2. **Çalıştırıcı (`exec.rs`):** İlk aşamadan son aşamaya kadar her süreci `std::process::Command` ile başlatır; bir önceki sürecin `ChildStdout` çıkışını `Stdio::from(out)` ile bir sonraki sürecin `stdin` girişine bağlar. Yönlendirme varsa son çıktıyı `File::create` (`>`) veya `append(true)` (`>>`) dosyasına yönlendirir.

## Denenebilecek Komut Örnekleri

`run-qemu.sh` ile sistemi başlattıktan sonra şu komutları deneyebilirsiniz:

```sh
uname -a
ps
ls -l /bin
echo "Merhaba Ender-OS" > /tmp/not.txt
echo "Faz 2 Tamam" >> /tmp/not.txt
cat /tmp/not.txt | grep Faz
cat /proc/meminfo | head -n 3
cd /tmp
pwd
exit      # ender-init kabuğu otomatik yeniden başlatır
poweroff  # sistemi güvenli kapatır
```
