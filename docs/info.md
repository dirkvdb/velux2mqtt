## Position convention

KLF-native position semantics remain unchanged inside `klf200`:

- `0%` is fully open.
- `100%` is fully closed.

MQTT and Home Assistant use open percentage, so the bridge publishes the inverse:

- `0` is fully closed.
- `100` is fully open.

## Shutdown behavior

Disconnecting while KLF house monitoring remains enabled can leave some gateways unable to finish a
later TLS handshake. Rust waits for monitor-disable confirmation before closing. Set
`V2M_KLF_REBOOT_ON_SHUTDOWN=true` to additionally reboot after monitor disable. The reboot is opt-in
because it interrupts the complete gateway and is never used for routine reconnect cleanup. The
manual MQTT reboot uses the same monitor-disable/reboot/close sequence and delays reconnection by
`V2M_KLF_REBOOT_RECONNECT_DELAY` seconds, defaulting to 30.
