//! Ender-OS v0.2 — Etkileşimli Rust Kabuğu (`ender-sh`).

mod exec;
mod parser;

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use parser::{Pipeline, parse_pipeline};

fn read_hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "(none)")
        .unwrap_or_else(|| "ender-os".to_string())
}

fn format_cwd() -> String {
    let cwd = match env::current_dir() {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => "?".to_string(),
    };

    if let Ok(home) = env::var("HOME")
        && !home.is_empty()
    {
        if cwd == home {
            return "~".to_string();
        }
        if let Some(rest) = cwd.strip_prefix(&home)
            && rest.starts_with('/')
        {
            return format!("~{rest}");
        }
    }

    cwd
}

fn list_external_commands() -> Vec<String> {
    let mut cmds = Vec::new();
    for dir in ["/bin", "/sbin"] {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    cmds.push(name.to_string());
                }
            }
        }
    }
    cmds.sort();
    cmds.dedup();
    cmds
}

fn builtin_cd(args: &[String]) -> i32 {
    let target = match args.first() {
        None => env::var("HOME").unwrap_or_else(|_| "/".to_string()),
        Some(arg) if arg == "~" => env::var("HOME").unwrap_or_else(|_| "/".to_string()),
        Some(arg) => arg.clone(),
    };

    if let Err(err) = env::set_current_dir(&target) {
        eprintln!("cd: '{target}': {err}");
        return 1;
    }
    0
}

fn builtin_export(args: &[String]) -> i32 {
    if args.is_empty() {
        return builtin_env();
    }

    for arg in args {
        let Some((key, value)) = arg.split_once('=') else {
            eprintln!("export: geçersiz ifade '{arg}' (kullanım: export AD=DEĞER)");
            return 1;
        };
        if key.is_empty() {
            eprintln!("export: değişken adı boş olamaz");
            return 1;
        }
        // SAFETY: `ender-sh` tek iş parçacıklıdır; ortam değişkeni yalnızca ana döngüde güncellenir.
        unsafe {
            env::set_var(key, value);
        }
    }
    0
}

fn builtin_env() -> i32 {
    let mut vars: Vec<(String, String)> = env::vars().collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in vars {
        println!("{k}={v}");
    }
    0
}

fn builtin_help() -> i32 {
    println!("Ender-OS Kabuğu (ender-sh v0.2)");
    println!();
    println!("Yerleşik kabuk komutları:");
    println!("  cd [dizin]       Çalışma dizinini değiştirir");
    println!("  export AD=DEĞER  Ortam değişkeni tanımlar");
    println!("  env              Tüm ortam değişkenlerini listeler");
    println!("  help             Bu yardım bilgisini gösterir");
    println!("  exit [kod]       Kabuk oturumunu sonlandırır");
    println!();

    let externals = list_external_commands();
    if !externals.is_empty() {
        println!("Sistem komutları (/bin, /sbin):");
        println!("  {}", externals.join("  "));
        println!();
    }

    println!("Boru hattı ve yönlendirme örnekleri:");
    println!("  cat /proc/meminfo | grep Mem | head -n 3");
    println!("  echo \"Merhaba Ender-OS\" > /tmp/not.txt");
    println!("  uname -a >> /tmp/not.txt");
    0
}

enum BuiltinOutcome {
    NotBuiltin,
    Handled(i32),
    Exit(i32),
}

fn try_builtin(pipeline: &Pipeline, last_status: i32) -> BuiltinOutcome {
    if pipeline.stages.len() != 1 || pipeline.stages[0].stdout_redirect.is_some() {
        return BuiltinOutcome::NotBuiltin;
    }

    let stage = &pipeline.stages[0];
    match stage.program.as_str() {
        "cd" => BuiltinOutcome::Handled(builtin_cd(&stage.args)),
        "export" => BuiltinOutcome::Handled(builtin_export(&stage.args)),
        "env" => BuiltinOutcome::Handled(builtin_env()),
        "help" => BuiltinOutcome::Handled(builtin_help()),
        "exit" => {
            let code = stage
                .args
                .first()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(last_status);
            BuiltinOutcome::Exit(code)
        }
        _ => BuiltinOutcome::NotBuiltin,
    }
}

fn main() {
    let hostname = read_hostname();
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut raw_line: Vec<u8> = Vec::new();
    let mut last_status = 0;

    // İlk kurulumda PATH ayarlanmamışsa standart dizinleri tanımla.
    if env::var_os("PATH").is_none() && Path::new("/bin").exists() {
        // SAFETY: Tek iş parçacıklı başlangıç adımıdır.
        unsafe {
            env::set_var("PATH", "/bin:/sbin");
        }
    }

    loop {
        print!("root@{hostname}:{}# ", format_cwd());
        let _ = io::stdout().flush();

        raw_line.clear();
        match reader.read_until(b'\n', &mut raw_line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(err) => {
                eprintln!("ender-sh: girdi okuma hatası: {err}");
                last_status = 1;
                break;
            }
        }

        let line = match std::str::from_utf8(&raw_line) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("ender-sh: geçersiz UTF-8 baytı algılandı, satır yoksayıldı.");
                last_status = 1;
                continue;
            }
        };

        let pipeline = match parse_pipeline(line) {
            Ok(Some(p)) => p,
            Ok(None) => continue,
            Err(err) => {
                eprintln!("ender-sh: sözdizimi hatası: {err}");
                last_status = 2;
                continue;
            }
        };

        match try_builtin(&pipeline, last_status) {
            BuiltinOutcome::Handled(code) => last_status = code,
            BuiltinOutcome::Exit(code) => std::process::exit(code),
            BuiltinOutcome::NotBuiltin => last_status = exec::execute_pipeline(&pipeline),
        }
    }

    std::process::exit(last_status);
}
