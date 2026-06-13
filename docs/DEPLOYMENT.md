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

Install the virtual Mijia package:

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

## Build The Rust Bridge

```bash
cargo build --release
cp state.example.tsv state.tsv
```

Manual run:

```bash
./target/release/ha-virtual-mijia-bridge \
  --addr 0.0.0.0:8787 \
  --state /Users/mac/repos/ha-touch-dashboard/state.tsv
```

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
HomeKit bridge: TCP 51827
```

## Current Known Warning

The current Mac mini logs may show a zeroconf/mDNS warning:

```text
No route to host
```

HomeKit Bridge still listens on TCP `51827`, but Apple Home pairing should be
tested from an iPhone on the same LAN because mDNS is part of discovery.
