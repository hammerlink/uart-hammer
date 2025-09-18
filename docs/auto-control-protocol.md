# Purpose

**Automated, repeatable, full-duplex UART validation** to verify kernel driver behavior across all common configs up to **2,000,000 baud**, with a framework that lets you add new tests over time.

---

# Test space (configs)

Vary along these axes (each axis can be filtered):

* **Baud:** `9600 … 2_000_000` (curated defaults; overridable).
* **Parity:** `none, even, odd`.
* **Data bits:** `7, 8`.
* **Stop bits:** `1`.
* **Direction:** `tx, rx, both` (full-duplex).
* **Flow control:** `none, rtscts`.

💡 **Capability exchange:** on connect, each side advertises supported maxima (e.g. highest baud, which parities/flow the driver supports). The master prunes the matrix accordingly.

---

# Tests (initial two)

1. **Max rate test**

   * For each config, stream fixed-size frames for `N` frames or `T` seconds (configurable).
   * **Pass:** 100% of frames received, `bad_crc=0`, `seq_gaps=0`, optional min throughput ≥ target.
   * **Metrics:** `rx_frames/bytes`, crc errors, seq gaps, driver error flags (overrun/framing/parity), `rx_rate_bps`.

2. **FIFO residue check**

   * Send incremental payload lengths with inter-frame delays (e.g. 1..payload\_max with `Δ=delay_us`) to probe buffering.
   * **Pass:** all frames observed in order; no drops; timing jitter within window if measured.
   * Runs on **default config** by default; flag to expand to **all configs**.

---

# Roles & control channel

* **Slave role → `auto` command**
  (`uart-hammer auto --dev …`)

  * Always started **first**.
  * Waits for a `HELLO` from the master.
  * Executes commands, returns results.
  * If no master traffic for **60s**, returns to waiting state.

* **Master role → `test` command**
  (`uart-hammer test --dev …`)

  * Orchestrates the suite, pushes test commands, synchronizes retunes, collects peer results.

**Control channel**: always **115200, 8N1, no flow** on the same UART under test.
Data tests retune *both ends* for each config; control messages handle the sync.

---

# Discovery & handshake

* **Auto boot behavior:**
  Broadcast `HELLO` every **500 ms** until it gets an `ACK`.
  Use exponential backoff (500 ms → max 4 s) if no master is present.

* **Test boot behavior:**
  Listen for `HELLO`; reply `ACK` with a **run-id** and its capabilities; request peer capabilities; compute the **test plan**.

**Collision safety:** roles are explicit; auto never drives tests.

---

# Protocol

At the start of an `auto`/`test` run, each side generates an ID.
All messages include `id=<run-id>` to allow ignoring strays.

* **Discovery**

  * auto: `HELLO id=<auto_id>`
  * test: `ACK id=<test_id>`
    Both sides must store the other’s ID to survive restarts.

* **Config**

  * test:
    `CONFIG SET id=<test_id> baud=<B> parity=<P> bits=<N> dir=<tx|rx|both> flow=<none|rtscts>`
  * auto:
    `CONFIG SET ACK id=<auto_id> baud=<B> parity=<P> bits=<N> dir=<tx|rx|both> flow=<none|rtscts>`

* **Test orchestration**

  * **Begin**

    * test:
      `TEST BEGIN id=<test_id> name=<max-rate|fifo-residue> frames=<M>|duration_ms=<T> payload=<K>`
    * auto:
      `TEST BEGIN ACK id=<auto_id> name=<max-rate|fifo-residue> frames=<M>|duration_ms=<T> payload=<K>`
  * **Done**

    * Half-duplex: TX side repeats until ACK.
    * Full-duplex: master repeats until ACK.
    * both:
      `TEST DONE id=<id> result=<pass/fail>`
  * **Done Ack**

    * Half-duplex: RX side sends ACK.
    * Full-duplex: auto sends ACK.
    * both:
      `TEST DONE ACK id=<id>`
  * **Result**

    * test/auto:
      `TEST RESULT id=<id> result=<pass/fail> rx_frames=<…> rx_bytes=<…> bad_crc=<…> seq_gaps=<…> overruns=<…> errors=<bitmask?> rate_bps=<…> reason=<optional>`
    * Each node prints results locally (both master + auto results).

* **Terminate**

  * test: `TERMINATE id=<test_id>`
  * auto: `TERMINATE ACK id=<auto_id>`
  * Test exits; auto reverts to waiting for the next run.

---

Would you like me to also **draw a timing/sequence diagram** (like MSC/PlantUML) for the `HELLO → ACK → CONFIG → TEST → RESULT` flow? That could make this much easier to follow for developers.
