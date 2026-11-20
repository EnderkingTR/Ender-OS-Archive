//! Çekirdek modülü yönetimi ve `/dev/ender` kontrol araçları
//! (`insmod`, `rmmod`, `lsmod`, `dmesg`, `enderctl`).

use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::os::fd::AsRawFd;
use std::ptr;

const ENDER_IOCTL_RESET: u32 = (('E' as u32) << 8) | 0x01;
const ENDER_IOCTL_GET_OPENS: u32 = (2 << 30) | (8 << 16) | (('E' as u32) << 8) | 0x02;
const ENDER_IOCTL_GET_WRITES: u32 = (2 << 30) | (8 << 16) | (('E' as u32) << 8) | 0x03;
const ENDER_IOCTL_GET_READS: u32 = (2 << 30) | (8 << 16) | (('E' as u32) << 8) | 0x04;

const SYSLOG_ACTION_READ_ALL: libc::c_int = 3;
const SYSLOG_ACTION_SIZE_BUFFER: libc::c_int = 10;

pub fn cmd_insmod(args: &[String]) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("insmod: kullanım: insmod <modul.ko> [parametreler...]");
        return 1;
    };

    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(err) => {
            eprintln!("insmod: '{path}' okunamadı: {err}");
            return 1;
        }
    };

    let params_str = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        String::new()
    };

    let c_params = match CString::new(params_str) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("insmod: geçersiz parametre: {err}");
            return 1;
        }
    };

    // SAFETY: `bytes` geçerli bir ELF çekirdek modülü tamponu, `c_params` null-sonlandırmalı
    // C dizgisi olarak `SYS_init_module` sistem çağrısına geçirilir.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_init_module,
            bytes.as_ptr(),
            bytes.len(),
            c_params.as_ptr(),
        )
    };

    if rc != 0 {
        eprintln!("insmod: '{path}' yüklenemedi: {}", io::Error::last_os_error());
        return 1;
    }
    0
}

pub fn cmd_rmmod(args: &[String]) -> i32 {
    let Some(name) = args.first() else {
        eprintln!("rmmod: kullanım: rmmod <modul_adi>");
        return 1;
    };

    let c_name = match CString::new(name.as_str()) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("rmmod: geçersiz modül adı: {err}");
            return 1;
        }
    };

    // SAFETY: `c_name` geçerli, null-sonlandırmalı modül adıdır; `SYS_delete_module`
    // ile modül çekirdekten güvenli biçimde kaldırılır.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_delete_module,
            c_name.as_ptr(),
            libc::O_NONBLOCK as libc::c_ulong,
        )
    };

    if rc != 0 {
        eprintln!("rmmod: '{name}' kaldırılamadı: {}", io::Error::last_os_error());
        return 1;
    }
    0
}

pub fn cmd_lsmod() -> i32 {
    let contents = match fs::read_to_string("/proc/modules") {
        Ok(s) => s,
        Err(err) => {
            eprintln!("lsmod: /proc/modules okunamadı: {err}");
            return 1;
        }
    };

    println!("{:<24} {:<10} KULLANIM", "MODUL", "BOYUT");
    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let size = parts.next().unwrap_or("0");
        let refs = parts.next().unwrap_or("0");
        let used_by = parts.next().unwrap_or("-");
        println!("{:<24} {:<10} {} ({})", name, size, refs, used_by);
    }
    0
}

pub fn cmd_dmesg() -> i32 {
    // SAFETY: SYSLOG_ACTION_SIZE_BUFFER tampon adresini kullanmaz; yalnızca çekirdek
    // halka tamponunun boyutunu döndürür.
    let raw_size = unsafe {
        libc::syscall(
            libc::SYS_syslog,
            SYSLOG_ACTION_SIZE_BUFFER,
            ptr::null_mut::<u8>(),
            0,
        )
    };

    let buf_size = if raw_size > 0 {
        raw_size as usize
    } else {
        16384
    };

    let mut buf = vec![0u8; buf_size];
    // SAFETY: `buf` `buf_size` bayt ayrılmış geçerli bir yazılabilir tampondur.
    let read_len = unsafe {
        libc::syscall(
            libc::SYS_syslog,
            SYSLOG_ACTION_READ_ALL,
            buf.as_mut_ptr(),
            buf.len() as libc::c_int,
        )
    };

    if read_len < 0 {
        eprintln!("dmesg: çekirdek günlüğü okunamadı: {}", io::Error::last_os_error());
        return 1;
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let slice = &buf[..read_len as usize];
    if let Err(err) = out.write_all(slice)
        && err.kind() != ErrorKind::BrokenPipe
    {
        eprintln!("dmesg: yazma hatası: {err}");
        return 1;
    }
    if !slice.ends_with(b"\n") {
        let _ = writeln!(out);
    }
    0
}

fn ioctl_get_u64(fd: i32, req: u32) -> io::Result<u64> {
    let mut val: u64 = 0;
    // SAFETY: `fd` açık bir `/dev/ender` tanıtıcısı, `val` geçerli bir `u64` işaretçisidir.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_ioctl,
            fd,
            req as libc::c_ulong,
            (&raw mut val).cast::<libc::c_void>(),
        )
    };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(val)
    }
}

pub fn cmd_enderctl(args: &[String]) -> i32 {
    let subcmd = args.first().map(String::as_str).unwrap_or("stats");

    if subcmd != "stats" && subcmd != "reset" {
        eprintln!("Kullanım: enderctl [stats | reset]");
        return 2;
    }

    let file = match OpenOptions::new().read(true).write(true).open("/dev/ender") {
        Ok(f) => f,
        Err(err) => {
            eprintln!("enderctl: /dev/ender açılamadı: {err}");
            eprintln!("İpucu: Modül yüklü değilse 'insmod /lib/modules/ender_dev.ko' çalıştırın.");
            return 1;
        }
    };
    let fd = file.as_raw_fd();

    if subcmd == "reset" {
        // SAFETY: `ENDER_IOCTL_RESET` argüman almayan bir `_IO` çağrısıdır.
        let rc = unsafe { libc::syscall(libc::SYS_ioctl, fd, ENDER_IOCTL_RESET as libc::c_ulong, 0usize) };
        if rc < 0 {
            eprintln!("enderctl: reset başarısız: {}", io::Error::last_os_error());
            return 1;
        }
        println!("ender_dev: tampon ve sayaçlar sıfırlandı.");
        return 0;
    }

    let opens = match ioctl_get_u64(fd, ENDER_IOCTL_GET_OPENS) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("enderctl: açılış sayacı okunamadı: {err}");
            return 1;
        }
    };
    let reads = match ioctl_get_u64(fd, ENDER_IOCTL_GET_READS) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("enderctl: okuma sayacı okunamadı: {err}");
            return 1;
        }
    };
    let writes = match ioctl_get_u64(fd, ENDER_IOCTL_GET_WRITES) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("enderctl: yazma sayacı okunamadı: {err}");
            return 1;
        }
    };

    println!("/dev/ender Çekirdek Modülü İstatistikleri:");
    println!("  Açılış (open) : {opens}");
    println!("  Okuma  (read) : {reads}");
    println!("  Yazma  (write): {writes}");
    0
}
