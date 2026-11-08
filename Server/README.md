# Setochka Server

A software reimplementation of the original GRiD Server.

## Build

Prebuilt binaries are not available yet, so you need to build the server yourself. Only Linux is currently supported.

Install [Rust using rustup](https://rustup.rs/), GCC, and the [Mold linker](https://github.com/rui314/mold).

Clone the repository, open the server directory, and build the server:

`$ git clone https://github.com/vklachkov/grid-compass-network.git`

`$ cd grid-compass-network/Server`

`$ cargo build --locked --release`

`cargo` installs the required Rust version automatically. The executable is created at `target/release/setochka`.

## Run

Copy the example configuration and edit it for your connection:

```sh
cp config.example.toml config.toml
```

Start the server:

```sh
setochka config.toml
```

For a serial connection, your user account must have permission to access the specified device.

## Configuration

An example configuration is available in [config.example.toml](config.example.toml):

```toml
# Path to the database storing users and service data.
database_path = "server.db"

# Path to the directory storing all Compass files.
filesystem_root = "filesystem"

# Web frontend. Uncomment both lines to enable.
# [frontend]
# listen_address = "127.0.0.1:8080"

# List of transports.
# Any number of transports can be configured.

[[transport]]
type = "serial"
path = "/dev/ttyUSB0"
baud_rate = 9600
char_size = 8
parity = "none"
stop_bits = 1
flow_control = "none"  # or "xon/xoff", "rts/cts".

# TCP transport. Uncomment all three lines to enable.
# [[transport]]
# type = "tcp"
# listen_address = "127.0.0.1:8000"
```
