# HA Touch Dashboard

Touch-first Rust dashboard for local Home Assistant / HomeKit virtual Mijia
testing. It exposes a web control panel, a small HTTP API, and a webhook sync
path back into Home Assistant.

- Web panel: `http://127.0.0.1:8787`
- LAN panel: `http://192.168.3.37:8787`
- Health API: `http://127.0.0.1:8787/api/health`
- Devices API: `http://127.0.0.1:8787/api/devices`

## What It Controls

- 虚拟米家台灯
- 虚拟米家灯带
- 虚拟米家空气净化器
- 虚拟小爱音箱场景

The dashboard is optimized for touch screens: large scene buttons, large power
targets, thick sliders, clear state words, and a mobile bottom action dock.

## What's Included

- Rust HTTP server and embedded touch dashboard: `src/main.rs`
- Home Assistant virtual device package: `ha/virtual_mijia.yaml`
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
  --state ./state.tsv
```

## Home Assistant

Install `ha/virtual_mijia.yaml` into:

```text
/Users/mac/HomeAssistantCore/config/packages/virtual_mijia.yaml
```

The package defines the template entities, HomeKit bridge filter, HA -> Rust
REST commands, and Rust -> HA webhook sync guard.

For the full setup path, see `docs/DEPLOYMENT.md`.

## Launchd

The example plist in `launchd/` points at this repository path. Copy it to:

```text
/Users/mac/Library/LaunchAgents/com.local.ha-virtual-mijia-bridge.plist
```

Then reload or kickstart that LaunchAgent.
