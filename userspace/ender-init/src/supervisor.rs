//! Mini-Systemd Servis Denetleyicisi.
//!
//! Arka plan servislerini (`ender-httpd`) ve etkileşimli kabuğu (`ender-sh`)
//! başlatır, canlı durumlarını `/run/services/*.status` altında yayınlar, yetim
//! zombi süreçleri `waitpid(-1)` ile toplar ve çöken servisleri yeniden başlatır.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

struct ManagedService {
    name: &'static str,
    binary: &'static str,
    interactive: bool,
    log_path: Option<&'static str>,
    child: Option<Child>,
    pid: Option<libc::pid_t>,
    restarts: u32,
}

impl ManagedService {
    fn new_daemon(name: &'static str, binary: &'static str, log_path: &'static str) -> Self {
        Self {
            name,
            binary,
            interactive: false,
            log_path: Some(log_path),
            child: None,
            pid: None,
            restarts: 0,
        }
    }

    fn new_interactive(name: &'static str, binary: &'static str) -> Self {
        Self {
            name,
            binary,
            interactive: true,
            log_path: None,
            child: None,
            pid: None,
            restarts: 0,
        }
    }

    fn write_status(&self, state: &str) {
        let _ = fs::create_dir_all("/run/services");
        let path = format!("/run/services/{}.status", self.name);
        let pid_val = self.pid.unwrap_or(0);
        let content = format!(
            "name={}\npid={pid_val}\nstate={state}\nrestarts={}\nbinary={}\n",
            self.name, self.restarts, self.binary
        );
        let _ = fs::write(path, content);
    }

    fn spawn(&mut self) {
        let mut cmd = Command::new(self.binary);

        if !self.interactive {
            cmd.stdin(Stdio::null());
            if let Some(log_path) = self.log_path {
                if let Ok(log_file) = OpenOptions::new().create(true).append(true).open(log_path) {
                    if let Ok(err_file) = log_file.try_clone() {
                        cmd.stdout(Stdio::from(log_file));
                        cmd.stderr(Stdio::from(err_file));
                    } else {
                        cmd.stdout(Stdio::null());
                        cmd.stderr(Stdio::null());
                    }
                } else {
                    cmd.stdout(Stdio::null());
                    cmd.stderr(Stdio::null());
                }
            }
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id() as libc::pid_t;
                self.pid = Some(pid);
                self.child = Some(child);
                self.write_status("running");
            }
            Err(err) => {
                self.pid = None;
                self.child = None;
                self.write_status("failed");
                log_init(&format!(
                    "[ender-init] HATA: '{}' ({}) başlatılamadı: {err}",
                    self.name, self.binary
                ));
            }
        }
    }
}

fn log_init(msg: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/var/log/init.log")
    {
        let _ = writeln!(f, "{msg}");
    }
}

fn format_exit_status(status: libc::c_int) -> String {
    if libc::WIFEXITED(status) {
        format!("çıkış {}", libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        format!("sinyal {}", libc::WTERMSIG(status))
    } else {
        format!("durum {status}")
    }
}

pub fn run_supervisor() -> ! {
    let mut services = [
        ManagedService::new_daemon("ender-httpd", "/bin/ender-httpd", "/var/log/ender-httpd.log"),
        ManagedService::new_interactive("ender-sh", "/bin/ender-sh"),
    ];

    for svc in &mut services {
        svc.spawn();
    }

    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: `waitpid(-1, ...)` PID 1 altında tüm yönetilen servislerin
        // çıkışlarını ve sistemdeki yetim zombi süreçleri bekler/toplar.
        let reaped = unsafe { libc::waitpid(-1, &mut status, 0) };

        if reaped == -1 {
            let err = io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::EINTR) => continue,
                Some(libc::ECHILD) => {
                    thread::sleep(Duration::from_secs(2));
                    for svc in &mut services {
                        if svc.pid.is_none() {
                            svc.spawn();
                        }
                    }
                    continue;
                }
                _ => {
                    log_init(&format!("[ender-init] waitpid hatası: {err}"));
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            }
        }

        if let Some(svc) = services.iter_mut().find(|s| s.pid == Some(reaped)) {
            drop(svc.child.take());
            svc.pid = None;
            svc.restarts = svc.restarts.saturating_add(1);
            svc.write_status("restarting");

            let reason = format_exit_status(status);
            if svc.interactive {
                println!(
                    "[ender-init] {} sonlandı ({reason}), 1 saniye içinde yeniden başlatılıyor...",
                    svc.name
                );
            } else {
                log_init(&format!(
                    "[ender-init] servis '{}' sonlandı ({reason}), yeniden başlatılıyor...",
                    svc.name
                ));
            }

            thread::sleep(Duration::from_secs(1));
            svc.spawn();
        }
    }
}
