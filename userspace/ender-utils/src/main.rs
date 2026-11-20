//! Ender-OS v0.2 — Çoklu-komut (`multi-call`) sistem araçları.
//!
//! Tek bir statik Rust ikili dosyası olarak derlenir; çağrıldığı sembolik link
//! adına (`ls`, `cat`, `ps`, `uname` vb.) göre ilgili alt komutu çalıştırır.

mod applets;

use std::env;
use std::path::Path;
use std::process;

const APPLET_NAMES: &[&str] = &[
    "ls", "cat", "mkdir", "rm", "touch", "pwd", "echo", "head", "grep", "clear", "ps", "uname",
    "uptime", "meminfo", "poweroff", "reboot", "insmod", "rmmod", "lsmod", "dmesg", "enderctl",
    "ip", "fetch", "svc", "df",
];

fn dispatch(cmd: &str, args: &[String]) -> i32 {
    match cmd {
        "ls" => applets::fs_ops::cmd_ls(args),
        "cat" => applets::fs_ops::cmd_cat(args),
        "mkdir" => applets::fs_ops::cmd_mkdir(args),
        "rm" => applets::fs_ops::cmd_rm(args),
        "touch" => applets::fs_ops::cmd_touch(args),
        "pwd" => applets::fs_ops::cmd_pwd(),
        "echo" => applets::text::cmd_echo(args),
        "head" => applets::text::cmd_head(args),
        "grep" => applets::text::cmd_grep(args),
        "clear" => applets::text::cmd_clear(),
        "ps" => applets::sys::cmd_ps(),
        "uname" => applets::sys::cmd_uname(args),
        "uptime" => applets::sys::cmd_uptime(),
        "meminfo" => applets::sys::cmd_meminfo(),
        "poweroff" => applets::sys::cmd_poweroff(),
        "reboot" => applets::sys::cmd_reboot(),
        "insmod" => applets::kmod::cmd_insmod(args),
        "rmmod" => applets::kmod::cmd_rmmod(args),
        "lsmod" => applets::kmod::cmd_lsmod(),
        "dmesg" => applets::kmod::cmd_dmesg(),
        "enderctl" => applets::kmod::cmd_enderctl(args),
        "ip" => applets::net_svc::cmd_ip(),
        "fetch" => applets::net_svc::cmd_fetch(args),
        "svc" => applets::net_svc::cmd_svc(),
        "df" => applets::net_svc::cmd_df(),
        other => {
            eprintln!("ender-utils: bilinmeyen komut '{other}'");
            127
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let invoked = args
        .first()
        .and_then(|s| Path::new(s).file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("ender-utils");

    let code = if invoked == "ender-utils" {
        if args.len() < 2 {
            println!("Ender-OS Sistem Araçları (ender-utils v0.5)");
            println!("Kullanım: ender-utils <komut> [argümanlar...]");
            println!("Desteklenen komutlar: {}", APPLET_NAMES.join(", "));
            0
        } else {
            dispatch(&args[1], &args[2..])
        }
    } else {
        dispatch(invoked, &args[1..])
    };

    process::exit(code);
}
