# velux2mqtt

`velux2mqtt` is an asynchronous Rust client and MQTT bridge for the local VELUX KLF 200 API. It
discovers gateway nodes and groups, publishes retained cover state and metadata, accepts basic
cover/group commands and can publish Home Assistant MQTT discovery records.

## Requirements

- A VELUX KLF 200
- The KLF WLAN password
- An MQTT v5 broker.

The replacement KLF 150 does not expose the same local API and is not supported.

## Build

Use the repository development environment:

```bash
just build
just check
```

For local configuration, create an untracked `.env` file:

```dotenv
V2M_KLF_HOST=192.0.2.10
V2M_KLF_PASSWORD=replace-with-wlan-password
V2M_MQTT_ADDRESS=192.0.2.20
V2M_MQTT_USER=velux2mqtt
V2M_MQTT_PASS=replace-with-mqtt-password
```

Then run:

```bash
just run -v
```

## Configuration

| CLI option | Environment variable | Default |
|---|---|---|
| `--klf-host` | `V2M_KLF_HOST` | required |
| `--klf-port` | `V2M_KLF_PORT` | `51200` |
| `--klf-password` | `V2M_KLF_PASSWORD` | required |
| `--klf-heartbeat-interval` | `V2M_KLF_HEARTBEAT_INTERVAL` | `30` seconds |
| `--status-refresh-interval` | `V2M_STATUS_REFRESH_INTERVAL` | `300` seconds |
| `--certificate` | `V2M_KLF_CERTIFICATE` | disabled verification with warning |
| `--klf-reboot-on-shutdown` | `V2M_KLF_REBOOT_ON_SHUTDOWN` | `false` |
| `--klf-reboot-reconnect-delay` | `V2M_KLF_REBOOT_RECONNECT_DELAY` | `30` seconds |
| `--mqtt-addr` | `V2M_MQTT_ADDRESS` | required |
| `--mqtt-port` | `V2M_MQTT_PORT` | `1883` |
| `--mqtt-user` | `V2M_MQTT_USER` | empty |
| `--mqtt-pass` | `V2M_MQTT_PASS` | empty |
| `--mqtt-client-id` | `V2M_CLIENT_ID` | `velux2mqtt` |
| `--mqtt-base-topic` | `V2M_MQTT_BASE_TOPIC` | `velux` |
| `--hass-discovery` | `V2M_HASS_DISCOVERY` | `false` |

### Shutdown reboot

Some KLF 200 firmware becomes unable to finish a later TLS handshake when a connection closes after
house monitoring was enabled. Normal shutdown first disables monitoring and confirms the request.
Set `V2M_KLF_REBOOT_ON_SHUTDOWN=true` to additionally send `GW_REBOOT_REQ`:

```bash
V2M_KLF_REBOOT_ON_SHUTDOWN=true just run -v
```

The option is disabled by default because rebooting interrupts the complete gateway. It is applied
only to intentional process shutdown, never to a failed connection attempt or ordinary reconnect.
If the gateway is already stuck during TLS negotiation, stop all API clients and power-cycle it;
the reboot command cannot be sent until TLS is working.

## TLS certificate

KLF certificates are self-signed and may be expired. Without `--certificate`, the service accepts
the gateway certificate without verification and logs a warning. This is vulnerable to a local
machine-in-the-middle attack.

Extract and pin the certificate for a specific gateway:

```bash
openssl s_client -showcerts -connect 192.0.2.10:51200 </dev/null 2>/dev/null \
  | sed -n '/BEGIN CERTIFICATE/,/END CERTIFICATE/p' > klf200.pem
export V2M_KLF_CERTIFICATE="$PWD/klf200.pem"
```

Do not assume a certificate copied from another KLF belongs to your gateway.

## MQTT

The default base topic is `velux`. State and metadata use QoS 1 and retained publications.

| Topic | Retained | Payload |
|---|---:|---|
| `<base>/state` | yes | `online` or `offline` |
| `<base>/nodes` | yes | JSON array of node IDs |
| `<base>/velux_node_<id>/info` | yes | Node metadata JSON |
| `<base>/velux_node_<id>/status` | yes | Atomic cover-status JSON |
| `<base>/velux_node_<id>/state` | yes | `open`, `closed`, `opening`, `closing`, or `unknown` |
| `<base>/velux_node_<id>/position` | yes | Open percentage `0..100`, or `unknown` |
| `<base>/velux_node_<id>/target` | yes | Target open percentage `0..100`, or `unknown` |
| `<base>/velux_node_<id>/cmnd/control` | no | `OPEN`, `CLOSE`, or `TOGGLE` |
| `<base>/groups` | yes | JSON array of KLF group IDs |
| `<base>/velux_group_<id>/info` | yes | Group metadata JSON, including member node IDs |
| `<base>/velux_group_<id>/cmnd/control` | no | `OPEN` or `CLOSE` |
| `<base>/cmnd/reboot` | no | `REBOOT` |

Command publications must not be retained. Retained commands are rejected to prevent an old broker
message from moving a cover or rebooting the gateway after restart. A manual reboot first disables
house monitoring, requests and confirms the reboot, closes TLS, and publishes `offline`. The bridge
waits 30 seconds by default before reconnecting so it does not contact the KLF during early boot;
adjust `V2M_KLF_REBOOT_RECONNECT_DELAY` if the gateway needs longer.

Groups use the KLF product-group operation. They intentionally support only `OPEN` and `CLOSE`:
there is no reliable aggregate group position from which to implement `TOGGLE`, and the bridge does
not publish a fabricated group state or position.

KLF native positions use `0% = open` and `100% = closed`; MQTT and Home Assistant use the inverse,
`0 = closed` and `100 = open`. The bridge performs this conversion at the MQTT boundary.

## Home Assistant

Enable discovery with:

```dotenv
V2M_HASS_DISCOVERY=true
```

Controllable nodes and discovered groups publish retained cover discovery records under
`homeassistant/cover/<unique_id>/config`. Group entities are optimistic and expose only open/close
because KLF does not provide a reliable aggregate group state. Unsupported node types remain visible
in generic node metadata but are not exposed as unsafe cover controls. MQTT intentionally exposes no
`STOP` or arbitrary position command.

## Docker

Build the image:

```bash
docker build -t velux2mqtt:local .
```

Run it with an environment file and host networking so the gateway and broker are reachable:

```bash
docker run --rm --network host --env-file .env velux2mqtt:local -v
```

`docker stop` sends `SIGTERM`; the service handles it through the same monitor-disable and optional
reboot sequence as `Ctrl+C`.

## Testing

Regular tests do not contact hardware:

```bash
devenv shell -- just check
devenv shell -- cargo build --locked --release
```

Ignored hardware tests require explicit `V2M_KLF_HOST` and `V2M_KLF_PASSWORD`. The reconnect test
deliberately enables house monitoring and requests a clean shutdown before reconnecting:

```bash
devenv shell -- cargo test --test hardware_klf \
  monitor_shutdown_allows_tls_reconnect -- --ignored --nocapture
```

## Attribution

The original Node.js implementation is the MIT-licensed
[`PLCHome/velux-klf200-api`](https://github.com/PLCHome/velux-klf200-api) project by Chris Traeger.
Protocol behavior is based on the VELUX KLF 200 API specification retained in this repository
