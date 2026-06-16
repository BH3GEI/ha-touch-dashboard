# Deployment Notes

These notes describe the current Mac mini deployment shape and the commands
needed to recreate it.

## Current Local Paths

```text
/Users/mac/repos/ha-touch-dashboard        # this repo
/Users/mac/HomeAssistantCore               # Home Assistant Core venv/config
/Users/mac/HomeAssistantCore/config        # HA config directory
```

## Install Home Assistant Core

Home Assistant Core 2026.6.x requires a recent Python. On this machine the
working path used Homebrew Python 3.14.

```bash
brew install python@3.14
mkdir -p /Users/mac/HomeAssistantCore
/opt/homebrew/bin/python3.14 -m venv /Users/mac/HomeAssistantCore/venv
/Users/mac/HomeAssistantCore/venv/bin/python -m pip install --upgrade pip
/Users/mac/HomeAssistantCore/venv/bin/python -m pip install homeassistant
```

If optional frontend/runtime packages are missing after install, install the
exact packages requested by the Home Assistant logs. On this machine the setup
needed:

```bash
/Users/mac/HomeAssistantCore/venv/bin/python -m pip install \
  home-assistant-frontend \
  PyQRCode \
  aioesphomeapi
brew install jpeg-turbo
```

## Configure Home Assistant

Create or update:

```text
/Users/mac/HomeAssistantCore/config/configuration.yaml
```

Use `ha/configuration.example.yaml` as the minimal config. It enables packages,
API, webhook, frontend, onboarding, and the services needed by this bridge.

Install the Mijia package:

```bash
mkdir -p /Users/mac/HomeAssistantCore/config/packages
cp ha/virtual_mijia.yaml \
  /Users/mac/HomeAssistantCore/config/packages/virtual_mijia.yaml
```

Validate:

```bash
/Users/mac/HomeAssistantCore/venv/bin/hass \
  --script check_config \
  -c /Users/mac/HomeAssistantCore/config
```

## Connect Xiaomi Home

Install the official `xiaomi_home` custom integration into:

```text
/Users/mac/HomeAssistantCore/config/custom_components/xiaomi_home
```

Complete the Xiaomi OAuth flow from Home Assistant, select the desired homes,
and confirm these entities exist before using the real-device dashboard:

```text
media_player.xiaomi_cn_mitv_c1640dcb988dac758708dcc723857a86_1ba845686779440d9ba27899df3c7997_v1
media_player.xiaomi_cn_2037162573_x4b
climate.lumi_cn_974076238_mcn02
cover.czmydz_cn_2143837107_mym1_s_2_curtain
sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_temperature_p_2_1001
sensor.xiaomi_cn_blt_3_1onep2uro4c03_mini_relative_humidity_p_2_1002
```

## Build The Rust Bridge

```bash
cargo build --release
cp state.example.tsv state.tsv
```

Manual run:

```bash
./target/release/ha-virtual-mijia-bridge \
  --addr 0.0.0.0:8787 \
  --state /Users/mac/repos/ha-touch-dashboard/state.tsv \
  --ha-token-file /Users/mac/HomeAssistantCore/HA-OWNER-ACCESS-TOKEN.txt
```

## Camera Stream Stack

The camera live view uses the local micam stack instead of Home Assistant's
native camera frontend. Runtime files live outside the repo:

```text
/Users/mac/HomeAssistantBridge/micam
```

Colima must run in bridged mode so the Mac browser can reach go2rtc inside the
Linux VM:

```bash
colima status --json
```

Copy the committed template once, then keep the real `.env` private:

```bash
mkdir -p /Users/mac/HomeAssistantBridge/micam
cp deploy/micam/docker-compose.yml deploy/micam/miloco-http-start.py \
  /Users/mac/HomeAssistantBridge/micam/
cp deploy/micam/.env.example /Users/mac/HomeAssistantBridge/micam/.env
mkdir -p /Users/mac/HomeAssistantBridge/micam/go2rtc
```

After Miloco has a Xiaomi login and admin password hash in the private `.env`,
start both camera stream profiles:

```bash
cd /Users/mac/HomeAssistantBridge/micam
docker compose --env-file .env --profile streaming --profile mimi up -d
```

The Rust bridge reads `GO2RTC_BASE_URL` when set. If it is unset, it tries to
detect the current bridged Colima IP from `colima status --json`, then falls
back to the checked-in default.

## Launchd Services

Copy the examples:

```bash
cp launchd/com.local.homeassistant-core.plist.example \
  /Users/mac/Library/LaunchAgents/com.local.homeassistant-core.plist

cp launchd/com.local.ha-touch-dashboard.plist.example \
  /Users/mac/Library/LaunchAgents/com.local.ha-virtual-mijia-bridge.plist
```

Load or reload:

```bash
uid=$(id -u)
launchctl bootstrap gui/$uid /Users/mac/Library/LaunchAgents/com.local.homeassistant-core.plist
launchctl bootstrap gui/$uid /Users/mac/Library/LaunchAgents/com.local.ha-virtual-mijia-bridge.plist
```

After editing a plist, reload just that LaunchAgent:

```bash
uid=$(id -u)
launchctl bootout gui/$uid /Users/mac/Library/LaunchAgents/com.local.ha-virtual-mijia-bridge.plist 2>/dev/null || true
launchctl bootstrap gui/$uid /Users/mac/Library/LaunchAgents/com.local.ha-virtual-mijia-bridge.plist
```

## URLs

```text
Dashboard:      http://127.0.0.1:8787/
LAN dashboard:  http://192.168.3.37:8787/
Health API:     http://127.0.0.1:8787/api/health
Home Assistant: http://127.0.0.1:8123/
go2rtc:         http://<colima-ip>:1984/
Miloco:         http://<colima-ip>:8000/
HomeKit bridge: TCP 51827
HomeKit TV accessory: TCP 51828
```

## Current Known Warning

The current Mac mini logs may show a zeroconf/mDNS warning:

```text
No route to host
```

HomeKit Bridge listens on TCP `51827`, and the TV accessory listens on TCP
`51828`. Apple Home pairing should be tested from an iPhone on the same LAN
because mDNS is part of discovery.
