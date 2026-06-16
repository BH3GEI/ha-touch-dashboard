# Validation Checklist

Run these checks before claiming the system works.

## Build And Config

```bash
cargo build --release
/Users/mac/HomeAssistantCore/venv/bin/hass \
  --script check_config \
  -c /Users/mac/HomeAssistantCore/config
PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm install
BASE_URL=http://127.0.0.1:8787 npx playwright test
cd deploy/micam && docker compose --env-file .env.example config --quiet
```

## Service Health

```bash
curl -sS http://127.0.0.1:8787/api/health
curl -sS http://127.0.0.1:8787/api/devices
curl -sS -o /dev/null -D - http://127.0.0.1:8123/
lsof -nP -iTCP:8787 -sTCP:LISTEN
lsof -nP -iTCP:8123 -sTCP:LISTEN
lsof -nP -iTCP:51827 -sTCP:LISTEN
lsof -nP -iTCP:51828 -sTCP:LISTEN
colima status --json
docker --context colima compose \
  --env-file /Users/mac/HomeAssistantBridge/micam/.env \
  -f /Users/mac/HomeAssistantBridge/micam/docker-compose.yml ps
```

Expected bridge health:

```json
{"ok":true,"devices":8,"home_assistant":"connected"}
```

## Dashboard Interaction

Test at least:

- Desktop viewport loads the dashboard with 8 device cards.
- Mobile viewport has no horizontal overflow.
- The visible cards are the real Xiaomi devices: two cameras, TV, XiaoAI, AC,
  curtain, temperature/humidity sensor, and Wi-Fi repeater.
- TV remote buttons and XiaoAI action buttons submit real Home Assistant
  service calls.
- AC temperature, mode, and fan controls render.
- Curtain position slider and open/stop/close buttons render.
- Camera live controls render without auto-starting a stream.
- The temperature/humidity sensor is read-only.
- Browser console has no relevant errors.

Avoid triggering XiaoAI broadcast or physical device changes during a passive
smoke test unless that side effect is intentional.

## Real Device State

The Rust bridge should return Xiaomi Home devices from Home Assistant. It should
not return placeholder devices when HA auth fails.

```bash
curl -sS http://127.0.0.1:8787/api/devices
```

Expected names include:

- `咪咪 小米智能摄像机2 云台版`
- `汪汪 小米智能摄像机 云台版2K`
- `客厅的小米电视`
- `Xiaomi 智能家庭屏 mini`
- `二楼主卧空调`
- `隔断帘`
- `米家温湿度计`
- `客厅小米Wi-Fi放大器Pro`

## Camera Stream

Passive smoke tests must not auto-start a camera stream. To test live view
intentionally:

```bash
curl -sS -X POST http://127.0.0.1:8787/api/cameras/camera_wangwang/stream \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'quality=2'

curl -sS -X POST http://127.0.0.1:8787/api/cameras/camera_mimi/stream \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'quality=2'
```

Expected: JSON contains `ok: true`, `protocol: rtc`, an RTSP `stream_url`, and
a go2rtc `playback_url` like `http://<colima-ip>:1984/stream.html?src=wangwang`
or `...src=mimi`.

Validate go2rtc directly:

```bash
curl -sS "http://$(colima status --json | jq -r .ip_address):1984/api/streams?src=wangwang"
curl -sS "http://$(colima status --json | jq -r .ip_address):1984/api/streams?src=mimi"
ffprobe -rtsp_transport tcp "rtsp://$(colima status --json | jq -r .ip_address):8554/wangwang"
```

The dashboard should create an iframe for each camera only after pressing
"尝试直播", show `直播中`, and avoid a blank video element.

## Space Check

This project is intentionally small. `target/`, logs, and live state are ignored
by Git.

```bash
du -sh /Users/mac/repos/ha-touch-dashboard
df -h /
```
