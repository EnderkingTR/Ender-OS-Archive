//! Ender-OS v0.4 — PID 1 (`/init`) Sistem ve Servis Denetleyicisi.
//!
//! Sanal dosya sistemlerini ve kalıcı `ext4` diski (`/data`) bağlar, ağ
//! arayüzlerini (`lo`, `eth0`) yapılandırır, `ender_dev.ko` çekirdek modülünü
//! otomatik yükler ve arka plan Web Dashboard + etkileşimli kabuğu yönetir.

mod net;
mod supervisor;

use std::env;
use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::ptr;
use std::thread;
use std::time::Duration;

const OS_VERSION: &str = "Ender-OS v0.6";
const HOSTNAME: &str = "ender-os";
const DEFAULT_KMOD: &str = "/lib/modules/ender_dev.ko";

/// Belirtilen dosya sistemini bağlar; zaten bağlıysa (EBUSY) başarı kabul eder.
fn mount_fs(source: &str, target: &str, fstype: &str) -> io::Result<()> {
    fs::create_dir_all(target)?;

    let to_cstring =
        |s: &str| CString::new(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e));

    let c_source = to_cstring(source)?;
    let c_target = to_cstring(target)?;
    let c_fstype = to_cstring(fstype)?;

    // SAFETY: Tüm işaretçiler geçerli null-sonlandırmalı CString nesnelerinden gelir
    // ve data parametresi standart bağlamalar için null geçilir.
    let rc = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            c_fstype.as_ptr(),
            0,
            ptr::null(),
        )
    };

    if rc == 0 {
        return Ok(());
    }

    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EBUSY) {
        return Ok(());
    }
    Err(err)
}

fn mount_essential_filesystems() {
    let mounts = [
        ("proc", "/proc", "proc"),
        ("sysfs", "/sys", "sysfs"),
        ("devtmpfs", "/dev", "devtmpfs"),
        ("tmpfs", "/tmp", "tmpfs"),
        ("tmpfs", "/run", "tmpfs"),
    ];

    for (source, target, fstype) in mounts {
        if let Err(err) = mount_fs(source, target, fstype) {
            eprintln!("UYARI: {target} ({fstype}) bağlanamadı: {err}");
        }
    }
}

fn mount_persistent_disk() -> bool {
    if !Path::new("/dev/vda").exists() {
        return false;
    }

    match mount_fs("/dev/vda", "/data", "ext4") {
        Ok(()) => {
            if !Path::new("/data/welcome.txt").exists() {
                let _ = fs::write(
                    "/data/welcome.txt",
                    "Ender-OS v0.4 kalici ext4 diski (/dev/vda -> /data) hazir!\n",
                );
            }
            true
        }
        Err(err) => {
            eprintln!("UYARI: /dev/vda (/data ext4) bağlanamadı: {err}");
            false
        }
    }
}

fn set_hostname(name: &str) {
    // SAFETY: `name` geçerli UTF-8 bayt dilimi olup `name.len()` geçerli bellek aralığıdır.
    let rc = unsafe { libc::sethostname(name.as_ptr().cast::<libc::c_char>(), name.len()) };
    if rc != 0 {
        eprintln!(
            "UYARI: hostname '{name}' ayarlanamadı: {}",
            io::Error::last_os_error()
        );
    }
}

fn set_env_var(key: &str, value: &str) {
    // SAFETY: PID 1 servisleri başlatmadan önce tek iş parçacıklı olarak çalışır.
    unsafe {
        env::set_var(key, value);
    }
}

fn configure_tty_utf8() {
    // SAFETY: `termios` tüm alanları tamsayı/dizi olan bir C yapısıdır; `tcgetattr`
    // başarısı kontrol edildikten sonra yalnızca `IUTF8` bayrağı etkinleştirilir.
    unsafe {
        let mut term: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(libc::STDIN_FILENO, &mut term) == 0 {
            term.c_iflag |= libc::IUTF8;
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &term);
        }
    }
}

fn ensure_base_files() {
    for dir in ["/root", "/etc", "/var/log", "/run/services", "/data"] {
        let _ = fs::create_dir_all(dir);
    }

    if !Path::new("/etc/os-release").exists() {
        let content = "NAME=\"Ender-OS\"\nVERSION=\"0.5\"\nID=enderos\nPRETTY_NAME=\"Ender-OS v0.5\"\n";
        let _ = fs::write("/etc/os-release", content);
    }
}

fn autoload_kernel_module(path: &str) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };

    // SAFETY: `bytes` geçerli ELF modülü, `c""` statik null-sonlandırmalı parametre dizgisidir.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_init_module,
            bytes.as_ptr(),
            bytes.len(),
            c"".as_ptr(),
        )
    };
    if rc != 0 {
        eprintln!(
            "UYARI: {path} otomatik yüklenemedi: {}",
            io::Error::last_os_error()
        );
        return false;
    }
    true
}

fn read_kernel_version() -> String {
    fs::read_to_string("/proc/version")
        .ok()
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "bilinmiyor".to_string())
}

fn read_uptime() -> String {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|u| format!("{u} sn")))
        .unwrap_or_else(|| "bilinmiyor".to_string())
}

fn print_banner(pid: u32, net_ok: bool, disk_ok: bool, kmod_ok: bool) {
    println!();
    println!("================================================");
    println!("  {OS_VERSION} — Ağ, Web Paneli & Kalıcı Disk");
    println!("================================================");
    println!("  PID      : {pid}");
    println!("  Makine   : {HOSTNAME}");
    println!("  Çekirdek : {}", read_kernel_version());
    println!("  Uptime   : {}", read_uptime());
    println!(
        "  Ağ (eth0): {}",
        if net_ok {
            "10.0.2.15/24 (gw 10.0.2.2) | Web Panel: :8080"
        } else {
            "devre dışı"
        }
    );
    println!(
        "  Disk     : {}",
        if disk_ok {
            "/dev/vda -> /data (ext4 kalıcı)"
        } else {
            "yok (yalnızca RAM)"
        }
    );
    println!(
        "  Modül    : {}",
        if kmod_ok {
            "ender_dev.ko (/dev/ender hazır)"
        } else {
            "yüklü değil"
        }
    );
    println!("  Komutlar : help, ip, svc, df, fetch, enderctl, ender-fb, poweroff");
    println!("================================================");
    println!();
}

fn parse_kernel_root_arg() -> Option<String> {
    let cmdline = fs::read_to_string("/proc/cmdline").ok()?;
    for tok in cmdline.split_whitespace() {
        if let Some(dev) = tok.strip_prefix("root=")
            && !dev.is_empty()
        {
            return Some(dev.to_string());
        }
    }
    None
}

fn move_mount(src: &str, dst: &str) {
    let _ = fs::create_dir_all(dst);
    let Ok(c_src) = CString::new(src) else { return };
    let Ok(c_dst) = CString::new(dst) else { return };
    // SAFETY: `MS_MOVE` çekirdeğe mevcut bağlama noktasını yeni kök altına taşımasını söyler.
    unsafe {
        libc::mount(
            c_src.as_ptr(),
            c_dst.as_ptr(),
            ptr::null(),
            libc::MS_MOVE,
            ptr::null(),
        );
    }
}

fn try_switch_root(root_dev: &str) {
    for _ in 0..30 {
        if Path::new(root_dev).exists() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    if let Err(err) = mount_fs(root_dev, "/newroot", "ext4") {
        eprintln!("UYARI: {root_dev} -> /newroot bağlanamadı: {err}");
        return;
    }

    if !Path::new("/newroot/sbin/init").exists() {
        eprintln!("UYARI: /newroot/sbin/init bulunamadı; initramfs modunda devam ediliyor.");
        return;
    }

    move_mount("/proc", "/newroot/proc");
    move_mount("/sys", "/newroot/sys");
    move_mount("/dev", "/newroot/dev");
    move_mount("/run", "/newroot/run");

    if env::set_current_dir("/newroot").is_err() {
        return;
    }

    // SAFETY: `/newroot` geçerli bağlama noktasıdır; kök değiştirilip `/sbin/init` çalıştırılır.
    unsafe {
        libc::mount(
            c".".as_ptr(),
            c"/".as_ptr(),
            ptr::null(),
            libc::MS_MOVE,
            ptr::null(),
        );
        if libc::chroot(c".".as_ptr()) != 0 {
            eprintln!("HATA: chroot başarısız: {}", io::Error::last_os_error());
            return;
        }
    }
    let _ = env::set_current_dir("/");

    let err = Command::new("/sbin/init").exec();
    eprintln!("HATA: /sbin/init (switch_root) başlatılamadı: {err}");
}

fn main() {
    let pid = std::process::id();

    if pid != 1 {
        println!("[Test Modu] ender-init PID={pid} ile çalışıyor; sistem başlatma atlanıyor.");
        return;
    }

    mount_essential_filesystems();

    if let Some(root_dev) = parse_kernel_root_arg() {
        try_switch_root(&root_dev);
    }
    set_hostname(HOSTNAME);
    ensure_base_files();
    configure_tty_utf8();

    set_env_var("PATH", "/bin:/sbin");
    set_env_var("HOME", "/root");
    set_env_var("USER", "root");
    set_env_var("TERM", "vt100");

    let _ = env::set_current_dir("/root");

    let disk_ok = mount_persistent_disk();
    let kmod_ok = autoload_kernel_module(DEFAULT_KMOD);
    let net_ok = net::setup_networking();

    print_banner(pid, net_ok, disk_ok, kmod_ok);
    supervisor::run_supervisor();
}
