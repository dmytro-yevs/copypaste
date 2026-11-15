# CopyPaste sync relay

The relay lets paired devices sync when they are not on the same network. It
introduces the devices so they can connect to each other directly, and it
forwards their traffic itself when a direct path is impossible. It stores
nothing.

## How devices connect

1. On the same network, devices find each other with mDNS and never use the
   relay.
2. Otherwise each device asks the relay's UDP observer for its public address.
   The relay passes the addresses between the two devices, both open their NAT
   towards the other at the same moment, and they connect with QUIC. Traffic
   then flows directly between the devices.
3. If that fails within five seconds (symmetric NAT, carrier-grade NAT, or
   blocked UDP), the relay forwards the session itself. A device that could
   not connect directly to a peer goes straight to the relay for the next ten
   minutes.

The optional **Router port mapping** setting lets a device also ask its router
(UPnP) to forward the UDP port for two minutes, which helps when both networks
would otherwise block a direct path.

## What the relay can see

Devices run the same end-to-end encrypted Noise session that they use on the
LAN, over the direct QUIC path as well as through the relay, so the relay only
ever forwards opaque frames. It never receives clipboard
content, item metadata, device names or pairing keys.

The relay does see:

- device IP addresses;
- when devices are online;
- how many pairings each device has;
- how many bytes each session moves.

Each pairing registers under an id derived from its key. Only the two paired
devices can prove that they own the id.

## Run it

Build and run the binary:

```bash
cargo run --release -p copypaste-relay -- --listen 0.0.0.0:8080
```

Or run it in Docker:

```bash
docker build -f crates/copypaste-relay/Dockerfile -t copypaste-relay .
```

```bash
docker run -d --name copypaste-relay -p 8080:8080 -p 3478:3478/udp -e COPYPASTE_RELAY_TOKEN=change-me copypaste-relay
```

The relay listens for plain `ws://`. To use `wss://`, put a reverse proxy in
front of it that terminates TLS and forwards WebSocket upgrades. The UDP
observer cannot go through an HTTP proxy: expose its port directly. If devices
must reach it under a different name than the relay URL (for example, when a
CDN proxies the relay hostname), set `--udp-public`. For example,
in Caddy:

```
relay.example.com {
	reverse_proxy 127.0.0.1:8080
}
```

## Settings

| Flag | Environment variable | Default |
| --- | --- | --- |
| `--listen` | `COPYPASTE_RELAY_LISTEN` | `0.0.0.0:8080` |
| `--token` | `COPYPASTE_RELAY_TOKEN` | none: the relay is open |
| `--max-connections` | `COPYPASTE_RELAY_MAX_CONNECTIONS` | 1024 |
| `--max-connections-per-ip` | `COPYPASTE_RELAY_MAX_PER_IP` | 32 |
| `--session-mib` | `COPYPASTE_RELAY_SESSION_MIB` | 256 MiB per session |
| `--rate-kib` | `COPYPASTE_RELAY_RATE_KIB` | 4096 KiB/s per direction (`0` turns the limit off) |
| `--udp-listen` | `COPYPASTE_RELAY_UDP_LISTEN` | `0.0.0.0:3478` (empty turns direct paths off) |
| `--udp-public` | `COPYPASTE_RELAY_UDP_PUBLIC` | the relay URL host and the observer port |

The relay also enforces these fixed limits:

- One frame holds at most one Noise message (64 KiB).
- One connection holds at most 32 registrations.
- A dial waits at most 8 seconds for the other device to answer.
- An idle presence is dropped after 150 seconds.
- An idle session is dropped after 120 seconds.
- The observer replies with at most as many bytes as it received, so it cannot
  be used to amplify spoofed traffic.
- A punch offers at most four candidate addresses.

## Connect devices

In **Settings → Sync → Relay server**, enter the same address on every device,
for example `wss://relay.example.com`. If the relay has a token, add it as
`wss://relay.example.com/?token=change-me`. The app sends the token inside the
first message, never in the request URL. Leave the field empty to turn the
relay off.

Once the relay is set, devices that are already paired sync through it, or
directly with its help, whenever they cannot reach each other on the LAN.

To pair two devices that have never been on the same network, enter only the
code on the joining device and leave the address empty. The relay routes the
code by its first byte. The PAKE protects the rest of the code, and both people
still compare the six-digit verification code.

## Delivery

The relay delivers only while both devices are online. Nothing waits on the
server. History sync is cursor-based, so whatever changed while a device was
away arrives in its next session. For devices that are never online at the same
time, use Supabase Sync.
