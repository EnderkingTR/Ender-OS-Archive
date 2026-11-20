//! Ender-OS v0.4 — Sıfır bağımlılıklı çok iş parçacıklı Rust HTTP/1.1 Web Sunucusu.
//!
//! `0.0.0.0:8080` portunda çalışarak canlı sistem kontrol paneli (`/`), JSON durum
//! API'si (`/api/status`) ve `/dev/ender` çekirdek modülü okuma/yazma uç noktası
//! (`/api/ender-dev`) sunar.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::thread;
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024;

struct HttpRequest {
    method: String,
    path: String,
    content_type: String,
    body: Vec<u8>,
}

struct ServiceRow {
    name: String,
    pid: String,
    state: String,
    restarts: String,
    binary: String,
}

fn find_header_end(buf: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some((pos, 4));
    }
    if let Some(pos) = buf.windows(2).position(|w| w == b"\n\n") {
        return Some((pos, 2));
    }
    None
}

fn parse_http_request(stream: &mut TcpStream) -> Option<HttpRequest> {
    let mut raw = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];

    let (hdr_end, sep_len) = loop {
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if let Some(found) = find_header_end(&raw) {
                    break found;
                }
                if raw.len() >= MAX_HEADER_BYTES {
                    return None;
                }
            }
            Err(_) => return None,
        }
    };

    let header_str = std::str::from_utf8(&raw[..hdr_end]).ok()?;
    let mut lines = header_str.lines();
    let req_line = lines.next()?.trim();
    let mut parts = req_line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    let path = target.split('?').next().unwrap_or(target).to_string();

    let mut content_length: usize = 0;
    let mut content_type = String::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim();
            if key == "content-length" {
                content_length = val.parse::<usize>().unwrap_or(0).min(MAX_BODY_BYTES);
            } else if key == "content-type" {
                content_type = val.to_ascii_lowercase();
            }
        }
    }

    let body_start = hdr_end + sep_len;
    let mut body = raw[body_start..].to_vec();
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let to_read = remaining.min(chunk.len());
        match stream.read(&mut chunk[..to_read]) {
            Ok(0) | Err(_) => break,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    body.truncate(content_length);

    Some(HttpRequest {
        method,
        path,
        content_type,
        body,
    })
}

fn send_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) {
    let mut header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in extra_headers {
        header.push_str(&format!("{k}: {v}\r\n"));
    }
    header.push_str("\r\n");

    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn url_decode(input: &[u8]) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'+' => {
                bytes.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < input.len() => {
                if let (Some(hi), Some(lo)) = (hex_val(input[i + 1]), hex_val(input[i + 2])) {
                    bytes.push((hi << 4) | lo);
                    i += 3;
                } else {
                    bytes.push(b'%');
                    i += 1;
                }
            }
            b => {
                bytes.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn extract_form_msg(body: &[u8]) -> Option<String> {
    let raw = std::str::from_utf8(body).ok()?;
    for pair in raw.split('&') {
        if let Some((k, v)) = pair.split_once('=')
            && k == "msg"
        {
            return Some(url_decode(v.as_bytes()));
        }
    }
    None
}

fn read_hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "(none)")
        .unwrap_or_else(|| "ender-os".to_string())
}

fn read_kernel_version() -> String {
    fs::read_to_string("/proc/version")
        .ok()
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
        .unwrap_or_else(|| "bilinmiyor".to_string())
}

fn read_uptime_secs() -> f64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn read_mem_kb() -> (u64, u64, u64) {
    let mut total = 0;
    let mut free = 0;
    let mut avail = 0;
    if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
        for line in contents.lines() {
            let val = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            if line.starts_with("MemTotal:") {
                total = val;
            } else if line.starts_with("MemFree:") {
                free = val;
            } else if line.starts_with("MemAvailable:") {
                avail = val;
            }
        }
    }
    (total, free, avail)
}

fn is_data_mounted() -> bool {
    fs::read_to_string("/proc/mounts")
        .map(|s| s.lines().any(|l| l.split_whitespace().nth(1) == Some("/data")))
        .unwrap_or(false)
}

fn list_modules() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    if let Ok(contents) = fs::read_to_string("/proc/modules") {
        for line in contents.lines() {
            let mut parts = line.split_whitespace();
            if let Some(name) = parts.next() {
                let size = parts.next().unwrap_or("0").to_string();
                let refs = parts.next().unwrap_or("0").to_string();
                out.push((name.to_string(), size, refs));
            }
        }
    }
    out
}

fn list_services() -> Vec<ServiceRow> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir("/run/services") {
        let mut paths: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("status"))
            .collect();
        paths.sort();

        for path in paths {
            if let Ok(raw) = fs::read_to_string(path) {
                let mut row = ServiceRow {
                    name: String::new(),
                    pid: String::new(),
                    state: String::new(),
                    restarts: String::from("0"),
                    binary: String::new(),
                };
                for line in raw.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        match k.trim() {
                            "name" => row.name = v.trim().to_string(),
                            "pid" => row.pid = v.trim().to_string(),
                            "state" => row.state = v.trim().to_string(),
                            "restarts" => row.restarts = v.trim().to_string(),
                            "binary" => row.binary = v.trim().to_string(),
                            _ => {}
                        }
                    }
                }
                if !row.name.is_empty() {
                    out.push(row);
                }
            }
        }
    }
    out
}

fn list_data_files() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir("/data") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name != "lost+found" {
                names.push(name);
            }
        }
    }
    names.sort();
    names
}

fn read_ender_dev() -> Option<String> {
    if !Path::new("/dev/ender").exists() {
        return None;
    }
    fs::read_to_string("/dev/ender").ok()
}

fn render_dashboard() -> String {
    let host = read_hostname();
    let kernel = read_kernel_version();
    let uptime = read_uptime_secs();
    let (mem_total, mem_free, mem_avail) = read_mem_kb();
    let mem_used = mem_total.saturating_sub(mem_avail);
    let mem_pct = if mem_total > 0 {
        (mem_used * 100) / mem_total
    } else {
        0
    };
    let data_ok = is_data_mounted();
    let services = list_services();
    let modules = list_modules();
    let data_files = list_data_files();
    let ender_msg = read_ender_dev();

    let mut svc_rows = String::new();
    for s in &services {
        svc_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td><span class=\"badge\">{}</span></td><td>{}</td><td><code>{}</code></td></tr>",
            html_escape(&s.name),
            html_escape(&s.pid),
            html_escape(&s.state),
            html_escape(&s.restarts),
            html_escape(&s.binary),
        ));
    }
    if svc_rows.is_empty() {
        svc_rows.push_str("<tr><td colspan=\"5\">Kayıtlı servis yok.</td></tr>");
    }

    let mut mod_rows = String::new();
    for (name, size, refs) in &modules {
        mod_rows.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{} bayt</td><td>{}</td></tr>",
            html_escape(name),
            html_escape(size),
            html_escape(refs),
        ));
    }
    if mod_rows.is_empty() {
        mod_rows.push_str("<tr><td colspan=\"3\">Yüklü modül yok.</td></tr>");
    }

    let data_list = if data_files.is_empty() {
        "<li>(boş)</li>".to_string()
    } else {
        data_files
            .iter()
            .map(|n| format!("<li><code>/data/{}</code></li>", html_escape(n)))
            .collect::<Vec<_>>()
            .join("")
    };

    let dev_panel = match ender_msg {
        Some(msg) => format!(
            "<pre class=\"devbox\">{}</pre>\
             <form method=\"POST\" action=\"/api/ender-dev\" class=\"form-row\">\
               <input type=\"text\" name=\"msg\" placeholder=\"/dev/ender modülüne mesaj yaz...\" required maxlength=\"480\" />\
               <button type=\"submit\">Çekirdeğe Yaz</button>\
             </form>",
            html_escape(&msg)
        ),
        None => "<p class=\"muted\"><code>/dev/ender</code> bulunamadı (<code>ender_dev.ko</code> yüklü değil).</p>".to_string(),
    };

    let disk_badge = if data_ok {
        "<span class=\"badge\">/data ext4 bağlı</span>"
    } else {
        "<span class=\"badge warn\">/data bağlı değil</span>"
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="tr">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Ender-OS v0.4 Kontrol Paneli</title>
<style>
  :root {{ --bg:#0b1020; --card:#131b2e; --border:#24314f; --fg:#e5ecff; --muted:#8fa3c7; --accent:#38bdf8; --ok:#22c55e; --warn:#f59e0b; }}
  * {{ box-sizing:border-box; }}
  body {{ margin:0; font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif; background:var(--bg); color:var(--fg); padding:24px; }}
  .wrap {{ max-width:980px; margin:0 auto; }}
  header {{ display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:12px; margin-bottom:20px; }}
  h1 {{ margin:0; font-size:1.6rem; color:var(--accent); }}
  .sub {{ color:var(--muted); font-size:0.9rem; margin-top:4px; }}
  .grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(290px,1fr)); gap:16px; margin-bottom:16px; }}
  .card {{ background:var(--card); border:1px solid var(--border); border-radius:12px; padding:16px; }}
  h2 {{ margin:0 0 12px 0; font-size:1.05rem; color:#cfe2ff; }}
  table {{ width:100%; border-collapse:collapse; font-size:0.9rem; }}
  th,td {{ text-align:left; padding:7px 8px; border-bottom:1px solid var(--border); }}
  th {{ color:var(--muted); font-weight:600; }}
  .badge {{ display:inline-block; padding:2px 8px; border-radius:999px; font-size:0.78rem; background:rgba(34,197,94,0.18); color:var(--ok); border:1px solid rgba(34,197,94,0.4); }}
  .badge.warn {{ background:rgba(245,158,11,0.18); color:var(--warn); border-color:rgba(245,158,11,0.4); }}
  .bar {{ height:10px; background:#090d18; border-radius:999px; overflow:hidden; margin:8px 0; border:1px solid var(--border); }}
  .bar > div {{ height:100%; background:linear-gradient(90deg,var(--accent),#6366f1); }}
  pre.devbox {{ background:#090d18; border:1px solid var(--border); padding:12px; border-radius:8px; overflow-x:auto; color:#a7f3d0; }}
  .form-row {{ display:flex; gap:8px; margin-top:10px; }}
  input[type=text] {{ flex:1; padding:9px 12px; border-radius:8px; border:1px solid var(--border); background:#090d18; color:var(--fg); }}
  button {{ padding:9px 16px; border-radius:8px; border:0; background:var(--accent); color:#04111f; font-weight:700; cursor:pointer; }}
  a {{ color:var(--accent); text-decoration:none; }}
  code {{ font-family:ui-monospace,Consolas,monospace; font-size:0.86rem; }}
  ul {{ margin:8px 0 0 18px; padding:0; }}
  .muted {{ color:var(--muted); }}
</style>
</head>
<body>
<div class="wrap">
  <header>
    <div>
      <h1>Ender-OS v0.4 — Sistem Kontrol Paneli</h1>
      <div class="sub">{host} &bull; {kernel}</div>
    </div>
    <div>{disk_badge} &nbsp; <a href="/api/status">JSON API (/api/status)</a></div>
  </header>

  <div class="grid">
    <div class="card">
      <h2>Sistem &amp; Bellek</h2>
      <div>Uptime: <strong>{uptime:.1} sn</strong></div>
      <div style="margin-top:8px;">RAM Kullanımı: <strong>{mem_used} / {mem_total} kB</strong> (%{mem_pct})</div>
      <div class="bar"><div style="width:{mem_pct}%"></div></div>
      <div class="muted">Kullanılabilir: {mem_avail} kB &bull; Boş: {mem_free} kB</div>
    </div>

    <div class="card">
      <h2>Kalıcı Disk (<code>/data</code> ext4)</h2>
      <ul>{data_list}</ul>
    </div>
  </div>

  <div class="card" style="margin-bottom:16px;">
    <h2>Rust Çekirdek Modülü (<code>/dev/ender</code>)</h2>
    {dev_panel}
  </div>

  <div class="grid">
    <div class="card">
      <h2>Servis Denetleyicisi (<code>ender-init</code>)</h2>
      <table>
        <thead><tr><th>Servis</th><th>PID</th><th>Durum</th><th>Restart</th><th>İkili</th></tr></thead>
        <tbody>{svc_rows}</tbody>
      </table>
    </div>

    <div class="card">
      <h2>Yüklü Çekirdek Modülleri</h2>
      <table>
        <thead><tr><th>Modül</th><th>Boyut</th><th>Ref</th></tr></thead>
        <tbody>{mod_rows}</tbody>
      </table>
    </div>
  </div>
</div>
</body>
</html>
"#,
        host = html_escape(&host),
        kernel = html_escape(&kernel),
        disk_badge = disk_badge,
        uptime = uptime,
        mem_used = mem_used,
        mem_total = mem_total,
        mem_pct = mem_pct,
        mem_avail = mem_avail,
        mem_free = mem_free,
        data_list = data_list,
        dev_panel = dev_panel,
        svc_rows = svc_rows,
        mod_rows = mod_rows,
    )
}

fn render_status_json() -> String {
    let host = read_hostname();
    let kernel = read_kernel_version();
    let uptime = read_uptime_secs();
    let (mem_total, mem_free, mem_avail) = read_mem_kb();
    let data_mounted = is_data_mounted();
    let modules = list_modules()
        .into_iter()
        .map(|(n, _, _)| format!("\"{}\"", json_escape(&n)))
        .collect::<Vec<_>>()
        .join(",");
    let services = list_services()
        .into_iter()
        .map(|s| {
            format!(
                "{{\"name\":\"{}\",\"pid\":{},\"state\":\"{}\",\"restarts\":{}}}",
                json_escape(&s.name),
                s.pid.parse::<u32>().unwrap_or(0),
                json_escape(&s.state),
                s.restarts.parse::<u32>().unwrap_or(0),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let ender_msg = read_ender_dev().unwrap_or_default();

    format!(
        "{{\"os\":\"Ender-OS v0.4\",\"hostname\":\"{}\",\"kernel\":\"{}\",\"uptime_secs\":{:.2},\"mem_total_kb\":{},\"mem_free_kb\":{},\"mem_available_kb\":{},\"data_mounted\":{},\"modules\":[{}],\"services\":[{}],\"ender_dev_msg\":\"{}\"}}\n",
        json_escape(&host),
        json_escape(&kernel),
        uptime,
        mem_total,
        mem_free,
        mem_avail,
        data_mounted,
        modules,
        services,
        json_escape(&ender_msg),
    )
}

fn handle_client(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));

    let Some(req) = parse_http_request(&mut stream) else {
        return;
    };

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => {
            let html = render_dashboard();
            send_response(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                &[],
                html.as_bytes(),
            );
        }
        ("GET", "/api/status") => {
            let json = render_status_json();
            send_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &[],
                json.as_bytes(),
            );
        }
        ("GET", "/api/ender-dev") => match read_ender_dev() {
            Some(msg) => send_response(
                &mut stream,
                "200 OK",
                "text/plain; charset=utf-8",
                &[],
                msg.as_bytes(),
            ),
            None => send_response(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                &[],
                b"/dev/ender yok\n",
            ),
        },
        ("POST", "/api/ender-dev") => {
            let form_msg = if req
                .content_type
                .contains("application/x-www-form-urlencoded")
            {
                extract_form_msg(&req.body)
            } else {
                None
            };
            let is_browser_form = form_msg.is_some();

            let payload = if let Some(mut decoded) = form_msg {
                if !decoded.ends_with('\n') {
                    decoded.push('\n');
                }
                decoded.into_bytes()
            } else if req.body.is_empty() {
                b"\n".to_vec()
            } else {
                req.body
            };

            match fs::write("/dev/ender", &payload) {
                Ok(()) => {
                    if is_browser_form {
                        send_response(
                            &mut stream,
                            "303 See Other",
                            "text/plain; charset=utf-8",
                            &[("Location", "/")],
                            b"Yonlendiriliyor...\n",
                        );
                    } else {
                        send_response(
                            &mut stream,
                            "200 OK",
                            "text/plain; charset=utf-8",
                            &[],
                            b"yazildi\n",
                        );
                    }
                }
                Err(err) => {
                    let msg = format!("HATA: /dev/ender yazilamadi: {err}\n");
                    send_response(
                        &mut stream,
                        "500 Internal Server Error",
                        "text/plain; charset=utf-8",
                        &[],
                        msg.as_bytes(),
                    );
                }
            }
        }
        ("GET", "/health") => {
            send_response(
                &mut stream,
                "200 OK",
                "text/plain; charset=utf-8",
                &[],
                b"ok\n",
            );
        }
        _ => {
            send_response(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                &[],
                b"404 Not Found\n",
            );
        }
    }
}

fn main() {
    let addr = env::var("ENDER_HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(err) => {
            eprintln!("ender-httpd: '{addr}' dinlenemedi: {err}");
            std::process::exit(1);
        }
    };

    println!("ender-httpd v0.4 dinleniyor: http://{addr}");
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                thread::spawn(move || handle_client(stream));
            }
            Err(err) => {
                eprintln!("ender-httpd: bağlantı hatası: {err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html_correctly() {
        assert_eq!(
            html_escape("<script>\"a&b\"</script>"),
            "&lt;script&gt;&quot;a&amp;b&quot;&lt;/script&gt;"
        );
    }

    #[test]
    fn escapes_json_correctly() {
        assert_eq!(
            json_escape("satir1\n\"teklif\"\\son"),
            "satir1\\n\\\"teklif\\\"\\\\son"
        );
    }

    #[test]
    fn decodes_form_url_encoded_msg() {
        let body = b"foo=bar&msg=Merhaba+Ender-OS%21%20%C3%BC&baz=1";
        assert_eq!(extract_form_msg(body).as_deref(), Some("Merhaba Ender-OS! ü"));
        assert_eq!(extract_form_msg(b"Ham API govdesi\n"), None);
    }

    #[test]
    fn finds_crlf_and_lf_header_boundaries() {
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\n\r\nbody"), Some((14, 4)));
        assert_eq!(find_header_end(b"GET / HTTP/1.1\n\nbody"), Some((14, 2)));
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\n"), None);
    }
}
