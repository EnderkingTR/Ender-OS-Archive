//! Çekirdek ve süreç yönetim araçları (`ps`, `uname`, `uptime`, `meminfo`, `poweroff`, `reboot`).

use std::ffi::CStr;
use std::fs;
use std::io::{self, Write};
use std::mem;

struct ProcInfo {
    pid: u32,
    ppid: String,
    state: String,
    rss_kb: String,
    name: String,
}

fn read_proc_status(pid: u32) -> Option<ProcInfo> {
    let status_path = format!("/proc/{pid}/status");
    let contents = fs::read_to_string(status_path).ok()?;

    let mut name = String::from("?");
    let mut state = String::from("?");
    let mut ppid = String::from("0");
    let mut rss_kb = String::from("0");

    for line in contents.lines() {
        if let Some(val) = line.strip_prefix("Name:") {
            name = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("State:") {
            if let Some(ch) = val.trim().chars().next() {
                state = ch.to_string();
            }
        } else if let Some(val) = line.strip_prefix("PPid:") {
            ppid = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("VmRSS:")
            && let Some(num) = val.split_whitespace().next()
        {
            rss_kb = num.to_string();
        }
    }

    Some(ProcInfo {
        pid,
        ppid,
        state,
        rss_kb,
        name,
    })
}

pub fn cmd_ps() -> i32 {
    let entries = match fs::read_dir("/proc") {
        Ok(rd) => rd,
        Err(err) => {
            eprintln!("ps: /proc okunamadı: {err}");
            return 1;
        }
    };

    let mut pids: Vec<u32> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .collect();
    pids.sort_unstable();

    println!(
        "{:<7} {:<7} {:<6} {:<8} CMD",
        "PID", "PPID", "STATE", "RSS(kB)"
    );
    for pid in pids {
        if let Some(info) = read_proc_status(pid) {
            println!(
                "{:<7} {:<7} {:<6} {:<8} {}",
                info.pid, info.ppid, info.state, info.rss_kb, info.name
            );
        }
    }
    0
}

fn c_char_field(buf: &[libc::c_char]) -> String {
    // SAFETY: `libc::uname` başarıyla döndükten sonra `utsname` alanları çekirdek
    // tarafından null karakterle sonlandırılmış geçerli C dizgileridir.
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

pub fn cmd_uname(args: &[String]) -> i32 {
    let mut show_s = false;
    let mut show_n = false;
    let mut show_r = false;
    let mut show_v = false;
    let mut show_m = false;

    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 {
            for ch in arg[1..].chars() {
                match ch {
                    'a' => {
                        show_s = true;
                        show_n = true;
                        show_r = true;
                        show_v = true;
                        show_m = true;
                    }
                    's' => show_s = true,
                    'n' => show_n = true,
                    'r' => show_r = true,
                    'v' => show_v = true,
                    'm' => show_m = true,
                    other => {
                        eprintln!("uname: geçersiz seçenek '-{other}'");
                        return 2;
                    }
                }
            }
        } else {
            eprintln!("uname: beklenmeyen argüman '{arg}'");
            return 2;
        }
    }

    if !show_s && !show_n && !show_r && !show_v && !show_m {
        show_s = true;
    }

    // SAFETY: `utsname` yalnızca C char dizilerinden oluşan bir POD yapıdır;
    // sıfırlanıp çekirdek `uname` sistem çağrısına geçirilir.
    let mut uts: libc::utsname = unsafe { mem::zeroed() };
    if unsafe { libc::uname(&mut uts) } != 0 {
        eprintln!("uname: {}", io::Error::last_os_error());
        return 1;
    }

    let mut parts = Vec::new();
    if show_s {
        parts.push(c_char_field(&uts.sysname));
    }
    if show_n {
        parts.push(c_char_field(&uts.nodename));
    }
    if show_r {
        parts.push(c_char_field(&uts.release));
    }
    if show_v {
        parts.push(c_char_field(&uts.version));
    }
    if show_m {
        parts.push(c_char_field(&uts.machine));
    }

    println!("{}", parts.join(" "));
    0
}

pub fn cmd_uptime() -> i32 {
    let uptime_raw = match fs::read_to_string("/proc/uptime") {
        Ok(s) => s,
        Err(err) => {
            eprintln!("uptime: /proc/uptime okunamadı: {err}");
            return 1;
        }
    };

    let secs: u64 = uptime_raw
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0);

    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let rem_secs = secs % 60;

    let load = fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| {
            s.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "0.00, 0.00, 0.00".to_string());

    println!("açık kalma: {hours:02}:{mins:02}:{rem_secs:02}, yük ortalaması: {load}");
    0
}

pub fn cmd_meminfo() -> i32 {
    match fs::read_to_string("/proc/meminfo") {
        Ok(contents) => {
            for line in contents.lines() {
                if line.starts_with("MemTotal:")
                    || line.starts_with("MemFree:")
                    || line.starts_with("MemAvailable:")
                    || line.starts_with("Buffers:")
                    || line.starts_with("Cached:")
                {
                    println!("{line}");
                }
            }
            0
        }
        Err(err) => {
            eprintln!("meminfo: /proc/meminfo okunamadı: {err}");
            1
        }
    }
}

fn do_power_action(cmd: libc::c_int, message: &str) -> i32 {
    // SAFETY: geteuid() salt okunur bir POSIX çağrısıdır.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("HATA: Bu komut yalnızca root yetkisiyle çalıştırılabilir.");
        return 1;
    }

    println!("{message}");
    let _ = io::stdout().flush();

    // SAFETY: root yetkisi doğrulandı; sync() disk tamponlarını boşaltır,
    // reboot(cmd) geçerli RB_POWER_OFF veya RB_AUTOBOOT sabitini kullanır.
    unsafe {
        libc::sync();
        if libc::reboot(cmd) != 0 {
            eprintln!("Sistem güç çağrısı başarısız: {}", io::Error::last_os_error());
            return 1;
        }
    }
    0
}

pub fn cmd_poweroff() -> i32 {
    do_power_action(libc::RB_POWER_OFF, "Ender-OS kapatılıyor. Görüşmek üzere!")
}

pub fn cmd_reboot() -> i32 {
    do_power_action(libc::RB_AUTOBOOT, "Ender-OS yeniden başlatılıyor...")
}
