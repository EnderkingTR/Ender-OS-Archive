//! Dosya sistemi ve dizin işlem araçları (`ls`, `cat`, `mkdir`, `rm`, `touch`, `pwd`).

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

fn print_ls_entry(path: &Path, name: &str, long: bool) {
    if !long {
        println!("{name}");
        return;
    }

    match fs::symlink_metadata(path) {
        Ok(meta) => {
            let kind = if meta.is_dir() {
                'd'
            } else if meta.file_type().is_symlink() {
                'l'
            } else {
                '-'
            };

            if meta.file_type().is_symlink()
                && let Ok(target) = fs::read_link(path)
            {
                println!("{kind}  {:>8}  {name} -> {}", meta.len(), target.display());
                return;
            }

            println!("{kind}  {:>8}  {name}", meta.len());
        }
        Err(err) => {
            eprintln!("ls: '{}': {err}", path.display());
        }
    }
}

fn list_path(target: &str, show_all: bool, long: bool) -> i32 {
    let path = Path::new(target);
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(err) => {
            eprintln!("ls: '{target}': {err}");
            return 1;
        }
    };

    if !meta.is_dir() {
        print_ls_entry(path, target, long);
        return 0;
    }

    let entries = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(err) => {
            eprintln!("ls: '{target}': {err}");
            return 1;
        }
    };

    let mut items: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !show_all && name.starts_with('.') {
            continue;
        }
        items.push((name, entry.path()));
    }

    items.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, item_path) in items {
        print_ls_entry(&item_path, &name, long);
    }
    0
}

pub fn cmd_ls(args: &[String]) -> i32 {
    let mut show_all = false;
    let mut long = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 {
            for ch in arg[1..].chars() {
                match ch {
                    'a' => show_all = true,
                    'l' => long = true,
                    other => {
                        eprintln!("ls: geçersiz seçenek '-{other}'");
                        return 2;
                    }
                }
            }
        } else {
            paths.push(arg);
        }
    }

    if paths.is_empty() {
        paths.push(".");
    }

    let mut code = 0;
    let multi = paths.len() > 1;
    for (idx, p) in paths.iter().enumerate() {
        if multi {
            if idx > 0 {
                println!();
            }
            println!("{p}:");
        }
        if list_path(p, show_all, long) != 0 {
            code = 1;
        }
    }
    code
}

pub fn cmd_cat(args: &[String]) -> i32 {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.is_empty() {
        let stdin = io::stdin();
        let mut inp = stdin.lock();
        if let Err(err) = io::copy(&mut inp, &mut out)
            && err.kind() != ErrorKind::BrokenPipe
        {
            eprintln!("cat: {err}");
            return 1;
        }
        return 0;
    }

    let mut code = 0;
    for path in args {
        if path == "-" {
            let stdin = io::stdin();
            let mut inp = stdin.lock();
            if let Err(err) = io::copy(&mut inp, &mut out)
                && err.kind() != ErrorKind::BrokenPipe
            {
                eprintln!("cat: {err}");
                code = 1;
            }
            continue;
        }

        match File::open(path) {
            Ok(mut f) => {
                if let Err(err) = io::copy(&mut f, &mut out) {
                    if err.kind() == ErrorKind::BrokenPipe {
                        return 0;
                    }
                    eprintln!("cat: '{path}': {err}");
                    code = 1;
                }
            }
            Err(err) => {
                eprintln!("cat: '{path}': {err}");
                code = 1;
            }
        }
    }
    code
}

pub fn cmd_mkdir(args: &[String]) -> i32 {
    let mut parents = false;
    let mut targets: Vec<&str> = Vec::new();

    for arg in args {
        if arg == "-p" {
            parents = true;
        } else if arg.starts_with('-') {
            eprintln!("mkdir: geçersiz seçenek '{arg}'");
            return 2;
        } else {
            targets.push(arg);
        }
    }

    if targets.is_empty() {
        eprintln!("mkdir: eksik dizin adı (kullanım: mkdir [-p] <dizin>...)");
        return 1;
    }

    let mut code = 0;
    for dir in targets {
        let res = if parents {
            fs::create_dir_all(dir)
        } else {
            fs::create_dir(dir)
        };
        if let Err(err) = res {
            eprintln!("mkdir: '{dir}': {err}");
            code = 1;
        }
    }
    code
}

pub fn cmd_rm(args: &[String]) -> i32 {
    let mut recursive = false;
    let mut force = false;
    let mut targets: Vec<&str> = Vec::new();

    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 {
            for ch in arg[1..].chars() {
                match ch {
                    'r' | 'R' => recursive = true,
                    'f' => force = true,
                    other => {
                        eprintln!("rm: geçersiz seçenek '-{other}'");
                        return 2;
                    }
                }
            }
        } else {
            targets.push(arg);
        }
    }

    if targets.is_empty() {
        if force {
            return 0;
        }
        eprintln!("rm: eksik dosya adı (kullanım: rm [-rf] <hedef>...)");
        return 1;
    }

    let mut code = 0;
    for target in targets {
        let res = match fs::symlink_metadata(target) {
            Ok(meta) if meta.is_dir() => {
                if recursive {
                    fs::remove_dir_all(target)
                } else {
                    eprintln!("rm: '{target}': bir dizin (-r kullanın)");
                    code = 1;
                    continue;
                }
            }
            Ok(_) => fs::remove_file(target),
            Err(err) => Err(err),
        };

        if let Err(err) = res {
            if force && err.kind() == ErrorKind::NotFound {
                continue;
            }
            eprintln!("rm: '{target}': {err}");
            code = 1;
        }
    }
    code
}

pub fn cmd_touch(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("touch: eksik dosya adı");
        return 1;
    }

    let mut code = 0;
    for path in args {
        if let Err(err) = OpenOptions::new().create(true).append(true).open(path) {
            eprintln!("touch: '{path}': {err}");
            code = 1;
        }
    }
    code
}

pub fn cmd_pwd() -> i32 {
    match env::current_dir() {
        Ok(p) => {
            println!("{}", p.display());
            0
        }
        Err(err) => {
            eprintln!("pwd: {err}");
            1
        }
    }
}
