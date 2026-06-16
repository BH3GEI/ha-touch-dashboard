use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

const DEFAULT_HA_ADDR: &str = "127.0.0.1:8123";
const DEFAULT_HA_TOKEN_FILE: &str = "/Users/mac/HomeAssistantCore/HA-OWNER-ACCESS-TOKEN.txt";
const DEFAULT_HA_REFRESH_TOKEN_FILE: &str =
    "/Users/mac/HomeAssistantCore/HA-OWNER-REFRESH-TOKEN.txt";
const DEFAULT_XIAOMI_HOME_DEVICE_CACHE: &str =
    "/Users/mac/HomeAssistantCore/config/.storage/xiaomi_home/miot_devices";
const HA_CLIENT_ID: &str = "http://127.0.0.1:8787/";
const CAMERA_HOME: &str = "李尧家（摄像头）";
const MAIN_HOME: &str = "李尧家（主设备）";

const TV_ENTITY: &str =
    "media_player.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1";
const XIAOAI_ENTITY: &str = "media_player.xiaomi_cn_2037162573_x4b";
const AC_ENTITY: &str = "climate.lumi_cn_974076238_mcn02";
const CURTAIN_ENTITY: &str = "cover.czmydz_cn_2143837107_mym1_s_2_curtain";
const TEMPERATURE_ENTITY: &str = "sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_temperature_p_2_1001";
const HUMIDITY_ENTITY: &str =
    "sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_relative_humidity_p_2_1002";
const BATTERY_ENTITY: &str = "sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_battery_level_p_3_1003";
const CAMERA_MIMI_POWER: &str = "switch.chuangmi_cn_1212440277_039c01_on_p_2_1";
const CAMERA_MIMI_STATUS: &str = "sensor.chuangmi_cn_1212440277_039c01_status_p_4_1";
const CAMERA_MIMI_INDICATOR: &str = "light.chuangmi_cn_1212440277_039c01_s_3_indicator_light";
const CAMERA_WANGWANG_POWER: &str = "switch.chuangmi_cn_2115992412_029a02_on_p_2_1";
const CAMERA_WANGWANG_STATUS: &str = "sensor.chuangmi_cn_2115992412_029a02_status_p_4_1";
const CAMERA_WANGWANG_STREAM_STATUS: &str =
    "sensor.chuangmi_cn_2115992412_029a02_stream_status_p_7_9";
const CAMERA_WANGWANG_INDICATOR: &str = "light.chuangmi_cn_2115992412_029a02_s_3_indicator_light";
const DEFAULT_GO2RTC_BASE_URL: &str = "http://192.168.3.8:1984";
const TV_VOLUME_UP_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_volume_up_a_7_12";
const TV_VOLUME_DOWN_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_volume_down_a_7_11";
const TV_HOME_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_home_a_7_2";
const TV_BACK_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_back_a_7_5";
const TV_OK_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_ok_a_7_10";
const TV_PLAY_PAUSE_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_press_play_pause_a_7_16";
const TV_TURN_ON_BUTTON: &str =
    "button.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1_turn_on_a_6_1";
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
    token: Arc<Mutex<Option<String>>>,
    token_file: Option<PathBuf>,
    refresh_token: Arc<Mutex<Option<String>>>,
    refresh_token_file: Option<PathBuf>,
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
        let token_file = arg_value("--ha-token-file")
            .or_else(|| env::var("HA_TOKEN_FILE").ok())
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(DEFAULT_HA_TOKEN_FILE)));
        let refresh_token_file = arg_value("--ha-refresh-token-file")
            .or_else(|| env::var("HA_REFRESH_TOKEN_FILE").ok())
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(DEFAULT_HA_REFRESH_TOKEN_FILE)));
        let token = arg_value("--ha-token")
            .or_else(|| env::var("HA_TOKEN").ok())
            .or_else(|| read_optional_secret(token_file.as_ref()))
            .filter(|token| !token.is_empty());
        let refresh_token = arg_value("--ha-refresh-token")
            .or_else(|| env::var("HA_REFRESH_TOKEN").ok())
            .or_else(|| read_optional_secret(refresh_token_file.as_ref()))
            .filter(|token| !token.is_empty());

        Self {
            addr,
            host,
            token: Arc::new(Mutex::new(token)),
            token_file,
            refresh_token: Arc::new(Mutex::new(refresh_token)),
            refresh_token_file,
        }
    }

    fn label(&self) -> String {
        if self.current_token().is_some() {
            format!("http://{} (token configured)", self.host)
        } else {
            format!("http://{} (token missing)", self.host)
        }
    }

    fn dashboard_devices_json(&self) -> std::io::Result<String> {
        let states = self.states()?;
        let devices = dashboard_devices_from_states(&states);
        if devices.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no Xiaomi Home devices found in Home Assistant",
            ));
        }
        Ok(json!({"source":"home_assistant","devices":devices}).to_string())
    }

    fn dashboard_device_count(&self) -> std::io::Result<usize> {
        let states = self.states()?;
        Ok(dashboard_devices_from_states(&states).len())
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
            if on {
                self.press_button(TV_TURN_ON_BUTTON)?;
            }
        }
        if let Some(action) = params.get("action").map(String::as_str) {
            match action {
                "volume_up" => self.press_button(TV_VOLUME_UP_BUTTON)?,
                "volume_down" => self.press_button(TV_VOLUME_DOWN_BUTTON)?,
                "home" => self.press_button(TV_HOME_BUTTON)?,
                "back" => self.press_button(TV_BACK_BUTTON)?,
                "ok" => self.press_button(TV_OK_BUTTON)?,
                "play_pause" => self.press_button(TV_PLAY_PAUSE_BUTTON)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn update_xiaoai(&self, params: &HashMap<String, String>) -> std::io::Result<()> {
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

    fn start_camera_stream(&self, id: &str, _quality: i64) -> std::io::Result<Value> {
        match id {
            "camera_wangwang" | "camera_mimi" => start_go2rtc_camera_stream(id),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "unknown camera",
            )),
        }
    }

    fn stop_camera_stream(&self, id: &str) -> std::io::Result<()> {
        match id {
            "camera_wangwang" | "camera_mimi" => Ok(()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "unknown camera",
            )),
        }
    }

    fn current_token(&self) -> Option<String> {
        self.token.lock().ok().and_then(|token| token.clone())
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
        let body = body.unwrap_or("");
        let (status, response_body) =
            self.request_once(method, path, body, "application/json", self.current_token())?;
        if is_unauthorized(&status) {
            self.refresh_access_token()?;
            let (status, response_body) =
                self.request_once(method, path, body, "application/json", self.current_token())?;
            if !status.contains(" 2") {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Home Assistant returned {status}: {response_body}"),
                ));
            }
            return Ok(response_body);
        }
        if !status.contains(" 2") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Home Assistant returned {status}: {response_body}"),
            ));
        }
        Ok(response_body)
    }

    fn refresh_access_token(&self) -> std::io::Result<()> {
        let refresh_token = self
            .refresh_token
            .lock()
            .ok()
            .and_then(|token| token.clone())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Home Assistant refresh token is not configured",
                )
            })?;
        let body = format!(
            "grant_type=refresh_token&refresh_token={}&client_id={}",
            form_encode(&refresh_token),
            form_encode(HA_CLIENT_ID)
        );
        let (status, response_body) = self.request_once(
            "POST",
            "/auth/token",
            &body,
            "application/x-www-form-urlencoded",
            None,
        )?;
        if !status.contains(" 2") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Home Assistant token refresh failed: {status}: {response_body}"),
            ));
        }
        let value: Value = serde_json::from_str(&response_body)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        let access_token = value
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Home Assistant token refresh response missed access_token",
                )
            })?
            .to_string();
        if let Ok(mut token) = self.token.lock() {
            *token = Some(access_token.clone());
        }
        if let Some(path) = &self.token_file {
            if let Err(err) = fs::write(path, &access_token) {
                eprintln!("failed to persist refreshed Home Assistant access token: {err}");
            }
        }
        if let Some(new_refresh_token) = value.get("refresh_token").and_then(Value::as_str) {
            if let Ok(mut token) = self.refresh_token.lock() {
                *token = Some(new_refresh_token.to_string());
            }
            if let Some(path) = &self.refresh_token_file {
                if let Err(err) = fs::write(path, new_refresh_token) {
                    eprintln!("failed to persist refreshed Home Assistant refresh token: {err}");
                }
            }
        }
        Ok(())
    }

    fn request_once(
        &self,
        method: &str,
        path: &str,
        body: &str,
        content_type: &str,
        token: Option<String>,
    ) -> std::io::Result<(String, String)> {
        let mut stream = TcpStream::connect_timeout(&self.addr, Duration::from_millis(900))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;

        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\n\
             Host: {}\r\n\
             {}\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            self.host,
            authorization,
            content_type,
            body.as_bytes().len(),
            body
        );
        stream.write_all(request.as_bytes())?;

        read_http_response(&mut stream)
    }
}

fn read_http_response(stream: &mut TcpStream) -> std::io::Result<(String, String)> {
    let mut buffer = Vec::with_capacity(4096);
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
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid Home Assistant response",
        ));
    };

    let body_start = header_end + 4;
    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let status = headers.lines().next().unwrap_or_default().to_string();

    if let Some(content_length) = header_content_length(&headers) {
        let needed = body_start + content_length;
        while buffer.len() < needed {
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }
        if buffer.len() < needed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "truncated Home Assistant response body",
            ));
        }
        return Ok((
            status,
            String::from_utf8_lossy(&buffer[body_start..needed]).to_string(),
        ));
    }

    Ok((
        status,
        String::from_utf8_lossy(buffer.get(body_start..).unwrap_or_default()).to_string(),
    ))
}

fn header_content_length(headers: &str) -> Option<usize> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    })
}

fn read_optional_secret(path: Option<&PathBuf>) -> Option<String> {
    path.and_then(|path| fs::read_to_string(path).ok())
        .map(|value| value.trim().to_string())
}

fn is_unauthorized(status: &str) -> bool {
    status.contains(" 401 ")
}

fn form_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char)
            }
            b' ' => output.push('+'),
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
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
        0 => "自动",
        1..=33 => "低",
        34..=66 => "中",
        _ => "高",
    }
}

fn speed_from_fan_mode(mode: &str) -> u8 {
    match mode {
        "自动" => 0,
        "低" => 33,
        "中" => 66,
        "高" => 100,
        _ => 45,
    }
}

fn dashboard_devices_from_states(states: &[Value]) -> Vec<Value> {
    let mut devices = real_devices_from_states(states);
    append_cached_xiaomi_devices(&mut devices);
    devices
}

fn real_devices_from_states(states: &[Value]) -> Vec<Value> {
    let mut devices = Vec::new();

    if let Some(camera) = camera_device(
        states,
        "camera_mimi",
        "咪咪 小米智能摄像机2 云台版",
        CAMERA_MIMI_POWER,
        CAMERA_MIMI_STATUS,
        Some(CAMERA_MIMI_INDICATOR),
        None,
    ) {
        devices.push(camera);
    }

    if let Some(camera) = camera_device(
        states,
        "camera_wangwang",
        "汪汪 小米智能摄像机 云台版2K",
        CAMERA_WANGWANG_POWER,
        CAMERA_WANGWANG_STATUS,
        Some(CAMERA_WANGWANG_INDICATOR),
        Some(CAMERA_WANGWANG_STREAM_STATUS),
    ) {
        devices.push(camera);
    }

    if let Some(tv) = state_by_id(states, TV_ENTITY) {
        let volume = percent_from_volume(attr_f64(tv, "volume_level"));
        let source = attr_str(tv, "source").unwrap_or_else(|| state_str(tv).to_string());
        let muted = attr_bool(tv, "is_volume_muted").unwrap_or(false);
        devices.push(json!({
            "id": "real_tv",
            "name": "客厅的小米电视",
            "home": MAIN_HOME,
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
            "readonly": true
        }));
    }

    if let Some(speaker) = state_by_id(states, XIAOAI_ENTITY) {
        let volume = percent_from_volume(attr_f64(speaker, "volume_level"));
        let muted = attr_bool(speaker, "is_volume_muted").unwrap_or(false);
        devices.push(json!({
            "id": "xiaoai",
            "name": "Xiaomi 智能家庭屏 mini",
            "home": MAIN_HOME,
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
            "note": format!("音量 {}% · 可唤醒、播放音乐/广播、播报文本", volume),
            "online": entity_is_available(speaker),
            "readonly": true
        }));
    }

    if let Some(ac) = state_by_id(states, AC_ENTITY) {
        let temperature = attr_f64(ac, "temperature").unwrap_or(24.0).round() as i64;
        let fan_mode = attr_str(ac, "fan_mode").unwrap_or_else(|| "自动".to_string());
        let state = state_str(ac).to_string();
        devices.push(json!({
            "id": "real_ac",
            "name": "二楼主卧空调",
            "home": MAIN_HOME,
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
            "home": MAIN_HOME,
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
            "name": "米家智能温湿度计3 mini",
            "home": MAIN_HOME,
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

fn camera_device(
    states: &[Value],
    id: &str,
    name: &str,
    power_entity: &str,
    status_entity: &str,
    indicator_entity: Option<&str>,
    stream_status_entity: Option<&str>,
) -> Option<Value> {
    let power = state_by_id(states, power_entity)?;
    let status = state_by_id(states, status_entity)
        .map(state_str)
        .unwrap_or("unknown");
    let indicator = indicator_entity
        .and_then(|entity_id| state_by_id(states, entity_id))
        .map(state_str)
        .unwrap_or("unknown");
    let stream = stream_status_entity
        .and_then(|entity_id| state_by_id(states, entity_id))
        .map(state_str);
    let note = match stream {
        Some(stream) => format!("工作状态 {} · 流状态 {}", status, stream),
        None => format!("工作状态 {} · 指示灯 {}", status, indicator),
    };
    Some(json!({
        "id": id,
        "name": name,
        "home": CAMERA_HOME,
        "room": "客厅",
        "kind": "camera",
        "on": entity_is_on(power),
        "brightness": if entity_is_on(power) { 70 } else { 8 },
        "volume": 0,
        "temperature": 0,
        "humidity": 0,
        "position": 0,
        "color": "#64748b",
        "speed": 0,
        "mode": status,
        "note": note,
        "online": entity_is_available(power),
        "readonly": true,
        "indicator": indicator,
        "stream": stream.unwrap_or("未启用"),
        "stream_capable": matches!(id, "camera_wangwang" | "camera_mimi"),
        "stream_protocol": "rtc"
    }))
}

fn append_cached_xiaomi_devices(devices: &mut Vec<Value>) {
    let Ok(entries) = fs::read_dir(DEFAULT_XIAOMI_HOME_DEVICE_CACHE) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("dict") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let Some(Ok(value)) = serde_json::Deserializer::from_str(&text)
            .into_iter::<Value>()
            .next()
        else {
            continue;
        };
        let Some(map) = value.as_object() else {
            continue;
        };
        for device in map.values() {
            if device.get("model").and_then(Value::as_str) != Some("xiaomi.repeater.v3") {
                continue;
            }
            let id = format!(
                "xiaomi_cache_{}",
                device
                    .get("did")
                    .and_then(Value::as_str)
                    .unwrap_or("repeater")
            );
            if devices
                .iter()
                .any(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str()))
            {
                continue;
            }
            let online = device
                .get("online")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let name = device
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("小米 Wi-Fi 放大器");
            let room = device
                .get("room_name")
                .and_then(Value::as_str)
                .unwrap_or("未分配");
            devices.push(json!({
                "id": id,
                "name": name,
                "home": MAIN_HOME,
                "room": room,
                "kind": "network",
                "on": online,
                "brightness": if online { 72 } else { 10 },
                "volume": 0,
                "temperature": 0,
                "humidity": 0,
                "position": 0,
                "color": "#0f766e",
                "speed": 0,
                "mode": if online { "online" } else { "offline" },
                "note": "Xiaomi Home 在线设备 · 暂无可控实体",
                "online": online,
                "readonly": true
            }));
        }
    }
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

fn start_go2rtc_camera_stream(id: &str) -> std::io::Result<Value> {
    let (camera, src) = match id {
        "camera_wangwang" => ("camera_wangwang", "wangwang"),
        "camera_mimi" => ("camera_mimi", "mimi"),
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "unknown camera",
            ))
        }
    };
    let base_url = go2rtc_base_url();
    let info_url = format!("{base_url}/api/streams?src={src}");
    let info = fetch_json_url(&info_url)?;
    let has_producer = info
        .get("producers")
        .and_then(Value::as_array)
        .map(|producers| !producers.is_empty())
        .unwrap_or(false);
    if !has_producer {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "本地摄像头流还没有就绪，请确认 micam 容器正在运行",
        ));
    }
    let playback_url = format!("{base_url}/stream.html?src={src}");
    let rtsp_url = format!("rtsp://{}/{}", go2rtc_rtsp_host(), src);
    Ok(json!({
        "ok": true,
        "camera": camera,
        "protocol": "rtc",
        "stream_url": rtsp_url,
        "playback_url": playback_url
    }))
}

fn go2rtc_base_url() -> String {
    env::var("GO2RTC_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(detect_colima_go2rtc_base_url)
        .unwrap_or_else(|| DEFAULT_GO2RTC_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn detect_colima_go2rtc_base_url() -> Option<String> {
    for binary in ["/opt/homebrew/bin/colima", "colima"] {
        let Ok(output) = Command::new(binary).args(["status", "--json"]).output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let Ok(status) = serde_json::from_slice::<Value>(&output.stdout) else {
            continue;
        };
        if let Some(ip) = status
            .get("ip_address")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|ip| !ip.is_empty())
        {
            return Some(format!("http://{ip}:1984"));
        }
    }
    None
}

fn go2rtc_rtsp_host() -> String {
    let base = go2rtc_base_url();
    let host = base
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&base)
        .split('/')
        .next()
        .unwrap_or("192.168.3.8:1984");
    let hostname = host.split(':').next().unwrap_or(host);
    format!("{hostname}:8554")
}

fn fetch_json_url(url: &str) -> std::io::Result<Value> {
    let output = Command::new("curl")
        .args(["-sS", "--fail", "--max-time", "3", "--", url])
        .output()?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr)
            .trim()
            .lines()
            .last()
            .unwrap_or("request failed")
            .to_string();
        return Err(std::io::Error::new(std::io::ErrorKind::Other, error));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
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
        return match ha_client.dashboard_device_count() {
            Ok(count) => (
                "200 OK",
                "application/json; charset=utf-8",
                format!(r#"{{"ok":true,"devices":{count},"home_assistant":"connected"}}"#),
            ),
            Err(err) => (
                "200 OK",
                "application/json; charset=utf-8",
                format!(
                    r#"{{"ok":false,"devices":0,"home_assistant":"error","error":"{}"}}"#,
                    escape_json(&err.to_string())
                ),
            ),
        };
    }

    if method == "GET" && path == "/api/devices" {
        return match ha_client.dashboard_devices_json() {
            Ok(json) => ("200 OK", "application/json; charset=utf-8", json),
            Err(err) => (
                "502 Bad Gateway",
                "application/json; charset=utf-8",
                format!(
                    r#"{{"ok":false,"devices":[],"error":"{}"}}"#,
                    escape_json(&err.to_string())
                ),
            ),
        };
    }

    if method == "POST" {
        if let Some(rest) = path.strip_prefix("/api/cameras/") {
            let mut parts = rest.split('/');
            let id = parts.next().unwrap_or_default();
            let action = parts.next().unwrap_or_default();
            let params = parse_form(body);
            return match action {
                "stream" => match ha_client
                    .start_camera_stream(id, int_param(&params, "quality").unwrap_or(2))
                {
                    Ok(value) => (
                        "200 OK",
                        "application/json; charset=utf-8",
                        value.to_string(),
                    ),
                    Err(err) => (
                        "502 Bad Gateway",
                        "application/json; charset=utf-8",
                        format!(
                            r#"{{"ok":false,"error":"{}"}}"#,
                            escape_json(&err.to_string())
                        ),
                    ),
                },
                "stop" => match ha_client.stop_camera_stream(id) {
                    Ok(()) => (
                        "200 OK",
                        "application/json; charset=utf-8",
                        r#"{"ok":true}"#.to_string(),
                    ),
                    Err(err) => (
                        "502 Bad Gateway",
                        "application/json; charset=utf-8",
                        format!(
                            r#"{{"ok":false,"error":"{}"}}"#,
                            escape_json(&err.to_string())
                        ),
                    ),
                },
                _ => (
                    "404 Not Found",
                    "application/json; charset=utf-8",
                    r#"{"ok":false,"error":"unknown camera action"}"#.to_string(),
                ),
            };
        }

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
    write_bytes_response(stream, status, content_type, body.as_bytes())
}

fn write_bytes_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
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
        body.len(),
        ""
    );
    stream.write_all(response.as_bytes())?;
    stream.write_all(body)
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
    Vec::new()
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
	    .camera-stage {
	      display: grid;
	      gap: 10px;
	    }
	    .camera-player,
	    .camera-placeholder {
	      width: 100%;
	      aspect-ratio: 16 / 9;
	      border: 1px solid #d8e0ea;
	      border-radius: var(--radius);
	      background: #0f172a;
	      overflow: hidden;
	    }
	    .camera-player {
	      display: block;
	      object-fit: cover;
	    }
	    .camera-placeholder {
	      display: grid;
	      place-items: center;
	      padding: 12px;
	      color: #dbeafe;
	      font-size: 14px;
	      font-weight: 720;
	      text-align: center;
	    }
	    .camera-status {
	      min-height: 20px;
	      color: var(--muted);
	      font-size: 12px;
	      font-weight: 680;
	    }
	    .camera-status[data-tone="error"] {
	      color: #b42318;
	    }
	    .camera-actions {
	      display: grid;
	      grid-template-columns: repeat(3, minmax(0, 1fr));
	      gap: 8px;
	    }
	    .stream-link {
	      display: inline-flex;
	      align-items: center;
	      justify-content: center;
	      min-height: 48px;
	      border: 1px solid var(--line);
	      border-radius: var(--radius);
	      background: #fff;
	      color: #344054;
	      font-size: 14px;
	      font-weight: 740;
	      text-decoration: none;
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
	            <span class="scene-icon" aria-hidden="true">▣</span><span class="scene-copy"><strong>影院</strong><span>电视开启，窗帘半开</span></span>
	          </button>
	          <button class="scene-button" data-scene="night" style="--scene-color:var(--amber);--scene-border:#fcd34d;--scene-bg:#fffbeb">
	            <span class="scene-icon" aria-hidden="true">☾</span><span class="scene-copy"><strong>夜间</strong><span>电视关闭，窗帘收起</span></span>
	          </button>
	          <button class="scene-button" data-scene="curtain-half" style="--scene-color:var(--violet);--scene-border:#c4b5fd;--scene-bg:#f5f3ff">
	            <span class="scene-icon" aria-hidden="true">✦</span><span class="scene-copy"><strong>窗帘半开</strong><span>隔断帘移动到中段</span></span>
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
	    const activeCameraStreams = new Map();
	    const cameraStreamMessages = new Map();
	    const hlsPlayers = new Map();
	    let hlsLoader = null;

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

	    function deviceSymbol(device) {
	      if (device.kind === 'light') return '▱';
	      if (device.kind === 'fan') return '✺';
	      if (device.kind === 'tv') return '▭';
	      if (device.kind === 'climate') return '❄';
	      if (device.kind === 'speaker') return '◉';
	      if (device.kind === 'cover') return '▥';
	      if (device.kind === 'sensor') return '℃';
	      if (device.kind === 'camera') return '◌';
	      if (device.kind === 'network') return '⌁';
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
	      if (device.kind === 'camera') return '摄像头';
	      if (device.kind === 'network') return '网络';
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
	      return `<div class="control-row"><div class="control-label">${escapeHtml(label)}</div><input aria-label="${escapeHtml(device.name)}${escapeHtml(label)}" type="range" min="${min}" max="${max}" value="${safeValue}" data-action="${action}" data-id="${escapeHtml(device.id)}" data-suffix="${escapeHtml(suffix)}"><div class="control-value">${safeValue}${escapeHtml(suffix)}</div></div>`;
	    }

	    function colorControl(device) {
	      return `<div class="color-row"><div class="control-label">颜色</div><div class="control-value">${escapeHtml(device.color)}</div><input aria-label="${escapeHtml(device.name)}颜色" type="color" value="${escapeHtml(device.color)}" data-action="color" data-id="${escapeHtml(device.id)}"></div>`;
	    }

	    function tvControls(device) {
	      const volume = Number(device.volume || device.brightness || 0);
	      return `
	        <div class="control-row"><div class="control-label">音量</div><div></div><div class="control-value">${volume}%</div></div>
	        <div class="action-buttons" aria-label="${escapeHtml(device.name)}音量快捷控制">
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="volume_down">-</button>
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="volume_up">+</button>
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="play_pause">播放</button>
	        </div>
	        <div class="action-buttons" aria-label="${escapeHtml(device.name)}遥控快捷控制">
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="home">主页</button>
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="ok">确定</button>
	          <button class="mini-button" data-action="tv-action" data-id="${escapeHtml(device.id)}" data-command="back">返回</button>
	        </div>`;
	    }

	    function speakerControls(device) {
	      const volume = Number(device.volume || device.brightness || 0);
	      return `
	        <div class="control-row"><div class="control-label">音量</div><div></div><div class="control-value">${volume}%</div></div>
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

	    function cameraControls(device) {
	      const streamUrl = activeCameraStreams.get(device.id);
	      const streamMessage = cameraStreamMessages.get(device.id);
	      const statusLine = streamMessage
	        ? `<div class="camera-status" data-camera-status="${escapeHtml(device.id)}" data-tone="${escapeHtml(streamMessage.tone || 'info')}">${escapeHtml(streamMessage.text)}</div>`
	        : '';
	      const capable = Boolean(device.stream_capable);
	      if (!capable) {
	        return `
	          <div class="sensor-readout">
	            <div class="sensor-tile"><strong>${escapeHtml(device.mode || '未知')}</strong><span>工作状态</span></div>
	            <div class="sensor-tile"><strong>未配置</strong><span>直播状态</span></div>
	          </div>
	          <div class="camera-placeholder">本地直播未配置</div>`;
	      }
	      const player = streamUrl
	        ? (streamUrl.includes('/stream.html?')
	          ? `<iframe class="camera-player" data-camera-frame="${escapeHtml(device.id)}" src="${escapeHtml(streamUrl)}" allow="autoplay; fullscreen" loading="eager"></iframe>`
	          : `<video class="camera-player" data-camera-video="${escapeHtml(device.id)}" data-stream-url="${escapeHtml(streamUrl)}" controls autoplay muted playsinline></video>`)
	        : `<div class="camera-placeholder">${escapeHtml(streamMessage?.text || '轻触尝试拉取画面')}</div>`;
	      return `
	        <div class="sensor-readout">
	          <div class="sensor-tile"><strong>${escapeHtml(device.mode || '未知')}</strong><span>工作状态</span></div>
	          <div class="sensor-tile"><strong>${escapeHtml(device.stream || device.indicator || '未知')}</strong><span>${device.stream ? '流状态' : '指示灯'}</span></div>
	        </div>
	        <div class="camera-stage">
	          ${player}
	          ${statusLine}
	          <div class="camera-actions">
	            <button class="mini-button" data-action="camera-stream" data-id="${escapeHtml(device.id)}">尝试直播</button>
	            <button class="mini-button" data-action="camera-stop" data-id="${escapeHtml(device.id)}">停止</button>
	            ${streamUrl ? `<a class="stream-link" href="${escapeHtml(streamUrl)}" target="_blank" rel="noreferrer">画面源</a>` : `<button class="mini-button" disabled>待机</button>`}
	          </div>
	        </div>`;
	    }

	    function networkControls(device) {
	      return `
	        <div class="sensor-readout">
	          <div class="sensor-tile"><strong>${device.online ? '在线' : '离线'}</strong><span>连接状态</span></div>
	          <div class="sensor-tile"><strong>${escapeHtml(device.mode || '设备')}</strong><span>型号</span></div>
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
	      if (device.kind === 'camera') {
	        return cameraControls(device);
	      }
	      if (device.kind === 'network') {
	        return networkControls(device);
	      }
	      return `<div class="control-row"><div class="control-label">场景</div><div></div><div class="control-value">${device.on ? '执行中' : '待机'}</div></div>`;
	    }

	    function visualLevel(device) {
	      if (device.kind === 'fan') return device.speed;
	      if (device.kind === 'tv' || device.kind === 'speaker') return device.volume || device.brightness;
	      if (device.kind === 'climate') return device.on ? clamp(device.speed || 45, 18, 100) : 8;
	      if (device.kind === 'cover') return clamp(device.position || device.brightness || 0, 12, 100);
	      if (device.kind === 'sensor') return 70;
	      if (device.kind === 'camera') return device.on ? 70 : 12;
	      if (device.kind === 'network') return device.online ? 72 : 12;
	      if (device.kind === 'switch') return device.on ? 76 : 8;
	      return device.brightness;
	    }

	    function stateWord(device) {
	      if (device.kind === 'sensor') return Number(device.temperature || 0).toFixed(1) + '°C';
	      if (device.kind === 'camera') return escapeHtml(device.mode || (device.on ? '在线' : '关闭'));
	      if (device.kind === 'network') return device.online ? '在线' : '离线';
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
	                <div class="room">${escapeHtml(device.home ? device.home + ' / ' : '')}${escapeHtml(device.room)} · ${deviceTypeLabel(device)}</div>
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

	    function destroyCameraPlayer(id) {
	      const hls = hlsPlayers.get(id);
	      if (hls) {
	        hls.destroy();
	        hlsPlayers.delete(id);
	      }
	    }

	    function destroyCameraPlayers() {
	      hlsPlayers.forEach(hls => hls.destroy());
	      hlsPlayers.clear();
	    }

	    function renderDevices() {
	      destroyCameraPlayers();
	      summaryCards(currentDevices);
	      grid.innerHTML = currentDevices.map(deviceCard).join('');
	      hydrateCameraPlayers();
	    }

	    function updateCameraStatus(id, text, tone = 'info') {
	      cameraStreamMessages.set(id, { text, tone });
	      document.querySelectorAll('[data-camera-status]').forEach(element => {
	        if (element.dataset.cameraStatus === id) {
	          element.textContent = text;
	          element.dataset.tone = tone;
	        }
	      });
	    }

	    async function loadDevices() {
	      const response = await fetch('/api/devices', { cache: 'no-store' });
	      if (!response.ok) throw new Error('HTTP ' + response.status);
	      const json = await response.text();
	      if (json !== lastJson) {
	        const data = JSON.parse(json);
	        currentDevices = data.devices || [];
	        renderDevices();
	        lastJson = json;
	      }
	      statusEl.textContent = '已连接 ' + new Date().toLocaleTimeString();
	    }

	    function loadHlsLibrary() {
	      if (window.Hls) return Promise.resolve(window.Hls);
	      if (hlsLoader) return hlsLoader;
	      hlsLoader = new Promise((resolve, reject) => {
	        const script = document.createElement('script');
	        script.src = 'https://cdn.jsdelivr.net/npm/hls.js@1.5.18/dist/hls.min.js';
	        script.async = true;
	        script.onload = () => resolve(window.Hls);
	        script.onerror = () => {
	          hlsLoader = null;
	          reject(new Error('播放器加载失败'));
	        };
	        document.head.appendChild(script);
	      });
	      return hlsLoader;
	    }

	    async function attachCameraVideo(video) {
	      const streamUrl = video.dataset.streamUrl;
	      const cameraId = video.dataset.cameraVideo;
	      if (!streamUrl || video.dataset.ready === 'true') return;
	      video.dataset.ready = 'true';
	      updateCameraStatus(cameraId, '正在缓冲直播');
	      video.addEventListener('playing', () => updateCameraStatus(cameraId, '直播中'), { once: true });
	      video.addEventListener('error', () => updateCameraStatus(cameraId, '画面加载失败，画面源可能已断开', 'error'));
	      if (video.canPlayType('application/vnd.apple.mpegurl')) {
	        video.src = streamUrl;
	      } else {
	        const Hls = await loadHlsLibrary();
	        if (!Hls || !Hls.isSupported()) {
	          throw new Error('当前环境暂时不能播放这个画面源');
	        }
	        destroyCameraPlayer(cameraId);
	        const hls = new Hls({
	          lowLatencyMode: false,
	          manifestLoadingMaxRetry: 8,
	          manifestLoadingRetryDelay: 700,
	          levelLoadingMaxRetry: 8,
	          fragLoadingMaxRetry: 8,
	          fragLoadingRetryDelay: 700
	        });
	        hls.on(Hls.Events.ERROR, (_event, data) => {
	          if (data?.fatal) {
	            updateCameraStatus(cameraId, '画面源连接失败，已停止播放器', 'error');
	            hls.destroy();
	          }
	        });
	        hlsPlayers.set(cameraId, hls);
	        hls.loadSource(streamUrl);
	        hls.attachMedia(video);
	      }
	      video.play().catch(() => {});
	    }

	    function hydrateCameraPlayers() {
	      document.querySelectorAll('[data-camera-frame]').forEach(frame => {
	        updateCameraStatus(frame.dataset.cameraFrame, '直播中');
	      });
	      document.querySelectorAll('[data-camera-video]').forEach(video => {
	        attachCameraVideo(video).catch(error => {
	          updateCameraStatus(video.dataset.cameraVideo, error.message || '播放器启动失败', 'error');
	          showError(error);
	        });
	      });
	    }

	    async function startCameraStream(id) {
	      updateCameraStatus(id, '正在打开本地摄像头流');
	      renderDevices();
	      const body = new URLSearchParams({ quality: 2 });
	      const response = await fetch('/api/cameras/' + encodeURIComponent(id) + '/stream', {
	        method: 'POST',
	        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
	        body
	      });
	      const data = await response.json();
	      if (!response.ok || !data.ok) throw new Error(data.error || ('HTTP ' + response.status));
	      activeCameraStreams.set(id, data.playback_url || data.stream_url);
	      updateCameraStatus(id, data.protocol === 'rtc' ? '正在打开实时画面' : '正在等待画面分片');
	      lastJson = '';
	      await loadDevices();
	    }

	    async function stopCameraStream(id) {
	      const response = await fetch('/api/cameras/' + encodeURIComponent(id) + '/stop', {
	        method: 'POST',
	        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
	        body: ''
	      });
	      const data = await response.json();
	      if (!response.ok || !data.ok) throw new Error(data.error || ('HTTP ' + response.status));
	      destroyCameraPlayer(id);
	      activeCameraStreams.delete(id);
	      cameraStreamMessages.set(id, { text: '直播已停止', tone: 'info' });
	      renderDevices();
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

	    function updateRangePreview(target) {
	      const row = target.closest('.control-row');
	      const value = row ? row.querySelector('.control-value') : null;
	      const suffix = target.dataset.suffix || '%';
	      if (value) value.textContent = target.value + suffix;
	      if (target.dataset.action === 'position') {
	        const card = target.closest('.device-card');
	        const state = card ? card.querySelector('.state-word') : null;
	        if (state) state.textContent = target.value + suffix;
	      }
	    }

	    document.addEventListener('click', event => {
	      const target = event.target.closest('[data-action="toggle"]');
	      if (!target || target.disabled) return;
	      updateDevice(target.dataset.id, { on: target.dataset.on !== 'true' }).catch(showError);
	    });

	    document.addEventListener('click', event => {
	      const target = event.target.closest('[data-action="tv-action"],[data-action="speaker-action"],[data-action="cover-action"],[data-action="camera-stream"],[data-action="camera-stop"],[data-action="temp-step"],[data-action="mode"]');
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
	      if (target.dataset.action === 'camera-stream') {
	        const id = target.dataset.id;
	        startCameraStream(id).catch(error => {
	          updateCameraStatus(id, error.message || '直播启动失败', 'error');
	          showError(error);
	        });
	      }
	      if (target.dataset.action === 'camera-stop') {
	        stopCameraStream(target.dataset.id).catch(showError);
	      }
	      if (target.dataset.action === 'temp-step') {
	        const temperature = Number(target.dataset.current || 24) + Number(target.dataset.delta || 0);
	        updateDevice(target.dataset.id, { temperature: clamp(temperature, 16, 30), on: true, note: '网页快捷操作：空调温度' }).catch(showError);
	      }
	      if (target.dataset.action === 'mode') {
	        updateDevice(target.dataset.id, { mode: target.dataset.mode, on: true, note: '网页快捷操作：空调模式' }).catch(showError);
	      }
	    });

	    document.addEventListener('input', event => {
	      const target = event.target;
	      if (!target.dataset.action || target.type !== 'range') return;
	      updateRangePreview(target);
	    });

	    document.addEventListener('change', event => {
	      const target = event.target;
	      if (!target.dataset.action) return;
	      if (target.type === 'range') updateRangePreview(target);
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
	      const tv = hasDevice('real_tv') ? 'real_tv' : '';
	      const ac = hasDevice('real_ac') ? 'real_ac' : '';
	      const curtain = hasDevice('curtain') ? 'curtain' : '';
	      const scenes = {
	        'all-on': [
	          [tv, { on: true, volume: 35, note: '网页快捷操作：全开' }],
	          [ac, { on: true, temperature: 24, speed: 50, mode: 'cool', note: '网页快捷操作：全开' }],
	          [curtain, { action: 'open' }]
	        ],
	        'all-off': [
	          [tv, { on: false, volume: 0, note: '网页快捷操作：全关' }],
	          [ac, { on: false, temperature: 24, speed: 0, mode: 'cool', note: '网页快捷操作：全关' }],
	          [curtain, { action: 'close' }]
	        ],
	        'movie': [
	          [tv, { on: true, volume: 32, note: '网页快捷操作：影院模式' }],
	          [ac, { on: true, temperature: 25, speed: 25, mode: 'cool', note: '网页快捷操作：影院模式' }],
	          [curtain, { position: 18 }]
	        ],
	        'night': [
	          [tv, { on: false, volume: 0, note: '网页快捷操作：夜间模式' }],
	          [ac, { on: true, temperature: 26, speed: 20, mode: 'cool', note: '网页快捷操作：夜间模式' }],
	          [curtain, { position: 0 }]
	        ],
	        'curtain-half': [
	          [curtain, { position: 50 }]
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
