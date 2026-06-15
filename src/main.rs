use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

const DEFAULT_HA_ADDR: &str = "127.0.0.1:8123";
const DEFAULT_HA_TOKEN_FILE: &str = "/Users/mac/HomeAssistantCore/HA-OWNER-ACCESS-TOKEN.txt";

const TV_ENTITY: &str =
    "media_player.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1";
const XIAOAI_ENTITY: &str = "media_player.xiaomi_cn_2037162573_x4b";
const AC_ENTITY: &str = "climate.lumi_cn_974076238_mcn02";
const CURTAIN_ENTITY: &str = "cover.czmydz_cn_2143837107_mym1_s_2_curtain";
const TEMPERATURE_ENTITY: &str = "sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_temperature_p_2_1001";
const HUMIDITY_ENTITY: &str =
    "sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_relative_humidity_p_2_1002";
const BATTERY_ENTITY: &str = "sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_battery_level_p_3_1003";
const TV_VOLUME_UP_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_volume_up_a_7_12";
const TV_VOLUME_DOWN_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_volume_down_a_7_11";
const XIAOAI_WAKE_BUTTON: &str = "button.xiaomi_cn_2037162573_x4b_wake_up_a_5_1";
const XIAOAI_RADIO_BUTTON: &str = "button.xiaomi_cn_2037162573_x4b_play_radio_a_5_2";
const XIAOAI_MUSIC_BUTTON: &str = "button.xiaomi_cn_2037162573_x4b_play_music_a_5_5";
const XIAOAI_NOTIFY_ENTITY: &str = "notify.xiaomi_cn_2037162573_x4b_play_text_a_5_3";

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
    mode: String,
    note: String,
}

type SharedDevices = Arc<Mutex<Vec<Device>>>;

#[derive(Clone, Debug)]
struct HaClient {
    addr: SocketAddr,
    host: String,
    token: Option<String>,
}

fn main() -> std::io::Result<()> {
    let addr = arg_value("--addr").unwrap_or_else(|| "0.0.0.0:8787".to_string());
    let state_path = PathBuf::from(
        arg_value("--state")
            .unwrap_or_else(|| "/Users/mac/HomeAssistantBridge/state.tsv".to_string()),
    );
    let ha_client = HaClient::from_args();
    let devices = Arc::new(Mutex::new(load_devices(&state_path)));
    let listener = TcpListener::bind(&addr)?;

    println!("Mijia bridge listening on http://{addr}");
    println!("State file: {}", state_path.display());
    println!("Home Assistant API: {}", ha_client.label());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let devices = Arc::clone(&devices);
                let state_path = state_path.clone();
                let ha_client = ha_client.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_client(stream, devices, state_path, ha_client) {
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

impl HaClient {
    fn from_args() -> Self {
        let host = arg_value("--ha-addr")
            .or_else(|| env::var("HA_ADDR").ok())
            .unwrap_or_else(|| DEFAULT_HA_ADDR.to_string());
        let addr = host.parse::<SocketAddr>().unwrap_or_else(|_| {
            DEFAULT_HA_ADDR
                .parse()
                .expect("static Home Assistant address")
        });
        let token = arg_value("--ha-token")
            .or_else(|| env::var("HA_TOKEN").ok())
            .or_else(|| {
                let path = arg_value("--ha-token-file")
                    .or_else(|| env::var("HA_TOKEN_FILE").ok())
                    .unwrap_or_else(|| DEFAULT_HA_TOKEN_FILE.to_string());
                fs::read_to_string(path)
                    .ok()
                    .map(|token| token.trim().to_string())
            })
            .filter(|token| !token.is_empty());

        Self { addr, host, token }
    }

    fn label(&self) -> String {
        if self.token.is_some() {
            format!("http://{} (token configured)", self.host)
        } else {
            format!("http://{} (token missing, using local fallback)", self.host)
        }
    }

    fn dashboard_devices_json(&self) -> std::io::Result<String> {
        let states = self.states()?;
        let devices = real_devices_from_states(&states);
        if devices.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no Xiaomi Home devices found in Home Assistant",
            ));
        }
        Ok(json!({"source":"home_assistant","devices":devices}).to_string())
    }

    fn update_real_device(
        &self,
        id: &str,
        params: &HashMap<String, String>,
    ) -> std::io::Result<()> {
        match id {
            "real_tv" => self.update_tv(params),
            "xiaoai" => self.update_xiaoai(params),
            "real_ac" => self.update_ac(params),
            "curtain" => self.update_curtain(params),
            "thermo" => Ok(()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "unknown Home Assistant mapped device",
            )),
        }
    }

    fn update_tv(&self, params: &HashMap<String, String>) -> std::io::Result<()> {
        if let Some(on) = bool_param(params, "on") {
            self.post_service(
                "media_player",
                if on { "turn_on" } else { "turn_off" },
                json!({"entity_id": TV_ENTITY}),
            )?;
        }
        if let Some(action) = params.get("action").map(String::as_str) {
            match action {
                "volume_up" => self.press_button(TV_VOLUME_UP_BUTTON)?,
                "volume_down" => self.press_button(TV_VOLUME_DOWN_BUTTON)?,
                "mute" => self.post_service(
                    "media_player",
                    "volume_mute",
                    json!({"entity_id": TV_ENTITY, "is_volume_muted": true}),
                )?,
                "unmute" => self.post_service(
                    "media_player",
                    "volume_mute",
                    json!({"entity_id": TV_ENTITY, "is_volume_muted": false}),
                )?,
                _ => {}
            }
        }
        if let Some(volume) = int_param(params, "volume").map(|value| value.clamp(0, 100)) {
            self.post_service(
                "media_player",
                "volume_set",
                json!({"entity_id": TV_ENTITY, "volume_level": (volume as f64 / 100.0)}),
            )?;
        }
        Ok(())
    }

    fn update_xiaoai(&self, params: &HashMap<String, String>) -> std::io::Result<()> {
        if let Some(volume) = int_param(params, "volume").map(|value| value.clamp(0, 100)) {
            self.post_service(
                "media_player",
                "volume_set",
                json!({"entity_id": XIAOAI_ENTITY, "volume_level": (volume as f64 / 100.0)}),
            )?;
        }
        if let Some(on) = bool_param(params, "on") {
            self.post_service(
                "media_player",
                if on { "turn_on" } else { "turn_off" },
                json!({"entity_id": XIAOAI_ENTITY}),
            )?;
        }
        if let Some(action) = params.get("action").map(String::as_str) {
            match action {
                "wake" => self.press_button(XIAOAI_WAKE_BUTTON)?,
                "radio" => self.press_button(XIAOAI_RADIO_BUTTON)?,
                "music" => self.press_button(XIAOAI_MUSIC_BUTTON)?,
                "say" => self.post_service(
                    "notify",
                    "send_message",
                    json!({
                        "entity_id": XIAOAI_NOTIFY_ENTITY,
                        "message": "中控已连接，可以用 Siri 触发小爱了"
                    }),
                )?,
                _ => {}
            }
        }
        Ok(())
    }

    fn update_ac(&self, params: &HashMap<String, String>) -> std::io::Result<()> {
        if let Some(on) = bool_param(params, "on") {
            self.post_service(
                "climate",
                if on { "turn_on" } else { "turn_off" },
                json!({"entity_id": AC_ENTITY}),
            )?;
        }
        if let Some(mode) = params.get("mode").and_then(|mode| hvac_mode(mode)) {
            self.post_service(
                "climate",
                "set_hvac_mode",
                json!({"entity_id": AC_ENTITY, "hvac_mode": mode}),
            )?;
        }
        if let Some(temperature) = int_param(params, "temperature").map(|value| value.clamp(16, 30))
        {
            self.post_service(
                "climate",
                "set_temperature",
                json!({"entity_id": AC_ENTITY, "temperature": temperature}),
            )?;
        }
        if let Some(speed) = int_param(params, "speed") {
            self.post_service(
                "climate",
                "set_fan_mode",
                json!({"entity_id": AC_ENTITY, "fan_mode": fan_mode_from_speed(speed)}),
            )?;
        }
        Ok(())
    }

    fn update_curtain(&self, params: &HashMap<String, String>) -> std::io::Result<()> {
        if let Some(on) = bool_param(params, "on") {
            self.post_service(
                "cover",
                if on { "open_cover" } else { "close_cover" },
                json!({"entity_id": CURTAIN_ENTITY}),
            )?;
        }
        if let Some(action) = params.get("action").map(String::as_str) {
            match action {
                "open" => {
                    self.post_service("cover", "open_cover", json!({"entity_id": CURTAIN_ENTITY}))?
                }
                "close" => {
                    self.post_service("cover", "close_cover", json!({"entity_id": CURTAIN_ENTITY}))?
                }
                "stop" => {
                    self.post_service("cover", "stop_cover", json!({"entity_id": CURTAIN_ENTITY}))?
                }
                _ => {}
            }
        }
        if let Some(position) = int_param(params, "position").map(|value| value.clamp(0, 100)) {
            self.post_service(
                "cover",
                "set_cover_position",
                json!({"entity_id": CURTAIN_ENTITY, "position": position}),
            )?;
        }
        Ok(())
    }

    fn press_button(&self, entity_id: &str) -> std::io::Result<()> {
        self.post_service("button", "press", json!({"entity_id": entity_id}))
    }

    fn states(&self) -> std::io::Result<Vec<Value>> {
        let body = self.request("GET", "/api/states", None)?;
        serde_json::from_str(&body)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
    }

    fn post_service(&self, domain: &str, service: &str, payload: Value) -> std::io::Result<()> {
        let body = payload.to_string();
        self.request(
            "POST",
            &format!("/api/services/{domain}/{service}"),
            Some(&body),
        )?;
        Ok(())
    }

    fn request(&self, method: &str, path: &str, body: Option<&str>) -> std::io::Result<String> {
        let Some(token) = self.token.as_ref() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Home Assistant token is not configured",
            ));
        };
        let body = body.unwrap_or("");
        let mut stream = TcpStream::connect_timeout(&self.addr, Duration::from_millis(900))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;

        let request = format!(
            "{method} {path} HTTP/1.1\r\n\
             Host: {}\r\n\
             Authorization: Bearer {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            self.host,
            token,
            body.as_bytes().len(),
            body
        );
        stream.write_all(request.as_bytes())?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        let Some((headers, response_body)) = response.split_once("\r\n\r\n") else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid Home Assistant response",
            ));
        };
        let status = headers.lines().next().unwrap_or_default();
        if !status.contains(" 2") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Home Assistant returned {status}: {response_body}"),
            ));
        }
        Ok(response_body.to_string())
    }
}

fn is_real_device_id(id: &str) -> bool {
    matches!(id, "real_tv" | "xiaoai" | "real_ac" | "curtain" | "thermo")
}

fn bool_param(params: &HashMap<String, String>, key: &str) -> Option<bool> {
    params
        .get(key)
        .filter(|value| !value.is_empty())
        .map(|value| matches!(value.as_str(), "1" | "true" | "on" | "yes"))
}

fn int_param(params: &HashMap<String, String>, key: &str) -> Option<i64> {
    params.get(key).and_then(|value| value.parse::<i64>().ok())
}

fn hvac_mode(mode: &str) -> Option<&'static str> {
    match mode {
        "auto" => Some("auto"),
        "cool" => Some("cool"),
        "dry" => Some("dry"),
        "heat" => Some("heat"),
        "fan" | "fan_only" => Some("fan_only"),
        "off" => Some("off"),
        _ => None,
    }
}

fn fan_mode_from_speed(speed: i64) -> &'static str {
    match speed.clamp(0, 100) {
        0..=33 => "低",
        34..=66 => "中",
        _ => "高",
    }
}

fn speed_from_fan_mode(mode: &str) -> u8 {
    match mode {
        "低" => 33,
        "中" => 66,
        "高" => 100,
        _ => 45,
    }
}

fn real_devices_from_states(states: &[Value]) -> Vec<Value> {
    let mut devices = Vec::new();

    if let Some(tv) = state_by_id(states, TV_ENTITY) {
        let volume = percent_from_volume(attr_f64(tv, "volume_level"));
        let source = attr_str(tv, "source").unwrap_or_else(|| state_str(tv).to_string());
        let muted = attr_bool(tv, "is_volume_muted").unwrap_or(false);
        devices.push(json!({
            "id": "real_tv",
            "name": "客厅的小米电视",
            "room": "客厅",
            "kind": "tv",
            "on": entity_is_on(tv),
            "brightness": volume,
            "volume": volume,
            "muted": muted,
            "temperature": 0,
            "humidity": 0,
            "position": 0,
            "color": "#60a5fa",
            "speed": 0,
            "mode": source,
            "note": format!("输入源 {} · {}", source, if muted { "已静音" } else { "声音开启" }),
            "online": entity_is_available(tv),
            "readonly": false
        }));
    }

    if let Some(speaker) = state_by_id(states, XIAOAI_ENTITY) {
        let volume = percent_from_volume(attr_f64(speaker, "volume_level"));
        let muted = attr_bool(speaker, "is_volume_muted").unwrap_or(false);
        devices.push(json!({
            "id": "xiaoai",
            "name": "小爱家庭屏 mini",
            "room": "客厅",
            "kind": "speaker",
            "on": entity_is_available(speaker) && state_str(speaker) != "off",
            "brightness": volume,
            "volume": volume,
            "muted": muted,
            "temperature": 0,
            "humidity": 0,
            "position": 0,
            "color": "#b692ff",
            "speed": 0,
            "mode": state_str(speaker),
            "note": format!("音量 {}% · 可唤醒、播报和播放音乐", volume),
            "online": entity_is_available(speaker),
            "readonly": false
        }));
    }

    if let Some(ac) = state_by_id(states, AC_ENTITY) {
        let temperature = attr_f64(ac, "temperature").unwrap_or(24.0).round() as i64;
        let fan_mode = attr_str(ac, "fan_mode").unwrap_or_else(|| "自动".to_string());
        let state = state_str(ac).to_string();
        devices.push(json!({
            "id": "real_ac",
            "name": "二楼主卧空调",
            "room": "主卧",
            "kind": "climate",
            "on": entity_is_available(ac) && state != "off",
            "brightness": temperature,
            "volume": 0,
            "temperature": temperature,
            "humidity": 0,
            "position": 0,
            "color": "#38bdf8",
            "speed": speed_from_fan_mode(&fan_mode),
            "mode": state,
            "note": format!("目标 {}°C · 风量 {}", temperature, fan_mode),
            "online": entity_is_available(ac),
            "readonly": false
        }));
    }

    if let Some(curtain) = state_by_id(states, CURTAIN_ENTITY) {
        let position = attr_f64(curtain, "current_position").unwrap_or(0.0).round() as i64;
        devices.push(json!({
            "id": "curtain",
            "name": "隔断帘",
            "room": "厨房",
            "kind": "cover",
            "on": entity_is_available(curtain) && state_str(curtain) != "closed",
            "brightness": position,
            "volume": 0,
            "temperature": 0,
            "humidity": 0,
            "position": position,
            "color": "#22c55e",
            "speed": position,
            "mode": state_str(curtain),
            "note": format!("当前位置 {}%", position),
            "online": entity_is_available(curtain),
            "readonly": false
        }));
    }

    if let Some(temp) = state_by_id(states, TEMPERATURE_ENTITY) {
        let temperature = state_number(temp).unwrap_or(0.0);
        let humidity = state_by_id(states, HUMIDITY_ENTITY)
            .and_then(state_number)
            .unwrap_or(0.0);
        let battery = state_by_id(states, BATTERY_ENTITY)
            .and_then(state_number)
            .unwrap_or(0.0);
        devices.push(json!({
            "id": "thermo",
            "name": "米家温湿度计",
            "room": "厨房",
            "kind": "sensor",
            "on": entity_is_available(temp),
            "brightness": temperature.round() as i64,
            "volume": 0,
            "temperature": one_decimal(temperature),
            "humidity": humidity.round() as i64,
            "position": 0,
            "color": "#f59e0b",
            "speed": 0,
            "mode": "measure",
            "note": format!("湿度 {}% · 电量 {}%", humidity.round() as i64, battery.round() as i64),
            "online": entity_is_available(temp),
            "readonly": true
        }));
    }

    devices
}

fn state_by_id<'a>(states: &'a [Value], entity_id: &str) -> Option<&'a Value> {
    states
        .iter()
        .find(|state| state.get("entity_id").and_then(Value::as_str) == Some(entity_id))
}

fn state_str(state: &Value) -> &str {
    state
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn entity_is_available(state: &Value) -> bool {
    !matches!(state_str(state), "unavailable" | "unknown")
}

fn entity_is_on(state: &Value) -> bool {
    entity_is_available(state) && !matches!(state_str(state), "off" | "closed")
}

fn attrs(state: &Value) -> Option<&Value> {
    state.get("attributes")
}

fn attr_str(state: &Value, key: &str) -> Option<String> {
    attrs(state)?
        .get(key)?
        .as_str()
        .map(|value| value.to_string())
}

fn attr_bool(state: &Value, key: &str) -> Option<bool> {
    attrs(state)?.get(key)?.as_bool()
}

fn attr_f64(state: &Value, key: &str) -> Option<f64> {
    attrs(state)?.get(key)?.as_f64()
}

fn state_number(state: &Value) -> Option<f64> {
    state_str(state).parse::<f64>().ok()
}

fn percent_from_volume(volume: Option<f64>) -> u8 {
    (volume.unwrap_or(0.0) * 100.0).round().clamp(0.0, 100.0) as u8
}

fn one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn handle_client(
    mut stream: TcpStream,
    devices: SharedDevices,
    state_path: PathBuf,
    ha_client: HaClient,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = read_request(&mut stream)?;
    let (method, path, body) = match request {
        Some(request) => request,
        None => return Ok(()),
    };

    let (status, content_type, response_body) =
        route_request(&method, &path, &body, devices, state_path, &ha_client);
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
    ha_client: &HaClient,
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
        let ha = if ha_client.token.is_some() {
            "configured"
        } else {
            "not_configured"
        };
        return (
            "200 OK",
            "application/json; charset=utf-8",
            format!(r#"{{"ok":true,"devices":{count},"home_assistant":"{ha}"}}"#),
        );
    }

    if method == "GET" && path == "/api/devices" {
        if let Ok(json) = ha_client.dashboard_devices_json() {
            return ("200 OK", "application/json; charset=utf-8", json);
        }
        let json = devices
            .lock()
            .map(|devices| devices_json(&devices))
            .unwrap_or_else(|_| r#"{"devices":[]}"#.to_string());
        return ("200 OK", "application/json; charset=utf-8", json);
    }

    if method == "POST" {
        if let Some(id) = path.strip_prefix("/api/devices/") {
            let params = parse_form(body);
            if is_real_device_id(id) {
                return match ha_client.update_real_device(id, &params) {
                    Ok(()) => (
                        "200 OK",
                        "application/json; charset=utf-8",
                        r#"{"ok":true,"source":"home_assistant"}"#.to_string(),
                    ),
                    Err(err) => (
                        "502 Bad Gateway",
                        "application/json; charset=utf-8",
                        format!(
                            r#"{{"ok":false,"error":"{}"}}"#,
                            escape_json(&err.to_string())
                        ),
                    ),
                };
            }
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
    if let Some(value) = params
        .get("brightness")
        .and_then(|value| value.parse::<u8>().ok())
    {
        device.brightness = value.min(100);
    }
    if let Some(value) = params
        .get("volume")
        .and_then(|value| value.parse::<u8>().ok())
    {
        device.brightness = value.min(100);
    }
    if let Some(value) = params
        .get("temperature")
        .and_then(|value| value.parse::<u8>().ok())
    {
        device.brightness = if device.kind == "climate" {
            value.clamp(16, 30)
        } else {
            value.min(100)
        };
    }
    if let Some(value) = params
        .get("speed")
        .and_then(|value| value.parse::<u8>().ok())
    {
        device.speed = value.min(100);
    }
    if let Some(value) = params.get("mode").filter(|value| !value.is_empty()) {
        device.mode = value.chars().take(32).collect();
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
    let addr: SocketAddr = "127.0.0.1:8123"
        .parse()
        .expect("static Home Assistant address");
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;

    let body = format!(
        r#"{{"id":"{}","on":{},"brightness":{},"volume":{},"temperature":{},"color":"{}","speed":{},"mode":"{}","note":"{}"}}"#,
        escape_json(&device.id),
        device.on,
        device.brightness,
        if device.kind == "tv" {
            device.brightness
        } else {
            0
        },
        if device.kind == "climate" {
            device.brightness
        } else {
            0
        },
        escape_json(&device.color),
        device.speed,
        escape_json(&device.mode),
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
        && value.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
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
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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
            device.brightness = fields[5]
                .parse::<u8>()
                .unwrap_or(device.brightness)
                .min(100);
            device.color = if is_hex_color(fields[6]) {
                fields[6].to_string()
            } else {
                device.color.clone()
            };
            device.speed = fields[7].parse::<u8>().unwrap_or(device.speed).min(100);
            if fields.len() >= 10 {
                device.mode = fields[8].to_string();
                device.note = fields[9].to_string();
            } else if let Some(note) = fields.get(8) {
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
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            device.id,
            device.name,
            device.room,
            device.kind,
            if device.on { "1" } else { "0" },
            device.brightness,
            device.color,
            device.speed,
            device.mode.replace('\t', " "),
            device.note.replace('\t', " ")
        ));
    }
    fs::write(path, output)
}

fn default_devices() -> Vec<Device> {
    vec![
        Device {
            id: "desk_lamp".to_string(),
            name: "米家台灯".to_string(),
            room: "书桌".to_string(),
            kind: "light".to_string(),
            on: false,
            brightness: 65,
            color: "#ffd36b".to_string(),
            speed: 0,
            mode: "warm".to_string(),
            note: "亮度映射到台灯色块".to_string(),
        },
        Device {
            id: "air_purifier".to_string(),
            name: "米家空气净化器".to_string(),
            room: "客厅".to_string(),
            kind: "fan".to_string(),
            on: false,
            brightness: 0,
            color: "#7bd389".to_string(),
            speed: 35,
            mode: "auto".to_string(),
            note: "风速越高，色块脉冲越快".to_string(),
        },
        Device {
            id: "tv".to_string(),
            name: "米家电视".to_string(),
            room: "客厅".to_string(),
            kind: "tv".to_string(),
            on: false,
            brightness: 35,
            color: "#60a5fa".to_string(),
            speed: 0,
            mode: "media".to_string(),
            note: "音量映射到电视控制".to_string(),
        },
        Device {
            id: "ac_companion".to_string(),
            name: "米家空调伴侣".to_string(),
            room: "卧室".to_string(),
            kind: "climate".to_string(),
            on: false,
            brightness: 24,
            color: "#38bdf8".to_string(),
            speed: 45,
            mode: "cool".to_string(),
            note: "温度、模式和风量映射到空调伴侣".to_string(),
        },
        Device {
            id: "xiaoai_scene".to_string(),
            name: "小爱音箱场景".to_string(),
            room: "语音".to_string(),
            kind: "switch".to_string(),
            on: false,
            brightness: 75,
            color: "#b692ff".to_string(),
            speed: 0,
            mode: "scene".to_string(),
            note: "模拟“小爱执行场景”开关".to_string(),
        },
    ]
}

fn devices_json(devices: &[Device]) -> String {
    let devices = devices
        .iter()
        .map(device_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"devices":[{devices}]}}"#)
}

fn device_json(device: &Device) -> String {
    format!(
        r#"{{"id":"{}","name":"{}","room":"{}","kind":"{}","on":{},"brightness":{},"volume":{},"temperature":{},"color":"{}","speed":{},"mode":"{}","note":"{}"}}"#,
        escape_json(&device.id),
        escape_json(&device.name),
        escape_json(&device.room),
        escape_json(&device.kind),
        device.on,
        device.brightness,
        if device.kind == "tv" {
            device.brightness
        } else {
            0
        },
        if device.kind == "climate" {
            device.brightness
        } else {
            0
        },
        escape_json(&device.color),
        device.speed,
        escape_json(&device.mode),
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
  <title>米家中控</title>
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
	    .power-button:disabled {
	      opacity: .42;
	      cursor: default;
	      transform: none;
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
    .action-buttons,
    .mode-buttons,
    .temperature-stepper {
      display: grid;
      gap: 8px;
    }
    .action-buttons {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .mode-buttons {
      grid-template-columns: repeat(5, minmax(0, 1fr));
    }
    .temperature-stepper {
      grid-template-columns: 56px minmax(0, 1fr) 56px;
      align-items: center;
      min-height: 58px;
      padding: 8px 12px;
      border: 1px solid #e5eaf1;
      border-radius: var(--radius);
      background: var(--surface-soft);
    }
    .mini-button,
    .mode-button {
      min-height: 48px;
      border: 1px solid var(--line);
      border-radius: var(--radius);
      background: #fff;
      color: #344054;
      font-size: 14px;
      font-weight: 740;
    }
    .mini-button:active,
    .mode-button:active {
      transform: scale(.97);
    }
    .mode-button.is-active {
      border-color: rgba(23, 178, 106, .35);
      background: var(--green);
      color: #fff;
    }
	    .temperature-readout {
	      display: grid;
	      place-items: center;
	      min-height: 48px;
	      color: #111827;
	      font-size: 24px;
	      font-weight: 800;
	    }
	    .sensor-readout {
	      display: grid;
	      grid-template-columns: repeat(2, minmax(0, 1fr));
	      gap: 8px;
	    }
	    .sensor-tile {
	      min-height: 68px;
	      padding: 10px 12px;
	      border: 1px solid #e5eaf1;
	      border-radius: var(--radius);
	      background: var(--surface-soft);
	    }
	    .sensor-tile strong {
	      display: block;
	      font-size: 24px;
	      line-height: 1.05;
	    }
	    .sensor-tile span {
	      display: block;
	      margin-top: 6px;
	      color: var(--muted);
	      font-size: 13px;
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
          <h1>米家中控</h1>
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
            <span class="scene-icon" aria-hidden="true">▣</span><span class="scene-copy"><strong>影院</strong><span>电视开启，主灯压暗</span></span>
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
	    let currentDevices = [];

	    function clamp(value, min, max) {
	      return Math.max(min, Math.min(max, value));
	    }

	    function escapeHtml(value) {
	      return String(value ?? '').replace(/[&<>"']/g, char => ({
	        '&': '&amp;',
	        '<': '&lt;',
	        '>': '&gt;',
	        '"': '&quot;',
	        "'": '&#39;'
	      }[char]));
	    }

	    function hasDevice(id) {
	      return currentDevices.some(device => device.id === id);
	    }

	    function mappedId(realId, fallbackId) {
	      return hasDevice(realId) ? realId : fallbackId;
	    }

	    function deviceSymbol(device) {
	      if (device.kind === 'light') return '▱';
	      if (device.kind === 'fan') return '✺';
	      if (device.kind === 'tv') return '▭';
	      if (device.kind === 'climate') return '❄';
	      if (device.kind === 'speaker') return '◉';
	      if (device.kind === 'cover') return '▥';
	      if (device.kind === 'sensor') return '℃';
	      return '▦';
	    }

	    function deviceTypeLabel(device) {
	      if (device.kind === 'light') return '灯光';
	      if (device.kind === 'fan') return '净化';
	      if (device.kind === 'tv') return '电视';
	      if (device.kind === 'climate') return '空调';
	      if (device.kind === 'speaker') return '小爱';
	      if (device.kind === 'cover') return '窗帘';
	      if (device.kind === 'sensor') return '环境';
	      return '场景';
	    }

	    function climateModeLabel(mode) {
	      const labels = { cool: '制冷', heat: '制热', dry: '除湿', fan: '送风', fan_only: '送风', auto: '自动', off: '关闭' };
	      return labels[mode] || escapeHtml(mode || '自动');
	    }

	    function summaryCards(devices) {
	      const active = devices.filter(device => device.on && !device.readonly).length;
	      const tv = devices.find(device => device.kind === 'tv');
	      const ac = devices.find(device => device.kind === 'climate');
	      const thermo = devices.find(device => device.kind === 'sensor');
	      const curtain = devices.find(device => device.kind === 'cover');
	      const cards = [
	        ['⏻', active, '设备开启中', 'var(--green)'],
	        ['▭', tv && tv.on ? Number(tv.volume || 0) + '%' : '待机', '电视音量', 'var(--blue)'],
	        ['℃', thermo ? Number(thermo.temperature || 0).toFixed(1) + '°C' : '--', '室内温度', 'var(--amber)'],
	        ['▥', curtain ? Number(curtain.position || 0) + '%' : (ac && ac.on ? Number(ac.temperature || ac.brightness || 24) + '°C' : '待机'), curtain ? '窗帘开合' : '空调', 'var(--violet)']
	      ];
	      overview.innerHTML = cards.map(([icon, value, label, color]) => `
	        <article class="stat-card">
	          <div class="stat-icon" style="--stat-color:${color}">${icon}</div>
	          <div><div class="stat-value">${escapeHtml(value)}</div><div class="stat-label">${escapeHtml(label)}</div></div>
	        </article>`).join('');
	    }

	    function sliderControl(device, label, action, value, suffix = '%', min = 0, max = 100) {
	      const safeValue = Number(value || 0);
	      return `<div class="control-row"><div class="control-label">${escapeHtml(label)}</div><input aria-label="${escapeHtml(device.name)}${escapeHtml(label)}" type="range" min="${min}" max="${max}" value="${safeValue}" data-action="${action}" data-id="${escapeHtml(device.id)}"><div class="control-value">${safeValue}${escapeHtml(suffix)}</div></div>`;
	    }

	    function colorControl(device) {
	      return `<div class="color-row"><div class="control-label">颜色</div><div class="control-value">${escapeHtml(device.color)}</div><input aria-label="${escapeHtml(device.name)}颜色" type="color" value="${escapeHtml(device.color)}" data-action="color" data-id="${escapeHtml(device.id)}"></div>`;
	    }

	    function tvControls(device) {
	      const volume = Number(device.volume || device.brightness || 0);
	      return `
	        ${sliderControl(device, '音量', 'volume', volume)}
	        <div class="action-buttons" aria-label="${escapeHtml(device.name)}音量快捷控制">
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="${device.muted ? 'unmute' : 'mute'}">${device.muted ? '取消静音' : '静音'}</button>
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="volume_down">-</button>
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="volume_up">+</button>
	        </div>`;
	    }

	    function speakerControls(device) {
	      const volume = Number(device.volume || device.brightness || 0);
	      return `
	        ${sliderControl(device, '音量', 'volume', volume)}
	        <div class="action-buttons" aria-label="${escapeHtml(device.name)}快捷控制">
	          <button class="mini-button" data-action="speaker-action" data-id="${escapeHtml(device.id)}" data-command="wake">唤醒</button>
	          <button class="mini-button" data-action="speaker-action" data-id="${escapeHtml(device.id)}" data-command="music">音乐</button>
	          <button class="mini-button" data-action="speaker-action" data-id="${escapeHtml(device.id)}" data-command="say">播报</button>
	        </div>`;
	    }

	    function coverControls(device) {
	      const position = Number(device.position || device.brightness || 0);
	      return `
	        ${sliderControl(device, '位置', 'position', position)}
	        <div class="action-buttons" aria-label="${escapeHtml(device.name)}快捷控制">
	          <button class="mini-button" data-action="cover-action" data-id="${escapeHtml(device.id)}" data-command="open">打开</button>
	          <button class="mini-button" data-action="cover-action" data-id="${escapeHtml(device.id)}" data-command="stop">停止</button>
	          <button class="mini-button" data-action="cover-action" data-id="${escapeHtml(device.id)}" data-command="close">关闭</button>
	        </div>`;
	    }

	    function sensorControls(device) {
	      return `
	        <div class="sensor-readout">
	          <div class="sensor-tile"><strong>${Number(device.temperature || 0).toFixed(1)}°C</strong><span>温度</span></div>
	          <div class="sensor-tile"><strong>${Number(device.humidity || 0)}%</strong><span>湿度</span></div>
	        </div>`;
	    }

	    function climateControls(device) {
	      const temperature = Number(device.temperature || device.brightness || 24);
	      const fanSpeed = Number(device.speed || 0);
	      const modes = [['cool', '制冷'], ['heat', '制热'], ['dry', '除湿'], ['fan', '送风'], ['auto', '自动']];
	      return `
	        <div class="temperature-stepper" aria-label="${escapeHtml(device.name)}温度控制">
	          <button class="mini-button" data-action="temp-step" data-id="${escapeHtml(device.id)}" data-delta="-1" data-current="${temperature}">-</button>
	          <div class="temperature-readout">${temperature}°C</div>
	          <button class="mini-button" data-action="temp-step" data-id="${escapeHtml(device.id)}" data-delta="1" data-current="${temperature}">+</button>
	        </div>
	        ${sliderControl(device, '风量', 'speed', fanSpeed)}
	        <div class="mode-buttons" aria-label="${escapeHtml(device.name)}模式">
	          ${modes.map(([mode, label]) => `<button class="mode-button ${device.mode === mode || (mode === 'fan' && device.mode === 'fan_only') ? 'is-active' : ''}" data-action="mode" data-id="${escapeHtml(device.id)}" data-mode="${mode}">${label}</button>`).join('')}
	        </div>`;
	    }

	    function deviceControls(device) {
	      if (device.kind === 'light') {
	        return `${sliderControl(device, '亮度', 'brightness', device.brightness)}${colorControl(device)}`;
	      }
	      if (device.kind === 'fan') {
	        return sliderControl(device, '风速', 'speed', device.speed);
	      }
	      if (device.kind === 'tv') {
	        return tvControls(device);
	      }
	      if (device.kind === 'speaker') {
	        return speakerControls(device);
	      }
	      if (device.kind === 'climate') {
	        return climateControls(device);
	      }
	      if (device.kind === 'cover') {
	        return coverControls(device);
	      }
	      if (device.kind === 'sensor') {
	        return sensorControls(device);
	      }
	      return `<div class="control-row"><div class="control-label">场景</div><div></div><div class="control-value">${device.on ? '执行中' : '待机'}</div></div>`;
	    }

	    function visualLevel(device) {
	      if (device.kind === 'fan') return device.speed;
	      if (device.kind === 'tv' || device.kind === 'speaker') return device.volume || device.brightness;
	      if (device.kind === 'climate') return device.on ? clamp(device.speed || 45, 18, 100) : 8;
	      if (device.kind === 'cover') return clamp(device.position || device.brightness || 0, 12, 100);
	      if (device.kind === 'sensor') return 70;
	      if (device.kind === 'switch') return device.on ? 76 : 8;
	      return device.brightness;
	    }

	    function stateWord(device) {
	      if (device.kind === 'sensor') return Number(device.temperature || 0).toFixed(1) + '°C';
	      if (device.kind === 'cover') return Number(device.position || 0) + '%';
	      if (device.kind === 'climate') return device.on ? climateModeLabel(device.mode) : '关闭';
	      if (device.kind === 'speaker') return device.online ? '待命' : '离线';
	      if (device.kind === 'tv') return device.on ? '开' : '待机';
	      return device.on ? '开' : '关';
	    }

	    function deviceCard(device) {
	      const activeBrightness = device.on ? clamp(visualLevel(device), 8, 100) : 8;
	      const opacity = device.on || device.readonly ? activeBrightness / 100 : 0.20;
	      const brightness = device.on || device.readonly ? 0.74 + activeBrightness / 125 : 0.54;
	      const saturation = device.on || device.readonly ? 1.15 : 0.38;
	      const spinSeconds = Math.max(0.45, 3.2 - (Number(device.speed || 0) / 100) * 2.6).toFixed(2) + 's';
	      const stateColor = device.online === false ? '#ef4444' : (device.on || device.readonly ? 'var(--green)' : '#64748b');
	      const onlineText = device.online === false ? '离线' : '在线';
	      const powerControl = device.readonly
	        ? `<button class="power-button" aria-label="${escapeHtml(device.name)}只读" disabled>•</button>`
	        : `<button class="power-button" aria-label="${escapeHtml(device.name)}${device.on ? '关闭' : '开启'}" data-action="toggle" data-id="${escapeHtml(device.id)}" data-on="${Boolean(device.on)}">⏻</button>`;
	      return `
	        <article class="device-card" data-kind="${escapeHtml(device.kind)}" data-on="${Boolean(device.on)}"
	          style="--device-color:${escapeHtml(device.color)};--device-opacity:${opacity};--device-brightness:${brightness};--device-saturation:${saturation};--spin-speed:${spinSeconds};--state-color:${stateColor}">
	          <div class="device-visual"><div class="device-symbol">${deviceSymbol(device)}</div></div>
	          <div class="device-body">
	            <div class="device-head">
	              <div>
	                <div class="device-name-row"><h2>${escapeHtml(device.name)}</h2><span class="online">${onlineText}</span></div>
	                <div class="room">${escapeHtml(device.room)} · ${deviceTypeLabel(device)}</div>
	                <div class="state-word">${stateWord(device)}</div>
	              </div>
	              ${powerControl}
	            </div>
	            <div class="control-stack">
	              ${deviceControls(device)}
	            </div>
	            <div class="note">${escapeHtml(device.note)}</div>
	          </div>
	        </article>`;
	    }

	    async function loadDevices() {
	      const response = await fetch('/api/devices', { cache: 'no-store' });
	      if (!response.ok) throw new Error('HTTP ' + response.status);
	      const json = await response.text();
	      if (json !== lastJson) {
	        const data = JSON.parse(json);
	        currentDevices = data.devices || [];
	        summaryCards(currentDevices);
	        grid.innerHTML = currentDevices.map(deviceCard).join('');
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
	      if (!target || target.disabled) return;
	      updateDevice(target.dataset.id, { on: target.dataset.on !== 'true' }).catch(showError);
	    });

	    document.addEventListener('click', event => {
	      const target = event.target.closest('[data-action="tv-action"],[data-action="speaker-action"],[data-action="cover-action"],[data-action="temp-step"],[data-action="mode"]');
	      if (!target) return;
	      if (target.dataset.action === 'tv-action') {
	        updateDevice(target.dataset.id, { action: target.dataset.command, on: true }).catch(showError);
	      }
	      if (target.dataset.action === 'speaker-action') {
	        updateDevice(target.dataset.id, { action: target.dataset.command, on: true }).catch(showError);
	      }
	      if (target.dataset.action === 'cover-action') {
	        updateDevice(target.dataset.id, { action: target.dataset.command }).catch(showError);
	      }
	      if (target.dataset.action === 'temp-step') {
	        const temperature = Number(target.dataset.current || 24) + Number(target.dataset.delta || 0);
	        updateDevice(target.dataset.id, { temperature: clamp(temperature, 16, 30), on: true, note: '网页快捷操作：空调温度' }).catch(showError);
	      }
	      if (target.dataset.action === 'mode') {
	        updateDevice(target.dataset.id, { mode: target.dataset.mode, on: true, note: '网页快捷操作：空调模式' }).catch(showError);
	      }
	    });

	    document.addEventListener('change', event => {
	      const target = event.target;
	      if (!target.dataset.action) return;
	      const card = target.closest('.device-card');
	      const kind = card ? card.dataset.kind : '';
	      if (target.dataset.action === 'brightness') {
	        updateDevice(target.dataset.id, { brightness: target.value, on: Number(target.value) > 0 }).catch(showError);
	      }
	      if (target.dataset.action === 'speed') {
	        updateDevice(target.dataset.id, { speed: target.value, on: kind === 'fan' ? Number(target.value) > 0 : true }).catch(showError);
	      }
	      if (target.dataset.action === 'volume') {
	        updateDevice(target.dataset.id, { volume: target.value, on: true, note: '网页快捷操作：音量' }).catch(showError);
	      }
	      if (target.dataset.action === 'position') {
	        updateDevice(target.dataset.id, { position: target.value }).catch(showError);
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
	      const realMode = hasDevice('real_tv') || hasDevice('xiaoai') || hasDevice('real_ac') || hasDevice('curtain');
	      const tv = mappedId('real_tv', 'tv');
	      const ac = mappedId('real_ac', 'ac_companion');
	      const speaker = hasDevice('xiaoai') ? 'xiaoai' : (realMode ? '' : 'xiaoai_scene');
	      const curtain = mappedId('curtain', '');
	      const lamp = realMode ? '' : 'desk_lamp';
	      const purifier = realMode ? '' : 'air_purifier';
	      const scenes = {
	        'all-on': [
	          [lamp, { on: true, brightness: 90, color: '#ffd36b', note: '网页快捷操作：全开' }],
	          [purifier, { on: true, speed: 55, note: '网页快捷操作：全开' }],
	          [tv, { on: true, volume: 35, note: '网页快捷操作：全开' }],
	          [ac, { on: true, temperature: 24, speed: 50, mode: 'cool', note: '网页快捷操作：全开' }],
	          [speaker, { on: true, action: hasDevice('xiaoai') ? 'wake' : undefined, brightness: 90, note: '网页快捷操作：全开' }],
	          [curtain, { action: 'open' }]
	        ],
	        'all-off': [
	          [lamp, { on: false, brightness: 25, note: '网页快捷操作：全关' }],
	          [purifier, { on: false, speed: 0, note: '网页快捷操作：全关' }],
	          [tv, { on: false, volume: 0, note: '网页快捷操作：全关' }],
	          [ac, { on: false, temperature: 24, speed: 0, mode: 'cool', note: '网页快捷操作：全关' }],
	          [curtain, { action: 'close' }]
	        ],
	        'movie': [
	          [lamp, { on: false, brightness: 12, color: '#f97316', note: '网页快捷操作：影院模式' }],
	          [purifier, { on: true, speed: 25, note: '网页快捷操作：影院模式' }],
	          [tv, { on: true, volume: 32, note: '网页快捷操作：影院模式' }],
	          [ac, { on: true, temperature: 25, speed: 25, mode: 'cool', note: '网页快捷操作：影院模式' }],
	          [speaker, { on: true, action: hasDevice('xiaoai') ? 'music' : undefined, brightness: 65, color: '#b692ff', note: '网页快捷操作：影院模式' }],
	          [curtain, { position: 18 }]
	        ],
	        'night': [
	          [lamp, { on: true, brightness: 18, color: '#f97316', note: '网页快捷操作：夜间模式' }],
	          [purifier, { on: true, speed: 18, note: '网页快捷操作：夜间模式' }],
	          [tv, { on: false, volume: 0, note: '网页快捷操作：夜间模式' }],
	          [ac, { on: true, temperature: 26, speed: 20, mode: 'cool', note: '网页快捷操作：夜间模式' }],
	          [curtain, { position: 0 }]
	        ],
	        'purify': [
	          [hasDevice('curtain') ? 'curtain' : purifier, hasDevice('curtain') ? { position: 50 } : { on: true, speed: 100, note: '网页快捷操作：净化器强档' }]
	        ]
	      };
	      if (trigger) trigger.classList.add('is-running');
	      for (const [id, params] of scenes[scene] || []) {
	        if (!id) continue;
	        await updateDevice(id, Object.fromEntries(Object.entries(params).filter(([, value]) => value !== undefined)));
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
