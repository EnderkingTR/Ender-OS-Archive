//! Ağ, servis denetleyicisi ve disk alanı araçları (`ip`, `fetch`, `svc`, `df`).

use std::collections::HashSet;
use std::ffi::CString;
use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::mem;
use std::net::{Ipv4Addr, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::time::Duration;

const SIOCGIFADDR: libc::c_ulong = 0x8915;
const SIOCGIFNETMASK: libc::c_ulong = 0x891B;

#[repr(C)]
struct IfReqAddr {
    ifr_name: [u8; 16],
    addr: libc::sockaddr_in,
    _pad: [u8; 8],
}

fn query_iface_ipv4(iface: &str) -> Option<(Ipv4Addr, u32)> {
    if iface.is_empty() || iface.len() >= 16 {
        return None;
    }
    let mut ifr_name = [0u8; 16];
    ifr_name[..iface.len()].copy_from_slice(iface.as_bytes());

    // SAFETY: AF_INET/SOCK_DGRAM standart kontrol soketi açar.
    let raw_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if raw_fd < 0 {
        return None;
    }
    // SAFETY: `raw_fd >= 0` sahipliği bize ait geçerli bir dosya tanıtıcısıdır.
    let sock = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let fd = sock.as_raw_fd();

    let mut req_addr = IfReqAddr {
        ifr_name,
        // SAFETY: sıfırlanmış `sockaddr_in` geçerli başlangıç durumudur.
        addr: unsafe { mem::zeroed() },
        _pad: [0u8; 8],
    };
    // SAFETY: `SIOCGIFADDR` 40 baytlık `ifreq` tamponuna IPv4 adresini yazar.
    if unsafe { libc::syscall(libc::SYS_ioctl, fd, SIOCGIFADDR, &raw mut req_addr) } < 0 {
        return None;
    }
    let ip = Ipv4Addr::from(req_addr.addr.sin_addr.s_addr.to_ne_bytes());

    let mut req_mask = IfReqAddr {
        ifr_name,
        // SAFETY: sıfırlanmış `sockaddr_in` geçerli başlangıç durumudur.
        addr: unsafe { mem::zeroed() },
        _pad: [0u8; 8],
    };
    // SAFETY: `SIOCGIFNETMASK` 40 baytlık `ifreq` tamponuna alt ağ maskesini yazar.
    let prefix = if unsafe { libc::syscall(libc::SYS_ioctl, fd, SIOCGIFNETMASK, &raw mut req_mask) } >= 0 {
        u32::from_be_bytes(req_mask.addr.sin_addr.s_addr.to_ne_bytes()).count_ones()
    } else {
        0
    };

    Some((ip, prefix))
}

fn hex_le_to_ipv4(hex: &str) -> Option<Ipv4Addr> {
    let raw = u32::from_str_radix(hex, 16).ok()?;
    Some(Ipv4Addr::from(raw.to_le_bytes()))
}

pub fn cmd_ip() -> i32 {
    let entries = match fs::read_dir("/sys/class/net") {
        Ok(rd) => rd,
        Err(err) => {
            eprintln!("ip: /sys/class/net okunamadı: {err}");
            return 1;
        }
    };

    let mut ifaces: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    ifaces.sort();

    println!("Ağ Arayüzleri:");
    for iface in ifaces {
        let state = fs::read_to_string(format!("/sys/class/net/{iface}/operstate"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let mac = fs::read_to_string(format!("/sys/class/net/{iface}/address"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "00:00:00:00:00:00".to_string());
        let inet = match query_iface_ipv4(&iface) {
            Some((ip, prefix)) => format!("{ip}/{prefix}"),
            None => "yok".to_string(),
        };
        println!("  {iface:<8} durum={state:<8} mac={mac}  inet={inet}");
    }

    if let Ok(routes) = fs::read_to_string("/proc/net/route") {
        println!();
        println!("Yönlendirme Tablosu:");
        for line in routes.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 8 {
                continue;
            }
            let iface = cols[0];
            let Some(dst) = hex_le_to_ipv4(cols[1]) else {
                continue;
            };
            let Some(gw) = hex_le_to_ipv4(cols[2]) else {
                continue;
            };
            let Some(mask) = hex_le_to_ipv4(cols[7]) else {
                continue;
            };
            let prefix = u32::from(mask).count_ones();
            if dst.is_unspecified() && !gw.is_unspecified() {
                println!("  default via {gw} dev {iface}");
            } else {
                println!("  {dst}/{prefix} dev {iface}");
            }
        }
    }
    0
}

pub fn cmd_fetch(args: &[String]) -> i32 {
    let mut show_headers = false;
    let mut url_arg: Option<&str> = None;

    for arg in args {
        if arg == "-i" {
            show_headers = true;
        } else if arg.starts_with('-') {
            eprintln!("fetch: geçersiz seçenek '{arg}'");
            return 2;
        } else {
            url_arg = Some(arg);
        }
    }

    let Some(raw_url) = url_arg else {
        eprintln!("fetch: kullanım: fetch [-i] http://host[:port][/yol]");
        return 2;
    };

    let stripped = raw_url.strip_prefix("http://").unwrap_or(raw_url);
    let (host_port, path) = match stripped.split_once('/') {
        Some((hp, rest)) => (hp, format!("/{rest}")),
        None => (stripped, "/".to_string()),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(80)),
        None => (host_port, 80),
    };

    let addr = format!("{host}:{port}");
    let mut stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("fetch: '{addr}' bağlanılamadı: {err}");
            return 1;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let req = format!("GET {path} HTTP/1.0\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
    if let Err(err) = stream.write_all(req.as_bytes()) {
        eprintln!("fetch: istek gönderilemedi: {err}");
        return 1;
    }

    let mut resp = Vec::new();
    if let Err(err) = stream.read_to_end(&mut resp) {
        eprintln!("fetch: yanıt okunamadı: {err}");
        return 1;
    }

    let output = if show_headers {
        resp.as_slice()
    } else if let Some(pos) = resp.windows(4).position(|w| w == b"\r\n\r\n") {
        &resp[pos + 4..]
    } else if let Some(pos) = resp.windows(2).position(|w| w == b"\n\n") {
        &resp[pos + 2..]
    } else {
        resp.as_slice()
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    if let Err(err) = out.write_all(output)
        && err.kind() != ErrorKind::BrokenPipe
    {
        eprintln!("fetch: çıktı hatası: {err}");
        return 1;
    }
    if !output.ends_with(b"\n") {
        let _ = writeln!(out);
    }
    0
}

pub fn cmd_svc() -> i32 {
    let dir = Path::new("/run/services");
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(err) => {
            eprintln!("svc: /run/services okunamadı: {err}");
            return 1;
        }
    };

    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("status"))
        .collect();
    paths.sort();

    println!(
        "{:<16} {:<7} {:<11} {:<8} IKILI",
        "SERVIS", "PID", "DURUM", "RESTART"
    );

    for path in paths {
        if let Ok(raw) = fs::read_to_string(path) {
            let mut name = String::new();
            let mut pid = String::from("0");
            let mut state = String::from("?");
            let mut restarts = String::from("0");
            let mut binary = String::new();

            for line in raw.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match k.trim() {
                        "name" => name = v.trim().to_string(),
                        "pid" => pid = v.trim().to_string(),
                        "state" => state = v.trim().to_string(),
                        "restarts" => restarts = v.trim().to_string(),
                        "binary" => binary = v.trim().to_string(),
                        _ => {}
                    }
                }
            }
            if !name.is_empty() {
                println!(
                    "{:<16} {:<7} {:<11} {:<8} {}",
                    name, pid, state, restarts, binary
                );
            }
        }
    }
    0
}

pub fn cmd_df() -> i32 {
    let mounts = match fs::read_to_string("/proc/mounts") {
        Ok(s) => s,
        Err(err) => {
            eprintln!("df: /proc/mounts okunamadı: {err}");
            return 1;
        }
    };

    println!(
        "{:<12} {:<10} {:<8} {:<11} {:<13} BOS(KB)",
        "AYGIT", "BAGLANTI", "TIP", "TOPLAM(KB)", "KULLANILAN"
    );

    let mut seen = HashSet::new();
    for line in mounts.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }
        let dev = cols[0];
        let target = cols[1];
        let fstype = cols[2];

        let interesting = dev.starts_with("/dev/")
            || matches!(target, "/" | "/tmp" | "/run" | "/data");
        if !interesting || !seen.insert(target.to_string()) {
            continue;
        }

        let Ok(c_target) = CString::new(target) else {
            continue;
        };

        // SAFETY: `statvfs` sıfırlanmış bir C yapısıdır ve `c_target` geçerli null-sonlandırmalıdır.
        let mut st: libc::statvfs = unsafe { mem::zeroed() };
        if unsafe { libc::statvfs(c_target.as_ptr(), &mut st) } != 0 {
            continue;
        }

        let block_size = if st.f_frsize > 0 {
            st.f_frsize as u64
        } else {
            st.f_bsize as u64
        };
        let total_kb = (st.f_blocks as u64).saturating_mul(block_size) / 1024;
        let free_kb = (st.f_bfree as u64).saturating_mul(block_size) / 1024;
        let avail_kb = (st.f_bavail as u64).saturating_mul(block_size) / 1024;
        let used_kb = total_kb.saturating_sub(free_kb);

        println!(
            "{:<12} {:<10} {:<8} {:<11} {:<13} {}",
            dev, target, fstype, total_kb, used_kb, avail_kb
        );
    }
    0
}
