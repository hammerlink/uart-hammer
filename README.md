# UART-Hammer 🔨

A cross-platform Rust CLI to **stress-test and benchmark UART/serial links**.  
Generate frames at line-rate, verify checksums and sequence numbers, and measure throughput, loss, and errors.

---

## Features

- **TX mode** blast frames with configurable payload size and pacing (`max`, fixed gap, or auto-paced by baud).
- **RX mode** receive and validate frames, track sequence gaps, checksum errors, and throughput.
- **Cross-compile ready** runs on x86 hosts and RISC-V SoCs (or any Linux with `/dev/ttyS*`).
- **Stats & logging** per-second stats with optional debug output for each bad/lost frame.

---

## Install

```bash
# Clone and build with Cargo
git clone https://github.com/hammerlink/uart-hammer.git
cd uart-hammer
just build # this builds both risc-v & x86
```

## 🚀 Usage

You’ll need **two devices** (or two UART ports on the same machine).
Run one side in **auto** mode, the other in **test** mode.

### Example: Auto/Responder Mode

```bash
uart-hammer auto --dev /dev/ttyS1
```

### Example: Baud Sweep Test

```bash
uart-hammer test \
  --dev /dev/ttyS1 \
  --bauds "115200,230400,460800,921600,1000000,1500000,3000000"
```

This will iterate through the listed baud rates, sending/receiving test frames and printing stats.

---

## ⚙️ Options

| Flag                 | Default                         | Description                                            |
| -------------------- | ------------------------------- | ------------------------------------------------------ |
| `--dev <PATH>`       | *(required)*                    | UART device path (`/dev/ttyS1`, `/dev/ttyUSB0`, etc.). |
| `--tests <LIST>`     | `max-rate,fifo-residue`         | Comma-separated test selection.                        |
| `--bauds <LIST>`     | `115200,57600,38400,19200,9600` | Baud rates to test (comma-separated).                  |
| `--parity <MODE>`    | `none`                          | Parity: `none`, `even`, `odd`.                         |
| `--bits <N>`         | `8`                             | Data bits (e.g. `7`, `8`).                             |
| `--dir <MODE>`       | `tx,rx`                         | Direction: `tx`, `rx`, or `both`.                      |
| `--flow <MODE>`      | `none`                          | Flow control: `none`, `rtscts`.                        |
| `--payload <BYTES>`  | `32`                            | Payload size per frame (bytes).                        |
| `--frames <N>`       | `200`                           | Number of frames per test.                             |
| `--duration-ms <MS>` | *(optional)*                    | Run test for given duration (overrides `--frames`).    |

---

## 📊 Example Output

```
# > uart-hammer test --dev /dev/ttyUSB1 --bauds "115_200, 230_400, 460_800, 921_600, 1_000_000, 1_500_000, 3_000_000"
[port] reconfigured to 115200 8-N-1-
[test] id=1c9ee05a-29e3-4576-bd4a-bc451731efdd awaiting slave
[test] got ACK from slave id=9e58663a-427d-422b-b30a-355566dcc529
[port] reconfigured to 115200 8-N-1-
[test] running test 'max-rate' dir=Tx at PortConfig { baud: 115200, parity: None, bits: 8, flow: None, stop_bits: 1 } 115200bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=91116 reason=none
[test] running test 'max-rate' dir=Rx at PortConfig { baud: 115200, parity: None, bits: 8, flow: None, stop_bits: 1 } 115200bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=91215 reason=none
[test] running test 'fifo-residue' dir=Tx at PortConfig { baud: 115200, parity: None, bits: 8, flow: None, stop_bits: 1 } 115200bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=91100 reason=none
[test] running test 'fifo-residue' dir=Rx at PortConfig { baud: 115200, parity: None, bits: 8, flow: None, stop_bits: 1 } 115200bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=91218 reason=none
[port] reconfigured to 230400 8-N-1-
[test] running test 'max-rate' dir=Tx at PortConfig { baud: 230400, parity: None, bits: 8, flow: None, stop_bits: 1 } 230400bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=180811 reason=none
[test] running test 'max-rate' dir=Rx at PortConfig { baud: 230400, parity: None, bits: 8, flow: None, stop_bits: 1 } 230400bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=180821 reason=none
[test] running test 'fifo-residue' dir=Tx at PortConfig { baud: 230400, parity: None, bits: 8, flow: None, stop_bits: 1 } 230400bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=180939 reason=none
[test] running test 'fifo-residue' dir=Rx at PortConfig { baud: 230400, parity: None, bits: 8, flow: None, stop_bits: 1 } 230400bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=180821 reason=none
[port] reconfigured to 460800 8-N-1-
[test] running test 'max-rate' dir=Tx at PortConfig { baud: 460800, parity: None, bits: 8, flow: None, stop_bits: 1 } 460800bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=354449 reason=none
[test] running test 'max-rate' dir=Rx at PortConfig { baud: 460800, parity: None, bits: 8, flow: None, stop_bits: 1 } 460800bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=355274 reason=none
[test] running test 'fifo-residue' dir=Tx at PortConfig { baud: 460800, parity: None, bits: 8, flow: None, stop_bits: 1 } 460800bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=356094 reason=none
[test] running test 'fifo-residue' dir=Rx at PortConfig { baud: 460800, parity: None, bits: 8, flow: None, stop_bits: 1 } 460800bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=355289 reason=none
[port] reconfigured to 921600 8-N-1-
[test] running test 'max-rate' dir=Tx at PortConfig { baud: 921600, parity: None, bits: 8, flow: None, stop_bits: 1 } 921600bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=689258 reason=none
[test] running test 'max-rate' dir=Rx at PortConfig { baud: 921600, parity: None, bits: 8, flow: None, stop_bits: 1 } 921600bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=686720 reason=none
[test] running test 'fifo-residue' dir=Tx at PortConfig { baud: 921600, parity: None, bits: 8, flow: None, stop_bits: 1 } 921600bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=691585 reason=none
[test] running test 'fifo-residue' dir=Rx at PortConfig { baud: 921600, parity: None, bits: 8, flow: None, stop_bits: 1 } 921600bps
[auto] PASS frames=200 bytes=19490 bad_crc=0 gaps=0 overruns=0 errors=0x0 rate_bps=686730 reason=none
[port] reconfigured to 1000000 8-N-1-
[test] running test 'max-rate' dir=Tx at PortConfig { baud: 1000000, parity: None, bits: 8, flow: None, stop_bits: 1 } 1000000bps
[test] max-rate test failed: running max-rate test
[test] running test 'max-rate' dir=Rx at PortConfig { baud: 1000000, parity: None, bits: 8, flow: None, stop_bits: 1 } 1000000bps
[test] max-rate test failed: running max-rate test
```

