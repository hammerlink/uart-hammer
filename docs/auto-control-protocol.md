# Purpose

**Automated, repeatable, full-duplex UART validation** to verify kernel driver behavior across all common configs up to **2,000,000 baud**, with a framework that lets you add new tests over time.

---

# Test space (configs)

Vary along these axes (each axis can be filtered):

- **Baud:** `9600 … 2_000_000` (curated defaults; overridable).
- **Parity:** `none, even, odd`.
- **Data bits:** `7, 8`.
- **Stop bits:** `1`.
- **Direction:** `tx, rx, both` (full-duplex).
- **Flow control:** `none, rtscts`.

💡 **Capability exchange:** on connect, each side advertises supported maxima (e.g., highest baud, which parities/flow the driver supports); master prunes the matrix accordingly.

---

# Tests (initial two)

1. **Max rate test**
    - For each config, stream fixed-size frames for `N` frames or `T` seconds (configurable).
    - **Pass:** 100% of frames received, `bad_crc=0`, `seq_gaps=0`, optional min throughput ≥ target.
    - **Metrics:** rx_frames/bytes, crc errors, seq gaps, driver error flags (overrun/framing/parity), rx_rate_bps.
2. **FIFO residue check**
    - Send incremental payload lengths with inter-frame delays (e.g., 1..payload_max with `Δ=delay_us`) to probe buffering.
    - **Pass:** all frames observed in order; no drops; timing jitter within window if measured.
    - Run on **default config** by default; flag to expand to **all configs**.

---

# Roles & control channel

- **Master** (`uart-hammer auto --master --dev …`)  
    Orchestrates the suite, pushes test commands, synchronizes retunes, collects peer results.
- **Slave** (`uart-hammer auto --slave --dev …`)  
    Announces presence, executes commands, returns results.

**Control channel**: always **115200, 8N1, no flow** on the same UART under test.  
Data tests retune _both ends_ for each config; control messages handle the sync.

---

# Discovery & handshake

- **Slave boot behavior:** broadcast `HELLO` every **500 ms** until it gets `ACK`.  
    Use **exponential backoff** (500 ms → max 4 s) to reduce chatter if master is absent.
- **Master boot behavior:** listen for `HELLO`; reply `ACK` with a **run-id** and its capabilities; request slave capabilities; compute the **test plan**.

**Collision safety:** both roles are explicit; slave never drives tests.

---

# Protocol

At the start of the auto, no matter the role generate an id.
Each message sent must include it's own id.
All lines include `id=<run-id>` to ignore strays.

- Discovery:
    - slave: `HELLO id=<slave_id>`
    - master: `ACK id=<master_id>`
Both sides must store the other side's id. To make sure that there is no sudden restart.

- Config:
	-  master: `CONFIG SET id=<master_id> baud=<B> parity=<P> bits=<N> dir=<tx|rx|both> flow=<none|rtscts>`
    - slave: `CONFIG SET ACK id=<slave_id> baud=<B> parity=<P> bits=<N> dir=<tx|rx|both> flow=<none|rtscts>

- Test orchestration:
	- Test Start:
	    - master: `TEST BEGIN id=<master_id> name=<max-rate|fifo-residue> frames=<M>|duration_ms=<T> payload=<K>`
	    - slave: `TEST BEGIN ACK id=<slave_id> name=<max-rate|fifo-residue> frames=<M>|duration_ms=<T> payload=<K>`
	- Test Done:
		- Direction: half-duplex, TX sides sends in [repeat mode](## Repeat mode) until ACK
		- Direction: full-duplex, Master sends in [repeat mode](## Repeat mode) until ACK
		- master/slave: `TEST DONE id=<master_id/slave_id> result=<pass/fail>
	- Test Done Ack:
	    - Direction: half-duplex, RX sides sends ACK
		- Direction: full-duplex, Slave sends ACK
		- master/slave: `TEST DONE ACK id=<master_id/slave_id>
	- Test Result:
		- master: `TEST RESULT id=<master_id> result=<pass/fail> rx_frames=<…> rx_bytes=<…> bad_crc=<…> seq_gaps=<…> overruns=<…> errors=<bitmask?> rate_bps=<…> reason=<optional>
		- slave: `TEST RESULT id=<slave_id> result=<pass/fail> rx_frames=<…> rx_bytes=<…> bad_crc=<…> seq_gaps=<…> overruns=<…> errors=<bitmask?> rate_bps=<…> reason=<optional>
		- each node now prints the results in the console (both master / slave results)
- Terminate:
	- master: `TERMINATE id=<master_id>`
	- slave: `TERMINATE ACK id=<slave_id>`
	Master can now terminate. Slave goes back to listening.
