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
```

Expected bridge health:

```json
{"ok":true,"devices":5,"home_assistant":"configured"}
```

## Dashboard Interaction

Test at least:

- Desktop viewport loads the dashboard with 5 device cards.
- Mobile viewport has no horizontal overflow.
- The visible cards are the real Xiaomi devices: TV, XiaoAI, AC, curtain, and
  temperature/humidity sensor.
- TV and XiaoAI volume sliders submit on `change`.
- AC temperature, mode, and fan controls render.
- Curtain position slider and open/stop/close buttons render.
- The temperature/humidity sensor is read-only.
- Browser console has no relevant errors.

Avoid triggering XiaoAI broadcast or physical device changes during a passive
smoke test unless that side effect is intentional.

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

After Home Assistant onboarding and Xiaomi OAuth are complete, confirm the Rust
API returns real Xiaomi devices:

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
