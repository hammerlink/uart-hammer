# UART-Hammer 🔨

A cross-platform Rust CLI to **stress-test and benchmark UART/serial links**.  
Generate frames at line-rate, verify checksums and sequence numbers, and measure throughput, loss, and errors.

---

## Features

- **TX mode** — blast frames with configurable payload size and pacing (`max`, fixed gap, or auto-paced by baud).
- **RX mode** — receive and validate frames, track sequence gaps, checksum errors, and throughput.
- **Cross-compile ready** — runs on x86 hosts and RISC-V SoCs (or any Linux with `/dev/ttyS*`).
- **Stats & logging** — per-second stats with optional debug output for each bad/lost frame.
- **Open source** — extend it with your own frame formats or logging backends.

---

## Install

```bash
# Clone and build with Cargo
git clone https://github.com/hammerlink/uart-hammer.git
cd uart-hammer
cargo build --release
