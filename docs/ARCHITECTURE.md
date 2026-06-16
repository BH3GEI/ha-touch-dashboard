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
      -> Xiaomi camera device state
    -> Xiaomi Home local device cache for read-only devices without entities
    -> go2rtc RTC camera pages from the local micam stack
    -> explicit error if Home Assistant is unavailable
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
- Accept `POST /api/cameras/<id>/stream` and `/stop` for camera live-view
  control.
- When a Home Assistant token is available, read real Xiaomi entity states from
  `/api/states` and call HA services for controls.
- Read Xiaomi Home's local device cache for devices that exist in Xiaomi Home
  but expose no Home Assistant entity, such as the Wi-Fi repeater.
- Keep `state.tsv` only for old local debugging endpoints.
- Never show local debug devices on the main dashboard when Home Assistant is
  unavailable.

The implementation intentionally has no third-party Rust dependencies. This
keeps the bridge small, fast to build, and easy to audit on a Mac mini.

## Home Assistant Package

Source: `ha/virtual_mijia.yaml`

It defines:

- Siri/HomeKit scripts for one-shot Xiaomi actions.
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

## Camera Live View

The Xiaomi Home integration exposes the cameras as MIoT action entities, not
standard Home Assistant `camera.*` entities. The dashboard therefore uses a
separate local stream bridge:

- Miloco completes Xiaomi OAuth and provides local camera metadata from both
  selected homes.
- micam runs in the bridged Colima VM and connects to each camera with Xiaomi's
  local/P2P protocol.
- go2rtc receives two RTSP producers, `wangwang` and `mimi`, and exposes
  browser-playable RTC pages at `/stream.html?src=...`.
- The Rust bridge maps `camera_wangwang` and `camera_mimi` to those go2rtc
  pages from `POST /api/cameras/<id>/stream`.

`GO2RTC_BASE_URL` can override the go2rtc address. When it is not set, the Rust
bridge tries to read the current Colima bridged IP from `colima status --json`
and falls back to the checked-in default.

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
