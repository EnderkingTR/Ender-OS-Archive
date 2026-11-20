//! Metin ve konsol araçları (`echo`, `head`, `grep`, `clear`).

use std::fs::File;
use std::io::{self, BufRead, BufReader, ErrorKind, Write};

fn write_line_safe(out: &mut impl Write, text: &str) -> Result<(), bool> {
    match writeln!(out, "{text}") {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::BrokenPipe => Err(false),
        Err(err) => {
            eprintln!("yazma hatası: {err}");
            Err(true)
        }
    }
}

pub fn cmd_echo(args: &[String]) -> i32 {
    let (no_newline, rest) = match args.first().map(String::as_str) {
        Some("-n") => (true, &args[1..]),
        _ => (false, args),
    };

    let joined = rest.join(" ");
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let res = if no_newline {
        write!(out, "{joined}")
    } else {
        writeln!(out, "{joined}")
    };

    if let Err(err) = res
        && err.kind() != ErrorKind::BrokenPipe
    {
        eprintln!("echo: {err}");
        return 1;
    }
    let _ = out.flush();
    0
}

fn head_reader<R: BufRead>(reader: R, limit: usize, out: &mut impl Write) -> i32 {
    for (idx, line_res) in reader.lines().enumerate() {
        if idx >= limit {
            break;
        }
        match line_res {
            Ok(line) => {
                if let Err(is_err) = write_line_safe(out, &line) {
                    return if is_err { 1 } else { 0 };
                }
            }
            Err(err) => {
                eprintln!("head: okuma hatası: {err}");
                return 1;
            }
        }
    }
    0
}

pub fn cmd_head(args: &[String]) -> i32 {
    let mut limit: usize = 10;
    let mut files: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg == "-n" {
            i += 1;
            let Some(val) = args.get(i) else {
                eprintln!("head: '-n' için satır sayısı eksik");
                return 2;
            };
            match val.parse::<usize>() {
                Ok(n) => limit = n,
                Err(_) => {
                    eprintln!("head: geçersiz satır sayısı '{val}'");
                    return 2;
                }
            }
        } else if let Some(rest) = arg.strip_prefix("-n") {
            match rest.parse::<usize>() {
                Ok(n) => limit = n,
                Err(_) => {
                    eprintln!("head: geçersiz satır sayısı '{rest}'");
                    return 2;
                }
            }
        } else if arg.starts_with('-') {
            eprintln!("head: geçersiz seçenek '{arg}'");
            return 2;
        } else {
            files.push(arg);
        }
        i += 1;
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    if files.is_empty() {
        let stdin = io::stdin();
        return head_reader(stdin.lock(), limit, &mut out);
    }

    let mut code = 0;
    let multi = files.len() > 1;
    for (idx, path) in files.iter().enumerate() {
        if multi {
            if idx > 0 && write_line_safe(&mut out, "").is_err() {
                break;
            }
            if write_line_safe(&mut out, &format!("==> {path} <==")).is_err() {
                break;
            }
        }
        match File::open(path) {
            Ok(f) => {
                if head_reader(BufReader::new(f), limit, &mut out) != 0 {
                    code = 1;
                }
            }
            Err(err) => {
                eprintln!("head: '{path}': {err}");
                code = 1;
            }
        }
    }
    code
}

fn grep_reader<R: BufRead>(
    reader: R,
    pattern: &str,
    ignore_case: bool,
    prefix: Option<&str>,
    out: &mut impl Write,
) -> Result<bool, i32> {
    let needle_lower = if ignore_case {
        pattern.to_lowercase()
    } else {
        String::new()
    };

    let mut matched = false;
    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(err) => {
                eprintln!("grep: okuma hatası: {err}");
                return Err(2);
            }
        };

        let is_match = if ignore_case {
            line.to_lowercase().contains(&needle_lower)
        } else {
            line.contains(pattern)
        };

        if is_match {
            matched = true;
            let formatted = match prefix {
                Some(name) => format!("{name}:{line}"),
                None => line,
            };
            if let Err(is_err) = write_line_safe(out, &formatted) {
                return Err(if is_err { 2 } else { 0 });
            }
        }
    }
    Ok(matched)
}

pub fn cmd_grep(args: &[String]) -> i32 {
    let mut ignore_case = false;
    let mut positional: Vec<&str> = Vec::new();

    for arg in args {
        if arg == "-i" {
            ignore_case = true;
        } else if arg.starts_with('-') {
            eprintln!("grep: geçersiz seçenek '{arg}'");
            return 2;
        } else {
            positional.push(arg);
        }
    }

    let Some(&pattern) = positional.first() else {
        eprintln!("grep: kullanım: grep [-i] <desen> [dosya...]");
        return 2;
    };

    let files = &positional[1..];
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if files.is_empty() {
        let stdin = io::stdin();
        return match grep_reader(stdin.lock(), pattern, ignore_case, None, &mut out) {
            Ok(true) => 0,
            Ok(false) => 1,
            Err(code) => code,
        };
    }

    let mut any_match = false;
    let mut had_error = false;
    let multi = files.len() > 1;

    for path in files {
        let prefix = if multi { Some(*path) } else { None };
        match File::open(path) {
            Ok(f) => match grep_reader(BufReader::new(f), pattern, ignore_case, prefix, &mut out) {
                Ok(true) => any_match = true,
                Ok(false) => {}
                Err(0) => return 0,
                Err(_) => had_error = true,
            },
            Err(err) => {
                eprintln!("grep: '{path}': {err}");
                had_error = true;
            }
        }
    }

    if had_error {
        2
    } else if any_match {
        0
    } else {
        1
    }
}

pub fn cmd_clear() -> i32 {
    print!("\x1b[2J\x1b[H");
    let _ = io::stdout().flush();
    0
}
