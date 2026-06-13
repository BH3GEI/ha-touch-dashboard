use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
struct Device {
    id: String,
    name: String,
    room: String,
    kind: String,
    on: bool,
    brightness: u8,
    color: String,
    speed: u8,
    note: String,
}

type SharedDevices = Arc<Mutex<Vec<Device>>>;

fn main() -> std::io::Result<()> {
    let addr = arg_value("--addr").unwrap_or_else(|| "0.0.0.0:8787".to_string());
    let state_path = PathBuf::from(
        arg_value("--state").unwrap_or_else(|| "/Users/mac/HomeAssistantBridge/state.tsv".to_string()),
    );
    let devices = Arc::new(Mutex::new(load_devices(&state_path)));
    let listener = TcpListener::bind(&addr)?;

    println!("Virtual Mijia bridge listening on http://{addr}");
    println!("State file: {}", state_path.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let devices = Arc::clone(&devices);
                let state_path = state_path.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_client(stream, devices, state_path) {
                        eprintln!("request failed: {err}");
                    }
                });
            }
            Err(err) => eprintln!("connection failed: {err}"),
        }
    }

    Ok(())
}

fn arg_value(name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

fn handle_client(
    mut stream: TcpStream,
    devices: SharedDevices,
    state_path: PathBuf,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = read_request(&mut stream)?;
    let (method, path, body) = match request {
        Some(request) => request,
        None => return Ok(()),
    };

    let (status, content_type, response_body) = route_request(&method, &path, &body, devices, state_path);
    write_response(&mut stream, status, content_type, &response_body)
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<(String, String, String)>> {
    let mut buffer = Vec::with_capacity(8192);
    let mut chunk = [0u8; 1024];
    let mut header_end = None;

    while header_end.is_none() && buffer.len() < 64 * 1024 {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = find_subsequence(&buffer, b"\r\n\r\n");
    }

    let Some(header_end) = header_end else {
        return Ok(None);
    };

    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = headers.lines();
    let Some(request_line) = lines.next() else {
        return Ok(None);
    };
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find_map(|(name, value)| {
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

    let body_start = header_end + 4;
    while buffer.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    let available = buffer.len().saturating_sub(body_start);
    let body_len = available.min(content_length);
    let body = String::from_utf8_lossy(&buffer[body_start..body_start + body_len]).to_string();

    Ok(Some((method, path, body)))
}

fn route_request(
    method: &str,
    path: &str,
    body: &str,
    devices: SharedDevices,
    state_path: PathBuf,
) -> (&'static str, &'static str, String) {
    if method == "OPTIONS" {
        return ("204 No Content", "text/plain; charset=utf-8", String::new());
    }

    if method == "GET" && path == "/" {
        return ("200 OK", "text/html; charset=utf-8", html());
    }

    if method == "GET" && path == "/favicon.ico" {
        return ("204 No Content", "image/x-icon", String::new());
    }

    if method == "GET" && path == "/api/health" {
        let count = devices.lock().map(|devices| devices.len()).unwrap_or(0);
        return (
            "200 OK",
            "application/json; charset=utf-8",
            format!(r#"{{"ok":true,"devices":{count}}}"#),
        );
    }

    if method == "GET" && path == "/api/devices" {
        let json = devices
            .lock()
            .map(|devices| devices_json(&devices))
            .unwrap_or_else(|_| r#"{"devices":[]}"#.to_string());
        return ("200 OK", "application/json; charset=utf-8", json);
    }

    if method == "POST" {
        if let Some(id) = path.strip_prefix("/api/devices/") {
            let params = parse_form(body);
            let mut locked = match devices.lock() {
                Ok(devices) => devices,
                Err(_) => {
                    return (
                        "500 Internal Server Error",
                        "application/json; charset=utf-8",
                        r#"{"ok":false,"error":"state lock poisoned"}"#.to_string(),
                    );
                }
            };

            let Some(device) = locked.iter_mut().find(|device| device.id == id) else {
                return (
                    "404 Not Found",
                    "application/json; charset=utf-8",
                    r#"{"ok":false,"error":"unknown device"}"#.to_string(),
                );
            };

            apply_update(device, &params);
            let updated_device = device.clone();
            let json = device_json(&updated_device);
            if let Err(err) = save_devices(&state_path, &locked) {
                eprintln!("failed to save state: {err}");
            }
            drop(locked);
            notify_home_assistant(&updated_device);
            return (
                "200 OK",
                "application/json; charset=utf-8",
                format!(r#"{{"ok":true,"device":{json}}}"#),
            );
        }
    }

    (
        "404 Not Found",
        "application/json; charset=utf-8",
        r#"{"ok":false,"error":"not found"}"#.to_string(),
    )
}

fn apply_update(device: &mut Device, params: &HashMap<String, String>) {
    if let Some(value) = params.get("on").filter(|value| !value.is_empty()) {
        device.on = matches!(value.as_str(), "1" | "true" | "on" | "yes");
    }
    if let Some(value) = params.get("brightness").and_then(|value| value.parse::<u8>().ok()) {
        device.brightness = value.min(100);
    }
    if let Some(value) = params.get("speed").and_then(|value| value.parse::<u8>().ok()) {
        device.speed = value.min(100);
    }
    if let Some(value) = params.get("color").filter(|value| is_hex_color(value)) {
        device.color = value.to_string();
    }
    if let Some(value) = params.get("note").filter(|value| !value.is_empty()) {
        device.note = value.chars().take(80).collect();
    }
}

fn notify_home_assistant(device: &Device) {
    if let Err(err) = post_home_assistant_webhook(device) {
        eprintln!("failed to sync Home Assistant webhook: {err}");
    }
}

fn post_home_assistant_webhook(device: &Device) -> std::io::Result<()> {
    let addr: SocketAddr = "127.0.0.1:8123".parse().expect("static Home Assistant address");
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;

    let body = format!(
        r#"{{"id":"{}","on":{},"brightness":{},"color":"{}","speed":{},"note":"{}"}}"#,
        escape_json(&device.id),
        device.on,
        device.brightness,
        escape_json(&device.color),
        device.speed,
        escape_json(&device.note)
    );
    let request = format!(
        "POST /api/webhook/virtual_mijia_bridge_state_b53b516a99ba5cf173601fd8ff7298e0 HTTP/1.1\r\n\
         Host: 127.0.0.1:8123\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    );
    stream.write_all(request.as_bytes())?;

    let mut response = [0u8; 128];
    let _ = stream.read(&mut response);
    Ok(())
}

fn parse_form(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hi = hex_value(bytes[index + 1]);
                let lo = hex_value(bytes[index + 2]);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push((hi << 4) | lo);
                    index += 3;
                } else {
                    out.push(bytes[index]);
                    index += 1;
                }
            }
            value => {
                out.push(value);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(|byte| byte.is_ascii_hexdigit())
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Connection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    );
    stream.write_all(response.as_bytes())
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn load_devices(path: &PathBuf) -> Vec<Device> {
    let mut devices = default_devices();
    let Ok(contents) = fs::read_to_string(path) else {
        return devices;
    };

    for line in contents.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 8 {
            continue;
        }
        if let Some(device) = devices.iter_mut().find(|device| device.id == fields[0]) {
            device.on = fields[4] == "1";
            device.brightness = fields[5].parse::<u8>().unwrap_or(device.brightness).min(100);
            device.color = if is_hex_color(fields[6]) {
                fields[6].to_string()
            } else {
                device.color.clone()
            };
            device.speed = fields[7].parse::<u8>().unwrap_or(device.speed).min(100);
            if let Some(note) = fields.get(8) {
                device.note = (*note).to_string();
            }
        }
    }

    devices
}

fn save_devices(path: &PathBuf, devices: &[Device]) -> std::io::Result<()> {
    let mut output = String::new();
    for device in devices {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            device.id,
            device.name,
            device.room,
            device.kind,
            if device.on { "1" } else { "0" },
            device.brightness,
            device.color,
            device.speed,
            device.note.replace('\t', " ")
        ));
    }
    fs::write(path, output)
}

fn default_devices() -> Vec<Device> {
    vec![
        Device {
            id: "desk_lamp".to_string(),
            name: "虚拟米家台灯".to_string(),
            room: "书桌".to_string(),
            kind: "light".to_string(),
            on: false,
            brightness: 65,
            color: "#ffd36b".to_string(),
            speed: 0,
            note: "亮度映射到台灯色块".to_string(),
        },
        Device {
            id: "lightstrip".to_string(),
            name: "虚拟米家灯带".to_string(),
            room: "电视墙".to_string(),
            kind: "light".to_string(),
            on: false,
            brightness: 45,
            color: "#4cc9f0".to_string(),
            speed: 0,
            note: "颜色和亮度映射到灯带色块".to_string(),
        },
        Device {
            id: "air_purifier".to_string(),
            name: "虚拟米家空气净化器".to_string(),
            room: "客厅".to_string(),
            kind: "fan".to_string(),
            on: false,
            brightness: 80,
            color: "#7bd389".to_string(),
            speed: 35,
            note: "风速越高，色块脉冲越快".to_string(),
        },
        Device {
            id: "xiaoai_scene".to_string(),
            name: "虚拟小爱音箱场景".to_string(),
            room: "语音".to_string(),
            kind: "switch".to_string(),
            on: false,
            brightness: 75,
            color: "#b692ff".to_string(),
            speed: 0,
            note: "模拟“小爱执行场景”开关".to_string(),
        },
    ]
}

fn devices_json(devices: &[Device]) -> String {
    let devices = devices.iter().map(device_json).collect::<Vec<_>>().join(",");
    format!(r#"{{"devices":[{devices}]}}"#)
}

fn device_json(device: &Device) -> String {
    format!(
        r#"{{"id":"{}","name":"{}","room":"{}","kind":"{}","on":{},"brightness":{},"color":"{}","speed":{},"note":"{}"}}"#,
        escape_json(&device.id),
        escape_json(&device.name),
        escape_json(&device.room),
        escape_json(&device.kind),
        device.on,
        device.brightness,
        escape_json(&device.color),
        device.speed,
        escape_json(&device.note)
    )
}

fn escape_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn html() -> String {
    r###"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <title>虚拟米家中控</title>
  <style>
    :root {
      color-scheme: light;
      --bg: #f4f6f8;
      --surface: #ffffff;
      --surface-soft: #f9fafb;
      --text: #111827;
      --muted: #667085;
      --faint: #98a2b3;
      --line: #d9e0ea;
      --green: #17b26a;
      --amber: #f59e0b;
      --cyan: #0891b2;
      --violet: #7c3aed;
      --blue: #2563eb;
      --danger: #ef4444;
      --shadow: 0 10px 28px rgba(15, 23, 42, .08);
      --radius: 8px;
    }
    * { box-sizing: border-box; }
    html {
      min-height: 100%;
      -webkit-text-size-adjust: 100%;
      touch-action: manipulation;
    }
    body {
      margin: 0;
      min-height: 100vh;
      background: var(--bg);
      color: var(--text);
      font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      letter-spacing: 0;
    }
    button, input {
      font: inherit;
      letter-spacing: 0;
    }
    button {
      -webkit-tap-highlight-color: transparent;
      touch-action: manipulation;
      cursor: pointer;
    }
    .shell {
      min-height: 100vh;
      padding-bottom: env(safe-area-inset-bottom);
    }
    .topbar {
      position: sticky;
      top: 0;
      z-index: 20;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 18px;
      min-height: 78px;
      padding: 12px max(22px, env(safe-area-inset-left)) 12px max(22px, env(safe-area-inset-right));
      border-bottom: 1px solid rgba(217, 224, 234, .86);
      background: rgba(255, 255, 255, .94);
      backdrop-filter: blur(18px);
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 14px;
      min-width: 0;
    }
    .brand-mark {
      display: grid;
      place-items: center;
      width: 48px;
      height: 48px;
      border: 1px solid #c7f0dc;
      border-radius: var(--radius);
      background: linear-gradient(135deg, #eafff4, #ffffff);
      color: var(--green);
      font-size: 26px;
      font-weight: 800;
      flex: 0 0 auto;
    }
    h1 {
      margin: 0;
      font-size: clamp(22px, 2.4vw, 32px);
      line-height: 1.08;
      font-weight: 760;
      letter-spacing: 0;
    }
    .subtitle {
      margin-top: 6px;
      color: var(--muted);
      font-size: 14px;
      line-height: 1.35;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .top-status {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 10px;
      flex-wrap: wrap;
    }
    .status-pill {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      min-height: 42px;
      padding: 0 14px;
      border: 1px solid var(--line);
      border-radius: var(--radius);
      background: var(--surface);
      color: #344054;
      font-size: 14px;
      white-space: nowrap;
    }
    .dot {
      width: 10px;
      height: 10px;
      border-radius: 999px;
      background: var(--green);
      box-shadow: 0 0 0 4px rgba(23, 178, 106, .14);
      flex: 0 0 auto;
    }
    main {
      width: min(1220px, calc(100% - 32px));
      margin: 0 auto;
      padding: 20px 0 28px;
    }
    .section-title {
      margin: 0 0 12px;
      font-size: 17px;
      line-height: 1.2;
      font-weight: 720;
      letter-spacing: 0;
    }
    .scene-strip {
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      gap: 12px;
      margin-bottom: 20px;
    }
    .scene-button {
      display: flex;
      align-items: center;
      gap: 14px;
      min-height: 92px;
      padding: 14px 16px;
      border: 2px solid var(--scene-border);
      border-radius: var(--radius);
      background: linear-gradient(135deg, var(--scene-bg), #fff);
      color: var(--text);
      box-shadow: 0 6px 18px rgba(15, 23, 42, .04);
      text-align: left;
    }
    .scene-button:active,
    .scene-button.is-running {
      transform: scale(.985);
      box-shadow: inset 0 0 0 999px rgba(255, 255, 255, .34);
    }
    .scene-icon {
      display: grid;
      place-items: center;
      width: 44px;
      height: 44px;
      border-radius: var(--radius);
      background: rgba(255,255,255,.72);
      color: var(--scene-color);
      font-size: 27px;
      flex: 0 0 auto;
    }
    .scene-copy strong {
      display: block;
      font-size: 18px;
      line-height: 1.1;
      font-weight: 750;
      letter-spacing: 0;
    }
    .scene-copy span {
      display: block;
      margin-top: 6px;
      color: var(--muted);
      font-size: 12px;
      line-height: 1.25;
    }
    .overview {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 12px;
      margin-bottom: 20px;
    }
    .stat-card {
      display: grid;
      grid-template-columns: 52px minmax(0, 1fr);
      align-items: center;
      gap: 12px;
      min-height: 82px;
      padding: 14px;
      border: 1px solid var(--line);
      border-radius: var(--radius);
      background: var(--surface);
      box-shadow: 0 6px 18px rgba(15, 23, 42, .035);
    }
    .stat-icon {
      display: grid;
      place-items: center;
      width: 52px;
      height: 52px;
      border-radius: var(--radius);
      color: #fff;
      font-size: 25px;
      background: var(--stat-color);
    }
    .stat-value {
      font-size: 25px;
      line-height: 1.05;
      font-weight: 780;
      letter-spacing: 0;
    }
    .stat-label {
      margin-top: 5px;
      color: var(--muted);
      font-size: 13px;
      line-height: 1.3;
    }
    .devices-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 14px;
    }
    .device-card {
      display: grid;
      grid-template-columns: 178px minmax(0, 1fr);
      min-height: 260px;
      border: 1px solid var(--line);
      border-radius: var(--radius);
      overflow: hidden;
      background: var(--surface);
      box-shadow: var(--shadow);
    }
    .device-visual {
      position: relative;
      display: grid;
      place-items: center;
      min-height: 100%;
      background:
        linear-gradient(135deg, rgba(255,255,255,.86), rgba(255,255,255,.12)),
        var(--device-color);
      opacity: var(--device-opacity);
      filter: saturate(var(--device-saturation)) brightness(var(--device-brightness));
      transition: opacity .18s ease, filter .18s ease, background-color .18s ease;
    }
    .device-visual::after {
      content: "";
      position: absolute;
      inset: 0;
      background:
        radial-gradient(circle at 32% 28%, rgba(255,255,255,.72), transparent 34%),
        repeating-linear-gradient(90deg, rgba(255,255,255,.12) 0 1px, transparent 1px 20px);
      pointer-events: none;
    }
    .device-symbol {
      position: relative;
      z-index: 1;
      display: grid;
      place-items: center;
      width: 94px;
      height: 94px;
      border: 2px solid rgba(255,255,255,.72);
      border-radius: var(--radius);
      background: rgba(255,255,255,.34);
      color: rgba(17, 24, 39, .72);
      font-size: 48px;
      font-weight: 700;
    }
    .device-card[data-kind="fan"][data-on="true"] .device-symbol {
      animation: spin var(--spin-speed) linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
    .device-body {
      display: grid;
      grid-template-rows: auto 1fr auto;
      gap: 14px;
      padding: 18px;
    }
    .device-head {
      display: grid;
      grid-template-columns: minmax(0, 1fr) 68px;
      gap: 14px;
      align-items: start;
    }
    .device-name-row {
      display: flex;
      align-items: center;
      gap: 10px;
      min-width: 0;
    }
    h2 {
      margin: 0;
      font-size: clamp(20px, 2vw, 25px);
      line-height: 1.12;
      font-weight: 760;
      letter-spacing: 0;
    }
    .online {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      color: var(--green);
      font-size: 13px;
      font-weight: 650;
      white-space: nowrap;
    }
    .online::before {
      content: "";
      width: 8px;
      height: 8px;
      border-radius: 999px;
      background: currentColor;
    }
    .room {
      margin-top: 8px;
      color: var(--muted);
      font-size: 14px;
      line-height: 1.3;
    }
    .state-word {
      margin-top: 10px;
      color: var(--state-color);
      font-size: 25px;
      line-height: 1.05;
      font-weight: 800;
    }
    .power-button {
      display: grid;
      place-items: center;
      width: 68px;
      height: 68px;
      border: 1px solid var(--line);
      border-radius: 999px;
      background: #fff;
      color: var(--muted);
      box-shadow: 0 8px 18px rgba(15, 23, 42, .08);
      font-size: 31px;
      font-weight: 780;
    }
    .power-button[data-on="true"] {
      border-color: rgba(23, 178, 106, .36);
      background: var(--green);
      color: #fff;
    }
    .power-button:active {
      transform: scale(.96);
    }
    .control-stack {
      display: grid;
      align-content: center;
      gap: 14px;
    }
    .control-row {
      display: grid;
      grid-template-columns: 58px minmax(0, 1fr) 54px;
      align-items: center;
      gap: 12px;
      min-height: 56px;
      padding: 8px 12px;
      border: 1px solid #e5eaf1;
      border-radius: var(--radius);
      background: var(--surface-soft);
    }
    .control-label {
      color: #475467;
      font-size: 14px;
      font-weight: 650;
    }
    .control-value {
      color: #344054;
      font-size: 15px;
      font-weight: 720;
      text-align: right;
      white-space: nowrap;
    }
    input[type="range"] {
      appearance: none;
      -webkit-appearance: none;
      width: 100%;
      height: 34px;
      margin: 0;
      background: transparent;
      touch-action: pan-y;
      accent-color: var(--device-color);
    }
    input[type="range"]::-webkit-slider-runnable-track {
      height: 10px;
      border-radius: 999px;
      background: linear-gradient(90deg, var(--device-color), rgba(148, 163, 184, .35));
    }
    input[type="range"]::-webkit-slider-thumb {
      appearance: none;
      -webkit-appearance: none;
      width: 34px;
      height: 34px;
      margin-top: -12px;
      border: 3px solid #fff;
      border-radius: 999px;
      background: var(--device-color);
      box-shadow: 0 4px 12px rgba(15, 23, 42, .20);
    }
    input[type="range"]::-moz-range-track {
      height: 10px;
      border-radius: 999px;
      background: linear-gradient(90deg, var(--device-color), rgba(148, 163, 184, .35));
    }
    input[type="range"]::-moz-range-thumb {
      width: 30px;
      height: 30px;
      border: 3px solid #fff;
      border-radius: 999px;
      background: var(--device-color);
      box-shadow: 0 4px 12px rgba(15, 23, 42, .20);
    }
    .color-row {
      display: grid;
      grid-template-columns: 58px minmax(0, 1fr) 64px;
      align-items: center;
      gap: 12px;
      min-height: 58px;
      padding: 8px 12px;
      border: 1px solid #e5eaf1;
      border-radius: var(--radius);
      background: var(--surface-soft);
    }
    input[type="color"] {
      width: 64px;
      height: 44px;
      border: 1px solid var(--line);
      border-radius: var(--radius);
      background: #fff;
      padding: 4px;
    }
    .note {
      display: flex;
      align-items: center;
      min-height: 34px;
      color: var(--muted);
      font-size: 13px;
      line-height: 1.35;
    }
    .touch-dock {
      position: sticky;
      bottom: 0;
      z-index: 18;
      display: none;
      grid-template-columns: repeat(3, 1fr);
      gap: 8px;
      padding: 10px 14px calc(10px + env(safe-area-inset-bottom));
      border-top: 1px solid rgba(217, 224, 234, .86);
      background: rgba(255,255,255,.95);
      backdrop-filter: blur(18px);
    }
    .dock-button {
      min-height: 48px;
      border: 1px solid var(--line);
      border-radius: var(--radius);
      background: #fff;
      color: #344054;
      font-size: 13px;
      font-weight: 720;
    }
    @media (max-width: 1060px) {
      .scene-strip { grid-template-columns: repeat(3, minmax(0, 1fr)); }
      .overview { grid-template-columns: repeat(2, minmax(0, 1fr)); }
      .devices-grid { grid-template-columns: 1fr; }
    }
    @media (max-width: 720px) {
      .topbar {
        position: static;
        align-items: flex-start;
        flex-direction: column;
        min-height: 0;
        padding: 16px;
      }
      .brand-mark { width: 44px; height: 44px; }
      .subtitle { white-space: normal; }
      .top-status { justify-content: flex-start; }
      main {
        width: calc(100% - 24px);
        padding-top: 14px;
        padding-bottom: 82px;
      }
      .scene-strip {
        display: grid;
        grid-auto-flow: column;
        grid-auto-columns: minmax(164px, 72%);
        grid-template-columns: none;
        overflow-x: auto;
        padding-bottom: 2px;
        scroll-snap-type: x mandatory;
      }
      .scene-button {
        min-height: 82px;
        scroll-snap-align: start;
      }
      .overview { grid-template-columns: 1fr 1fr; }
      .stat-card {
        grid-template-columns: 44px minmax(0, 1fr);
        min-height: 76px;
        padding: 12px;
      }
      .stat-icon { width: 44px; height: 44px; font-size: 22px; }
      .stat-value { font-size: 21px; }
      .device-card {
        grid-template-columns: 1fr;
        min-height: 0;
      }
      .device-visual { min-height: 132px; }
      .device-symbol { width: 76px; height: 76px; font-size: 40px; }
      .device-body { padding: 16px; }
      .device-head { grid-template-columns: minmax(0, 1fr) 64px; }
      .power-button { width: 64px; height: 64px; }
      .control-row, .color-row {
        grid-template-columns: 52px minmax(0, 1fr) 48px;
        padding-left: 10px;
        padding-right: 10px;
      }
      .touch-dock { display: grid; }
    }
    @media (prefers-reduced-motion: reduce) {
      *, *::before, *::after {
        animation-duration: .001ms !important;
        transition-duration: .001ms !important;
      }
    }
  </style>
</head>
<body>
  <div class="shell">
    <header class="topbar">
      <div class="brand">
        <div class="brand-mark" aria-hidden="true">米</div>
        <div>
          <h1>虚拟米家中控</h1>
          <div class="subtitle">触摸屏控制台 · Rust Bridge · Home Assistant 同步</div>
        </div>
      </div>
      <div class="top-status">
        <div class="status-pill"><span class="dot"></span><span id="status">连接中</span></div>
        <div class="status-pill">HomeKit 端口 51827</div>
      </div>
    </header>
    <main>
      <section aria-labelledby="scenes-title">
        <h2 class="section-title" id="scenes-title">快捷场景</h2>
        <div class="scene-strip" aria-label="快捷场景">
          <button class="scene-button" data-scene="all-on" style="--scene-color:var(--green);--scene-border:#86efac;--scene-bg:#ecfdf3">
            <span class="scene-icon" aria-hidden="true">⌂</span><span class="scene-copy"><strong>全开</strong><span>全部设备进入工作状态</span></span>
          </button>
          <button class="scene-button" data-scene="all-off" style="--scene-color:#64748b;--scene-border:#cbd5e1;--scene-bg:#f8fafc">
            <span class="scene-icon" aria-hidden="true">□</span><span class="scene-copy"><strong>全关</strong><span>离家或睡前快速收束</span></span>
          </button>
          <button class="scene-button" data-scene="movie" style="--scene-color:var(--cyan);--scene-border:#67e8f9;--scene-bg:#ecfeff">
            <span class="scene-icon" aria-hidden="true">▣</span><span class="scene-copy"><strong>影院</strong><span>灯带亮起，主灯压暗</span></span>
          </button>
          <button class="scene-button" data-scene="night" style="--scene-color:var(--amber);--scene-border:#fcd34d;--scene-bg:#fffbeb">
            <span class="scene-icon" aria-hidden="true">☾</span><span class="scene-copy"><strong>夜间</strong><span>低亮度，净化器轻档</span></span>
          </button>
          <button class="scene-button" data-scene="purify" style="--scene-color:var(--violet);--scene-border:#c4b5fd;--scene-bg:#f5f3ff">
            <span class="scene-icon" aria-hidden="true">✦</span><span class="scene-copy"><strong>净化强档</strong><span>空气净化器满速运行</span></span>
          </button>
        </div>
      </section>
      <section aria-labelledby="overview-title">
        <h2 class="section-title" id="overview-title">状态总览</h2>
        <div class="overview" id="overview"></div>
      </section>
      <section aria-labelledby="devices-title">
        <h2 class="section-title" id="devices-title">设备</h2>
        <div class="devices-grid" id="devices"></div>
      </section>
    </main>
    <nav class="touch-dock" aria-label="触摸快捷栏">
      <button class="dock-button" data-scene="all-on">全开</button>
      <button class="dock-button" data-scene="movie">影院</button>
      <button class="dock-button" data-scene="all-off">全关</button>
    </nav>
  </div>
  <script>
    const grid = document.getElementById('devices');
    const overview = document.getElementById('overview');
    const statusEl = document.getElementById('status');
    let lastJson = '';

    function clamp(value, min, max) {
      return Math.max(min, Math.min(max, value));
    }

    function deviceSymbol(device) {
      if (device.id === 'desk_lamp') return '▱';
      if (device.id === 'lightstrip') return '◌';
      if (device.id === 'air_purifier') return '✺';
      return '▦';
    }

    function deviceTypeLabel(device) {
      if (device.kind === 'light') return '灯光';
      if (device.kind === 'fan') return '净化';
      return '场景';
    }

    function summaryCards(devices) {
      const active = devices.filter(device => device.on).length;
      const litDevices = devices.filter(device => device.kind !== 'switch');
      const averageBrightness = litDevices.length
        ? Math.round(litDevices.reduce((sum, device) => sum + device.brightness, 0) / litDevices.length)
        : 0;
      const purifier = devices.find(device => device.id === 'air_purifier');
      const xiaoai = devices.find(device => device.id === 'xiaoai_scene');
      const cards = [
        ['⏻', active, '设备开启中', 'var(--green)'],
        ['☀', averageBrightness + '%', '平均亮度', 'var(--amber)'],
        ['✺', (purifier ? Math.ceil(purifier.speed / 25) : 0) + '档', '净化器风速', 'var(--cyan)'],
        ['⌁', xiaoai && xiaoai.on ? '执行中' : '待机', '小爱场景', 'var(--violet)']
      ];
      overview.innerHTML = cards.map(([icon, value, label, color]) => `
        <article class="stat-card">
          <div class="stat-icon" style="--stat-color:${color}">${icon}</div>
          <div><div class="stat-value">${value}</div><div class="stat-label">${label}</div></div>
        </article>`).join('');
    }

    function deviceCard(device) {
      const activeBrightness = device.on ? clamp(device.brightness, 8, 100) : 8;
      const opacity = device.on ? activeBrightness / 100 : 0.20;
      const brightness = device.on ? 0.74 + activeBrightness / 125 : 0.54;
      const saturation = device.on ? 1.15 : 0.38;
      const spinSeconds = Math.max(0.45, 3.2 - (device.speed / 100) * 2.6).toFixed(2) + 's';
      const stateText = device.on ? '开' : (device.kind === 'switch' ? '待机' : '关');
      const stateColor = device.on ? 'var(--green)' : '#64748b';
      const speedControl = device.kind === 'fan'
        ? `<div class="control-row"><div class="control-label">风速</div><input aria-label="${device.name}风速" type="range" min="0" max="100" value="${device.speed}" data-action="speed" data-id="${device.id}"><div class="control-value">${device.speed}%</div></div>`
        : '';
      const colorControl = device.kind !== 'fan'
        ? `<div class="color-row"><div class="control-label">颜色</div><div class="control-value">${device.color}</div><input aria-label="${device.name}颜色" type="color" value="${device.color}" data-action="color" data-id="${device.id}"></div>`
        : '';
      return `
        <article class="device-card" data-kind="${device.kind}" data-on="${device.on}"
          style="--device-color:${device.color};--device-opacity:${opacity};--device-brightness:${brightness};--device-saturation:${saturation};--spin-speed:${spinSeconds};--state-color:${stateColor}">
          <div class="device-visual"><div class="device-symbol">${deviceSymbol(device)}</div></div>
          <div class="device-body">
            <div class="device-head">
              <div>
                <div class="device-name-row"><h2>${device.name}</h2><span class="online">在线</span></div>
                <div class="room">${device.room} · ${deviceTypeLabel(device)}</div>
                <div class="state-word">${stateText}</div>
              </div>
              <button class="power-button" aria-label="${device.name}${device.on ? '关闭' : '开启'}" data-action="toggle" data-id="${device.id}" data-on="${device.on}">⏻</button>
            </div>
            <div class="control-stack">
              <div class="control-row"><div class="control-label">亮度</div><input aria-label="${device.name}亮度" type="range" min="0" max="100" value="${device.brightness}" data-action="brightness" data-id="${device.id}"><div class="control-value">${device.brightness}%</div></div>
              ${speedControl}
              ${colorControl}
            </div>
            <div class="note">${device.note}</div>
          </div>
        </article>`;
    }

    async function loadDevices() {
      const response = await fetch('/api/devices', { cache: 'no-store' });
      if (!response.ok) throw new Error('HTTP ' + response.status);
      const json = await response.text();
      if (json !== lastJson) {
        const data = JSON.parse(json);
        summaryCards(data.devices);
        grid.innerHTML = data.devices.map(deviceCard).join('');
        lastJson = json;
      }
      statusEl.textContent = '已连接 ' + new Date().toLocaleTimeString();
    }

    async function updateDevice(id, params) {
      const body = new URLSearchParams(params);
      const response = await fetch('/api/devices/' + encodeURIComponent(id), {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body
      });
      if (!response.ok) throw new Error('HTTP ' + response.status);
      lastJson = '';
      await loadDevices();
    }

    document.addEventListener('click', event => {
      const target = event.target.closest('[data-action="toggle"]');
      if (!target) return;
      updateDevice(target.dataset.id, { on: target.dataset.on !== 'true' }).catch(showError);
    });

    document.addEventListener('change', event => {
      const target = event.target;
      if (!target.dataset.action) return;
      if (target.dataset.action === 'brightness') {
        updateDevice(target.dataset.id, { brightness: target.value }).catch(showError);
      }
      if (target.dataset.action === 'speed') {
        updateDevice(target.dataset.id, { speed: target.value, on: Number(target.value) > 0 }).catch(showError);
      }
      if (target.dataset.action === 'color') {
        updateDevice(target.dataset.id, { color: target.value }).catch(showError);
      }
    });

    document.addEventListener('click', event => {
      const target = event.target.closest('[data-scene]');
      if (!target) return;
      applyScene(target.dataset.scene, target).catch(showError);
    });

    async function applyScene(scene, trigger) {
      const scenes = {
        'all-on': [
          ['desk_lamp', { on: true, brightness: 90, color: '#ffd36b', note: '网页快捷操作：全开' }],
          ['lightstrip', { on: true, brightness: 82, color: '#4cc9f0', note: '网页快捷操作：全开' }],
          ['air_purifier', { on: true, brightness: 55, speed: 55, note: '网页快捷操作：全开' }],
          ['xiaoai_scene', { on: true, brightness: 90, note: '网页快捷操作：全开' }]
        ],
        'all-off': [
          ['desk_lamp', { on: false, brightness: 25, note: '网页快捷操作：全关' }],
          ['lightstrip', { on: false, brightness: 25, note: '网页快捷操作：全关' }],
          ['air_purifier', { on: false, brightness: 20, speed: 0, note: '网页快捷操作：全关' }],
          ['xiaoai_scene', { on: false, brightness: 20, note: '网页快捷操作：全关' }]
        ],
        'movie': [
          ['desk_lamp', { on: false, brightness: 12, color: '#f97316', note: '网页快捷操作：影院模式' }],
          ['lightstrip', { on: true, brightness: 35, color: '#b692ff', note: '网页快捷操作：影院模式' }],
          ['air_purifier', { on: true, brightness: 25, speed: 25, note: '网页快捷操作：影院模式' }],
          ['xiaoai_scene', { on: true, brightness: 65, color: '#b692ff', note: '网页快捷操作：影院模式' }]
        ],
        'night': [
          ['desk_lamp', { on: true, brightness: 18, color: '#f97316', note: '网页快捷操作：夜间模式' }],
          ['lightstrip', { on: false, brightness: 10, color: '#4cc9f0', note: '网页快捷操作：夜间模式' }],
          ['air_purifier', { on: true, brightness: 20, speed: 18, note: '网页快捷操作：夜间模式' }],
          ['xiaoai_scene', { on: false, brightness: 20, note: '网页快捷操作：夜间模式' }]
        ],
        'purify': [
          ['air_purifier', { on: true, brightness: 100, speed: 100, note: '网页快捷操作：净化器强档' }]
        ]
      };
      if (trigger) trigger.classList.add('is-running');
      for (const [id, params] of scenes[scene] || []) {
        await updateDevice(id, params);
      }
      if (trigger) window.setTimeout(() => trigger.classList.remove('is-running'), 180);
      lastJson = '';
      await loadDevices();
    }

    function showError(error) {
      statusEl.textContent = '连接错误: ' + error.message;
    }

    loadDevices().catch(showError);
    setInterval(() => loadDevices().catch(showError), 1500);
  </script>
</body>
</html>"###
        .to_string()
}
