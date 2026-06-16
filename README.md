# HA Touch Dashboard

Touch-first Rust dashboard for local Home Assistant, Xiaomi Home, HomeKit, and
Siri control. It exposes a web control panel, a small HTTP API, and direct
Home Assistant service calls for real Xiaomi entities.

- Web panel: `http://127.0.0.1:8787`
- LAN panel: `http://192.168.3.37:8787`
- Health API: `http://127.0.0.1:8787/api/health`
- Devices API: `http://127.0.0.1:8787/api/devices`

## What It Controls

- 客厅的小米电视
- 小爱家庭屏 mini
- 二楼主卧空调
- 隔断帘
- 米家温湿度计
- 两台小米摄像机，通过本地 Miloco + micam + go2rtc 链路在 Dashboard 内播放
- 客厅小米 Wi-Fi 放大器 Pro 只读状态卡

The dashboard is optimized for touch screens: large scene buttons, large power
targets, thick sliders, clear state words, and a mobile bottom action dock.

## What's Included

- Rust HTTP server and embedded touch dashboard: `src/main.rs`
- Home Assistant package: `ha/virtual_mijia.yaml`
- Minimal Home Assistant config example: `ha/configuration.example.yaml`
- LaunchAgent templates for the Rust bridge and Home Assistant Core: `launchd/`
- Seed device state: `state.example.tsv`
- Deployment and validation docs: `docs/`

## What's Not Included

- Home Assistant Core virtual environment.
- Live `state.tsv`.
- Build outputs under `target/`.
- launchd logs and screenshots.
- Any GitHub, Home Assistant, or Apple credentials.

## Build

```bash
cargo build --release
```

## Run

```bash
cp state.example.tsv state.tsv
./target/release/ha-virtual-mijia-bridge \
  --addr 0.0.0.0:8787 \
  --state ./state.tsv \
  --ha-token-file /Users/mac/HomeAssistantCore/HA-OWNER-ACCESS-TOKEN.txt
```

## Home Assistant

Install `ha/virtual_mijia.yaml` into:

```text
/Users/mac/HomeAssistantCore/config/packages/virtual_mijia.yaml
```

The package defines HomeKit bridge filters and Siri-ready scripts for TV volume
and XiaoAI actions. It does not create placeholder devices.

Camera live view runs through the local micam stack in `deploy/micam/`.
Miloco handles Xiaomi account/device metadata, micam pulls each camera stream
inside the bridged Colima VM, and go2rtc exposes browser-playable RTC pages
that the Rust dashboard embeds after the user presses "尝试直播".

For the full setup path, see `docs/DEPLOYMENT.md`.

## Launchd

The example plist in `launchd/` points at this repository path. Copy it to:

```text
/Users/mac/Library/LaunchAgents/com.local.ha-virtual-mijia-bridge.plist
```

Then reload or kickstart that LaunchAgent.
