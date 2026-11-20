//! Ender-OS v0.5 — Saf Rust `/dev/fb0` Framebuffer Grafik Arayüzü.
//!
//! Doğrudan Linux Framebuffer cihazını (`/dev/fb0`) `ioctl` ile sorgular, video
//! belleğini `mmap` ile kullanıcı alanına eşler ve harici kütüphane olmadan
//! sistem durum ekranını piksel piksel çizer.

mod canvas;
mod font;

use std::env;
use std::fs::{self, OpenOptions};
use std::io;
use std::mem;
use std::os::fd::AsRawFd;
use std::ptr;
use std::slice;
use std::thread;
use std::time::Duration;

use canvas::{Canvas, PixelFormat};

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FbFixScreeninfo {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    fb_type: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

struct FbDeviceInfo {
    id: String,
    fix: FbFixScreeninfo,
    var: FbVarScreeninfo,
}

fn query_fb_info(fd: i32) -> io::Result<FbDeviceInfo> {
    // SAFETY: `FbFixScreeninfo` ve `FbVarScreeninfo` yalnızca POD tam sayı/dizi alanlarından
    // oluşan `#[repr(C)]` Linux UAPI yapılarıdır.
    let mut fix: FbFixScreeninfo = unsafe { mem::zeroed() };
    let mut var: FbVarScreeninfo = unsafe { mem::zeroed() };

    if unsafe { libc::syscall(libc::SYS_ioctl, fd, FBIOGET_FSCREENINFO, &raw mut fix) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::syscall(libc::SYS_ioctl, fd, FBIOGET_VSCREENINFO, &raw mut var) } < 0 {
        return Err(io::Error::last_os_error());
    }

    let id_end = fix.id.iter().position(|&b| b == 0).unwrap_or(fix.id.len());
    let id = String::from_utf8_lossy(&fix.id[..id_end]).into_owned();

    Ok(FbDeviceInfo { id, fix, var })
}

fn read_mem_kb() -> (u64, u64) {
    let mut total = 0;
    let mut avail = 0;
    if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
        for line in contents.lines() {
            let val = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            if line.starts_with("MemTotal:") {
                total = val;
            } else if line.starts_with("MemAvailable:") {
                avail = val;
            }
        }
    }
    (total, avail)
}

fn read_uptime() -> String {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|u| format!("{u} SN")))
        .unwrap_or_else(|| "0 SN".to_string())
}

fn read_hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "(none)")
        .unwrap_or_else(|| "ender-os".to_string())
}

fn read_services_summary() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir("/run/services") {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if let Ok(raw) = fs::read_to_string(path) {
                let mut name = String::new();
                let mut pid = String::new();
                let mut state = String::new();
                for line in raw.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        match k.trim() {
                            "name" => name = v.trim().to_string(),
                            "pid" => pid = v.trim().to_string(),
                            "state" => state = v.trim().to_string(),
                            _ => {}
                        }
                    }
                }
                if !name.is_empty() {
                    out.push(format!("{name} (PID {pid}) [{state}]"));
                }
            }
        }
    }
    out
}

fn read_modules_summary() -> String {
    let Ok(raw) = fs::read_to_string("/proc/modules") else {
        return "YOK".to_string();
    };
    let names: Vec<_> = raw
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    if names.is_empty() {
        "YOK".to_string()
    } else {
        names.join(", ")
    }
}

fn render_dashboard_frame(canvas: &mut Canvas<'_>, info: &FbDeviceInfo, frame_idx: u32) {
    let bg = (11, 16, 32);
    let card = (19, 27, 46);
    let border = (45, 65, 108);
    let accent = (56, 189, 248);
    let ok_green = (34, 197, 94);
    let fg = (229, 236, 255);
    let muted = (143, 163, 199);

    canvas.clear(bg);

    let w = canvas.width as i32;
    let margin = 24;
    let box_w = (w - margin * 2).max(200) as u32;

    // Üst başlık çubuğu
    canvas.fill_rect(margin, 20, box_w, 64, card);
    canvas.draw_rect(margin, 20, box_w, 64, accent);
    canvas.draw_text(margin + 18, 34, 2, "ENDER-OS V0.5 GRAFIK EKRANI", accent);
    let subtitle = format!(
        "HOST: {}  |  FB: {} ({}X{} {}BPP)  |  KARE: {}",
        read_hostname(),
        info.id,
        info.var.xres,
        info.var.yres,
        info.var.bits_per_pixel,
        frame_idx + 1
    );
    canvas.draw_text(margin + 18, 60, 1, &subtitle, muted);

    // Sistem ve Bellek Kartı
    canvas.fill_rect(margin, 104, box_w, 128, card);
    canvas.draw_rect(margin, 104, box_w, 128, border);
    canvas.draw_text(margin + 18, 120, 2, "SISTEM VE BELLEK", fg);

    let (mem_total, mem_avail) = read_mem_kb();
    let mem_used = mem_total.saturating_sub(mem_avail);
    let mem_pct = if mem_total > 0 {
        ((mem_used * 100) / mem_total) as u32
    } else {
        0
    };
    let sys_line = format!(
        "UPTIME: {}   |   RAM: {} / {} KB (%{})",
        read_uptime(),
        mem_used,
        mem_total,
        mem_pct
    );
    canvas.draw_text(margin + 18, 152, 1, &sys_line, fg);
    canvas.draw_bar(
        margin + 18,
        176,
        box_w.saturating_sub(36),
        28,
        mem_pct,
        ok_green,
        (9, 13, 24),
        border,
    );

    // Servisler ve Çekirdek Modülleri Kartı
    canvas.fill_rect(margin, 250, box_w, 148, card);
    canvas.draw_rect(margin, 250, box_w, 148, border);
    canvas.draw_text(margin + 18, 266, 2, "SERVISLER VE MODULLER", fg);

    let services = read_services_summary();
    for (idx, svc) in services.iter().take(3).enumerate() {
        canvas.draw_text(margin + 18, 298 + (idx as i32) * 20, 1, svc, ok_green);
    }
    if services.is_empty() {
        canvas.draw_text(margin + 18, 298, 1, "KAYITLI SERVIS YOK", muted);
    }
    let mod_line = format!("MODULLER: {}", read_modules_summary());
    canvas.draw_text(margin + 18, 368, 1, &mod_line, accent);

    // /dev/ender Rust Çekirdek Modülü Kartı
    canvas.fill_rect(margin, 416, box_w, 110, card);
    canvas.draw_rect(margin, 416, box_w, 110, border);
    canvas.draw_text(margin + 18, 432, 2, "/DEV/ENDER CEKIRDEK TAMPONU", fg);

    let dev_msg = fs::read_to_string("/dev/ender")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/DEV/ENDER HAZIR DEGIL".to_string());
    let truncated: String = dev_msg.chars().take(64).collect();
    canvas.draw_text(margin + 18, 470, 1, &truncated, ok_green);
    canvas.draw_text(
        margin + 18,
        498,
        1,
        "WEB PANEL: HTTP://LOCALHOST:8080  |  CIKIS ICIN BEKLEYIN",
        muted,
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("loop");

    let fb_file = match OpenOptions::new().read(true).write(true).open("/dev/fb0") {
        Ok(f) => f,
        Err(err) => {
            eprintln!("ender-fb: /dev/fb0 açılamadı: {err}");
            eprintln!("İpucu: Sistemin GRUB menüsünden framebuffer destekli başlatıldığından emin olun.");
            std::process::exit(1);
        }
    };

    let info = match query_fb_info(fb_file.as_raw_fd()) {
        Ok(i) => i,
        Err(err) => {
            eprintln!("ender-fb: ekran bilgisi okunamadı: {err}");
            std::process::exit(1);
        }
    };

    if mode == "info" {
        println!("Framebuffer : /dev/fb0 ({})", info.id);
        println!(
            "Çözünürlük  : {}x{} ({} bpp)",
            info.var.xres, info.var.yres, info.var.bits_per_pixel
        );
        println!(
            "Satır/Bellek: line_length={} smem_len={}",
            info.fix.line_length, info.fix.smem_len
        );
        println!(
            "RGB Düzeni  : R({}:{}) G({}:{}) B({}:{})",
            info.var.red.offset,
            info.var.red.length,
            info.var.green.offset,
            info.var.green.length,
            info.var.blue.offset,
            info.var.blue.length
        );
        return;
    }

    let frames: u32 = if mode == "once" {
        1
    } else {
        mode.parse::<u32>().unwrap_or(5).clamp(1, 60)
    };

    let map_len = (info.fix.smem_len as usize)
        .max((info.fix.line_length as usize).saturating_mul(info.var.yres as usize));
    if map_len == 0 {
        eprintln!("ender-fb: geçersiz framebuffer bellek boyutu (0)");
        std::process::exit(1);
    }

    // SAFETY: `fb_file` geçerli bir `/dev/fb0` tanıtıcısı ve `map_len` çekirdeğin
    // bildirdiği framebuffer bellek boyutudur.
    let ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            map_len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fb_file.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        eprintln!("ender-fb: mmap başarısız: {}", io::Error::last_os_error());
        std::process::exit(1);
    }

    let format = PixelFormat {
        bpp: info.var.bits_per_pixel,
        r_off: info.var.red.offset,
        r_len: info.var.red.length,
        g_off: info.var.green.offset,
        g_len: info.var.green.length,
        b_off: info.var.blue.offset,
        b_len: info.var.blue.length,
    };

    // SAFETY: `mmap` başarılı oldu; `ptr` en az `map_len` baytlık geçerli bellek bölgesidir
    // ve `munmap` çağrısından önce bu dilim düşürülür.
    {
        let fb_slice = unsafe { slice::from_raw_parts_mut(ptr.cast::<u8>(), map_len) };
        let mut canvas = Canvas::new(
            fb_slice,
            info.var.xres,
            info.var.yres,
            info.fix.line_length as usize,
            format,
        );

        for idx in 0..frames {
            render_dashboard_frame(&mut canvas, &info, idx);
            if idx + 1 < frames {
                thread::sleep(Duration::from_secs(1));
            }
        }
    }

    // SAFETY: `ptr` ve `map_len` yukarıdaki `mmap` çağrısından gelir ve `fb_slice` kapsam dışındadır.
    unsafe {
        libc::munmap(ptr, map_len);
    }

    println!(
        "ender-fb: kare çizildi ({}x{} {}bpp)",
        info.var.xres, info.var.yres, info.var.bits_per_pixel
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fb_uapi_struct_sizes_match_linux_x86_64() {
        assert_eq!(mem::size_of::<FbBitfield>(), 12);
        assert_eq!(mem::size_of::<FbFixScreeninfo>(), 80);
        assert_eq!(mem::size_of::<FbVarScreeninfo>(), 160);
    }
}
