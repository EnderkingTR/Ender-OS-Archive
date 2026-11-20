# Faz 5: Önyüklenebilir GRUB `.iso` İmajı, Ekran/Klavye ve Framebuffer (`Ender-OS v0.5`)

Bu fazda Ender-OS doğrudan `-kernel` parametresiyle çalıştırılan bir sanal makine imajından çıkıp **kendi önyükleyicisine (GRUB BIOS + UEFI)**, **sanal terminal + PS/2 klavye sürücülerine** ve **saf Rust `/dev/fb0` grafik arayüzüne (`ender-fb`)** sahip eksiksiz bir işletim sistemi `.iso` imajına dönüşür.

## Öne Çıkan Bileşenler

1. **Hibrit BIOS + UEFI GRUB `.iso` (`boot/grub/grub.cfg` & `scripts/build-iso.sh`):**
   - `grub-mkrescue` ve `xorriso` kullanarak hem klasik **BIOS (VirtualBox / eski PC)** hem **UEFI** sistemlerde önyüklenebilen `build/Ender-OS-v0.5.iso` dosyasını üretir.
   - `set gfxpayload=keep` ayarı sayesinde GRUB'un kurduğu `1024x768x32` grafik ekran tamponu doğrudan Linux çekirdeğine (`CONFIG_FB_SIMPLE` + `CONFIG_FB_DEVICE`) devredilir.
   - İki önyükleme seçeneği sunar:
     - **Varsayılan (`console=ttyS0 console=tty0`):** VirtualBox, gerçek bilgisayar ve `GUI=1` QEMU pencereleri için fiziksel ekran + klavyeyi aktif konsol yapar.
     - **Seri Konsol (`console=tty0 console=ttyS0`):** Başsız (`-nographic`) QEMU seri oturumları için seri portu aktif konsol yaparken arka planda `/dev/fb0` cihazını açık tutar.

2. **Saf Rust Framebuffer Grafik Arayüzü (`userspace/ender-fb`):**
   - Hiçbir harici grafik kütüphanesi (SDL, X11, Wayland) olmadan `/dev/fb0` cihazını açıp `FBIOGET_FSCREENINFO` / `FBIOGET_VSCREENINFO` `ioctl` çağrılarıyla çözünürlüğü öğrenir.
   - `libc::mmap` ile çekirdek video belleğini doğrudan Rust dilimine (`&mut [u8]`) bağlar ve yerleşik 8x8 piksel bitmap yazı tipiyle canlı bellek barı, servisler, çekirdek modülleri ve `/dev/ender` tamponunu çizer.
   - Komutlar: `ender-fb info`, `ender-fb once` veya `ender-fb [saniye]`.

## Derleme ve Çalıştırma (WSL2 Ubuntu)

```bash
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-kernel.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-module.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-initramfs.sh"
bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/build-iso.sh"
```

### QEMU Grafik Penceresinde Çalıştırma (WSLg)

```bash
GUI=1 bash "/mnt/c/Users/nadir/Desktop/Nükleer Başlıklı Füze Fırlatma Kodları/Ender-OS/scripts/run-iso-qemu.sh"
```

Açılan QEMU penceresinde veya terminalde `ender-fb info` ve `ender-fb 10` komutlarını çalıştırarak Rust grafik arayüzünü doğrudan ekranda görebilirsiniz. `build/Ender-OS-v0.5.iso` dosyasını ayrıca doğrudan **VirtualBox** sanal makinesine bağlayıp boot edebilirsiniz.
