//! Boru hattı (`|`) ve dosya yönlendirme (`>`, `>>`) çalıştırıcısı.

use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::process::{Child, ChildStdout, Command, Stdio};

use crate::parser::{Pipeline, Redirect, RedirectMode};

fn open_redirect_file(redir: &Redirect) -> std::io::Result<File> {
    match redir.mode {
        RedirectMode::Truncate => File::create(&redir.path),
        RedirectMode::Append => OpenOptions::new()
            .create(true)
            .append(true)
            .open(&redir.path),
    }
}

/// Ayrıştırılmış bir `Pipeline` içindeki tüm süreçleri başlatır ve son çıkış kodunu döndürür.
pub fn execute_pipeline(pipeline: &Pipeline) -> i32 {
    let mut prev_stdout: Option<ChildStdout> = None;
    let mut children: Vec<Child> = Vec::with_capacity(pipeline.stages.len());
    let total = pipeline.stages.len();
    let mut spawn_error_code: Option<i32> = None;

    for (idx, stage) in pipeline.stages.iter().enumerate() {
        let is_last = idx + 1 == total;
        let mut cmd = Command::new(&stage.program);
        cmd.args(&stage.args);

        if let Some(out) = prev_stdout.take() {
            cmd.stdin(Stdio::from(out));
        }

        if let Some(ref redir) = stage.stdout_redirect {
            match open_redirect_file(redir) {
                Ok(file) => {
                    cmd.stdout(Stdio::from(file));
                }
                Err(err) => {
                    eprintln!("ender-sh: '{}': {err}", redir.path);
                    spawn_error_code = Some(1);
                    break;
                }
            }
        } else if !is_last {
            cmd.stdout(Stdio::piped());
        }

        match cmd.spawn() {
            Ok(mut child) => {
                if !is_last && stage.stdout_redirect.is_none() {
                    prev_stdout = child.stdout.take();
                }
                children.push(child);
            }
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    eprintln!("ender-sh: '{}': komut bulunamadı", stage.program);
                    spawn_error_code = Some(127);
                } else {
                    eprintln!("ender-sh: '{}': {err}", stage.program);
                    spawn_error_code = Some(126);
                }
                break;
            }
        }
    }

    let mut last_code = 0;
    for mut child in children {
        match child.wait() {
            Ok(status) => last_code = status.code().unwrap_or(1),
            Err(err) => {
                eprintln!("ender-sh: süreç bekleme hatası: {err}");
                last_code = 1;
            }
        }
    }

    spawn_error_code.unwrap_or(last_code)
}
