# micam local bridge

This directory contains the reproducible, commit-safe template for running
Miloco, go2rtc, and micam from this Mac with Colima.

Runtime state lives outside the repository:

```text
/Users/mac/HomeAssistantBridge/micam
```

The real `.env` file is not committed. It contains the Miloco admin password
hash and camera device IDs.

## Why this differs from upstream

The upstream one-click script is written for Linux hosts and uses
`network_mode: host`. On macOS, Docker runs inside a Linux VM. This template
keeps host networking, but runs Colima in bridged mode so go2rtc and Miloco are
reachable from the Mac and other LAN clients through the Colima IP:

- Miloco WebUI: `http://<colima-ip>:8000`
- go2rtc WebUI: `http://<colima-ip>:1984`
- RTSP: `rtsp://<colima-ip>:8554/wangwang`
- RTSP: `rtsp://<colima-ip>:8554/mimi`

Find the current Colima IP with:

```bash
colima status --json | jq -r .ip_address
```

Only `miloco` and `go2rtc` start by default. Start camera stream containers
after Miloco has an admin password and the Xiaomi account is bound:

```bash
docker compose --profile streaming --profile mimi up -d
```

You can also start one camera profile at a time:

```bash
docker compose --profile streaming up -d
docker compose --profile mimi up -d
```

## Validation

```bash
docker compose ps
ip=$(colima status --json | jq -r .ip_address)
curl -I "http://$ip:8000"
curl -sS "http://$ip:1984/api/streams"
ffprobe -rtsp_transport tcp "rtsp://$ip:8554/wangwang"
```
