# Architecture

This repo is more than the dashboard shell. It contains the local Rust bridge,
the Home Assistant package, launchd templates, and reproducible setup notes.

## Runtime Pieces

```text
Touch dashboard browser
  -> Rust bridge HTTP API on :8787
    -> state.tsv persisted device state
    -> Home Assistant webhook on :8123
      -> input helpers and template entities
      -> HomeKit Bridge on :51827
        -> Apple Home / Siri
```

## Rust Bridge

Source: `src/main.rs`

Responsibilities:

- Serve the touch-first dashboard at `/`.
- Serve `GET /api/health`.
- Serve `GET /api/devices`.
- Accept `POST /api/devices/<id>` form updates.
- Persist state to `state.tsv`.
- POST each device update back into Home Assistant through:

```text
/api/webhook/virtual_mijia_bridge_state_b53b516a99ba5cf173601fd8ff7298e0
```

The implementation intentionally has no third-party Rust dependencies. This
keeps the bridge small, fast to build, and easy to audit on a Mac mini.

## Home Assistant Package

Source: `ha/virtual_mijia.yaml`

It defines:

- `rest_command.virtual_mijia_update` for HA -> Rust updates.
- `input_boolean`, `input_number`, and `input_select` helpers.
- Template entities:
  - `light.virtual_mijia_desk_lamp`
  - `light.virtual_mijia_lightstrip`
  - `fan.virtual_mijia_air_purifier`
  - `switch.virtual_mijia_xiaoai_scene`
- Automations for HA -> Rust sync.
- A webhook automation for Rust -> HA sync.
- `input_boolean.virtual_mijia_sync_guard` to prevent sync loops.
- HomeKit Bridge filter for exposing only these virtual entities.

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
