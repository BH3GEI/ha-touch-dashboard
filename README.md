# HA Touch Dashboard

Touch-first Rust dashboard for local Home Assistant, Xiaomi Home, HomeKit, and
Siri control. It exposes a web control panel, a small HTTP API, direct
Home Assistant service calls for real Xiaomi entities, and a local fallback
state path.

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

If the Home Assistant token is unavailable, the dashboard falls back to the
local `state.tsv` demo devices instead of rendering a blank page.

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

The package defines the local fallback template entities, HA -> Rust REST
commands, Rust -> HA webhook sync guard, HomeKit bridge filters, and Siri-ready
scripts for TV volume and XiaoAI actions.

For the full setup path, see `docs/DEPLOYMENT.md`.

## Launchd

The example plist in `launchd/` points at this repository path. Copy it to:

```text
/Users/mac/Library/LaunchAgents/com.local.ha-virtual-mijia-bridge.plist
```

Then reload or kickstart that LaunchAgent.
