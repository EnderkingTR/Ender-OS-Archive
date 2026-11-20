//! Ender-OS v0.6 — Masaüstü Telemetri & Oyun Modu Aracı (`ender-hud`).
//!
//! Canlı CPU/RAM kullanımını, `/dev/ender` çekirdek modülü durumunu ve
//! Oyun Modu (`gamemode`) bilgisini Waybar JSON formatında veya terminalde sunar.

use std::env;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

const GAMEMODE_FLAG: &str = "/run/ender-gamemode";

fn read_cpu_sample() -> Option<(u64, u64)> {
    let raw = fs::read_to_string("/proc/stat").ok()?;
    let line = raw.lines().find(|l| l.starts_with("cpu "))?;
    parse_cpu_line(line)
}

pub fn parse_cpu_line(line: &str) -> Option<(u64, u64)> {
    let nums: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    if nums.len() < 4 {
        return None;
    }
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
    let total: u64 = nums.iter().sum();
    Some((total, idle))
}

pub fn calc_cpu_percent(prev: (u64, u64), curr: (u64, u64)) -> u32 {
    let dt = curr.0.saturating_sub(prev.0);
    let di = curr.1.saturating_sub(prev.1);
    if dt == 0 {
        return 0;
    }
    let busy = dt.saturating_sub(di);
    ((busy * 100) / dt) as u32
}

fn read_mem_mb() -> (u64, u64, u32) {
    let mut total_kb = 0u64;
    let mut avail_kb = 0u64;
    if let Ok(raw) = fs::read_to_string("/proc/meminfo") {
        for line in raw.lines() {
            let val = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            if line.starts_with("MemTotal:") {
                total_kb = val;
            } else if line.starts_with("MemAvailable:") {
                avail_kb = val;
            }
        }
    }
    let used_kb = total_kb.saturating_sub(avail_kb);
    let pct = if total_kb > 0 {
        ((used_kb * 100) / total_kb) as u32
    } else {
        0
    };
    (used_kb / 1024, total_kb / 1024, pct)
}

fn is_gamemode() -> bool {
    Path::new(GAMEMODE_FLAG).exists()
}

pub fn format_waybar_json(cpu_pct: u32, mem_used_mb: u64, mem_total_mb: u64, mem_pct: u32, gamemode: bool) -> String {
    let mode_tag = if gamemode { "GAME" } else { "DEV" };
    let class = if gamemode { "gamemode" } else { "normal" };
    let text = format!("Ender-OS [{mode_tag}] | CPU {cpu_pct}% | RAM {mem_used_mb}/{mem_total_mb} MB ({mem_pct}%)");
    let tooltip = format!(
        "Ender-OS v0.6\\nMod: {mode_tag}\\nCPU: {cpu_pct}%\\nRAM: {mem_used_mb} / {mem_total_mb} MB"
    );
    format!("{{\"text\":\"{text}\",\"tooltip\":\"{tooltip}\",\"class\":\"{class}\"}}")
}

fn cmd_waybar() {
    let s1 = read_cpu_sample().unwrap_or((0, 0));
    thread::sleep(Duration::from_millis(200));
    let s2 = read_cpu_sample().unwrap_or(s1);
    let cpu_pct = calc_cpu_percent(s1, s2);
    let (used_mb, total_mb, mem_pct) = read_mem_mb();
    println!(
        "{}",
        format_waybar_json(cpu_pct, used_mb, total_mb, mem_pct, is_gamemode())
    );
}

fn cmd_gamemode(arg: Option<&str>) -> i32 {
    let target_on = match arg {
        Some("on") => true,
        Some("off") => false,
        Some("toggle") => !is_gamemode(),
        Some("status") | None => {
            println!("Oyun Modu: {}", if is_gamemode() { "AKTIF" } else { "KAPALI" });
            return 0;
        }
        Some(other) => {
            eprintln!("Kullanım: ender-hud gamemode [on | off | toggle | status] (geçersiz: {other})");
            return 2;
        }
    };

    if target_on {
        if let Err(err) = fs::write(GAMEMODE_FLAG, "1\n") {
            eprintln!("ender-hud: oyun modu açılamadı: {err}");
            return 1;
        }
        if Path::new("/dev/ender").exists() {
            let _ = fs::write("/dev/ender", "Ender-OS Oyun Modu AKTIF!\n");
        }
        println!("Oyun Modu: AKTIF");
    } else {
        let _ = fs::remove_file(GAMEMODE_FLAG);
        if Path::new("/dev/ender").exists() {
            let _ = fs::write("/dev/ender", "Ender-OS Gelistirici Modu AKTIF!\n");
        }
        println!("Oyun Modu: KAPALI");
    }
    0
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("waybar") => cmd_waybar(),
        Some("gamemode") => std::process::exit(cmd_gamemode(args.get(2).map(String::as_str))),
        _ => {
            let (used_mb, total_mb, mem_pct) = read_mem_mb();
            println!(
                "Ender-OS v0.6 | Mod: {} | RAM: {used_mb}/{total_mb} MB ({mem_pct}%)",
                if is_gamemode() { "GAME" } else { "DEV" }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat_cpu_and_computes_percent() {
        let s1 = parse_cpu_line("cpu  100 0 50 850 0 0 0 0 0 0").unwrap();
        let s2 = parse_cpu_line("cpu  150 0 50 900 0 0 0 0 0 0").unwrap();
        assert_eq!(calc_cpu_percent(s1, s2), 50);
    }

    #[test]
    fn formats_waybar_json_cleanly() {
        let out = format_waybar_json(12, 512, 2048, 25, true);
        assert!(out.contains("\"class\":\"gamemode\""));
        assert!(out.contains("CPU 12%"));
        assert!(out.contains("RAM 512/2048 MB (25%)"));
    }
}
