# Architecture

This repo is more than the dashboard shell. It contains the local Rust bridge,
the Home Assistant package, launchd templates, and reproducible setup notes.

## Runtime Pieces

```text
Touch dashboard browser
  -> Rust bridge HTTP API on :8787
    -> Home Assistant REST API on :8123
      -> Xiaomi Home entities
      -> real TV / XiaoAI / AC / curtain / sensors
    -> state.tsv persisted fallback device state
    -> Home Assistant webhook on :8123 for fallback template sync
    -> HomeKit Bridge on :51827 and TV accessory on :51828
      -> Apple Home / Siri
```

## Rust Bridge

Source: `src/main.rs`

Responsibilities:

- Serve the touch-first dashboard at `/`.
- Serve `GET /api/health`.
- Serve `GET /api/devices`.
- Accept `POST /api/devices/<id>` form updates.
- When a Home Assistant token is available, read real Xiaomi entity states from
  `/api/states` and call HA services for controls.
- Persist fallback state to `state.tsv`.
- POST fallback device updates back into Home Assistant through:

```text
/api/webhook/virtual_mijia_bridge_state_b53b516a99ba5cf173601fd8ff7298e0
```

The implementation intentionally has no third-party Rust dependencies. This
keeps the bridge small, fast to build, and easy to audit on a Mac mini.

## Home Assistant Package

Source: `ha/virtual_mijia.yaml`

It defines:

- `rest_command.virtual_mijia_update` for fallback HA -> Rust updates.
- `input_boolean`, `input_number`, and `input_select` fallback helpers.
- Template entities:
  - `light.virtual_mijia_desk_lamp`
  - `fan.virtual_mijia_air_purifier`
  - `switch.virtual_mijia_tv`
  - `switch.virtual_mijia_ac_companion`
  - `switch.virtual_mijia_xiaoai_scene`
- Automations for HA -> Rust sync.
- A webhook automation for Rust -> HA sync.
- `input_boolean.virtual_mijia_sync_guard` to prevent sync loops.
- Scripts for Siri/HomeKit actions:
  - `script.mijia_tv_volume_up`
  - `script.mijia_tv_volume_down`
  - `script.mijia_xiaoai_wake`
  - `script.mijia_xiaoai_play_music`
  - `script.mijia_xiaoai_broadcast`
- HomeKit Bridge filter for real AC, curtain, temperature/humidity sensors, and
  Siri scripts on TCP `51827`.
- A separate TV HomeKit accessory on TCP `51828` because HomeKit requires
  television media players to run in accessory mode.

## Dashboard

The dashboard is embedded in `src/main.rs` so the Rust bridge can ship as one
binary. It is optimized for touch:

- Large scene controls.
- Large power buttons.
- Thick range sliders.
- Mobile bottom dock.
- No dependency on Home Assistant's web UI for daily operation.

## Runtime Files Not Committed

These are intentionally ignored:

- `target/` build outputs.
- `state.tsv` live device state.
- launchd logs.
- screenshots and temporary QA artifacts.

Use `state.example.tsv` as the seed state.
