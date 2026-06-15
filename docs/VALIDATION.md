# Validation Checklist

Run these checks before claiming the system works.

## Build And Config

```bash
cargo build --release
/Users/mac/HomeAssistantCore/venv/bin/hass \
  --script check_config \
  -c /Users/mac/HomeAssistantCore/config
```

## Service Health

```bash
curl -sS http://127.0.0.1:8787/api/health
curl -sS http://127.0.0.1:8787/api/devices
curl -sS -o /dev/null -D - http://127.0.0.1:8123/
lsof -nP -iTCP:8787 -sTCP:LISTEN
lsof -nP -iTCP:8123 -sTCP:LISTEN
lsof -nP -iTCP:51827 -sTCP:LISTEN
```

Expected bridge health:

```json
{"ok":true,"devices":5}
```

## Dashboard Interaction

Test at least:

- Desktop viewport loads the dashboard with 5 device cards.
- Mobile viewport has no horizontal overflow.
- Scene button updates all relevant devices.
- Power button toggles a device.
- Light brightness slider submits on `change`.
- Fan speed slider turns the fan on when speed is greater than zero.
- Browser console has no relevant errors.

## Rust -> HA Sync

The Rust bridge posts updates into HA through the local webhook. If onboarding is
not complete and a long-lived access token is unavailable, verify sync through
the recorder database:

```bash
sqlite3 -header -column /Users/mac/HomeAssistantCore/config/home-assistant_v2.db \
"with latest as (
  select sm.entity_id, s.state, s.last_updated_ts,
         row_number() over (partition by sm.entity_id order by s.state_id desc) rn
  from states s
  join states_meta sm on sm.metadata_id=s.metadata_id
  where sm.entity_id like 'input_%virtual_mijia%'
)
select entity_id, state, datetime(last_updated_ts,'unixepoch','localtime') updated
from latest
where rn=1
order by entity_id;"
```

The sync guard should end as:

```text
input_boolean.virtual_mijia_sync_guard off
```

## HA -> Rust Sync

After Home Assistant onboarding is complete, use HA UI or API to change one of
the template entities/helpers, then confirm the Rust API state changes:

```bash
curl -sS http://127.0.0.1:8787/api/devices
```

## Space Check

This project is intentionally small. `target/`, logs, and live state are ignored
by Git.

```bash
du -sh /Users/mac/repos/ha-touch-dashboard
df -h /
```
