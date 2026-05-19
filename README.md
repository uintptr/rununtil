# rununtil

Run a command until a given wall-clock time, then kill it.

## Usage

```
rununtil --time <TIME> [--restart] [--restart-delay <SECS>] [--quiet] -- <COMMAND>...
```

`<TIME>` accepts:

- `HH:MM`
- `HH:MM:SS`
- RFC 3339 (e.g. `2026-05-19T18:30:00-07:00`)

If the time has already passed today, it wraps to tomorrow.

### Flags

- `-t, --time` — when to stop the command
- `-r, --restart` — relaunch the command after it exits, looping until the deadline logic is re-evaluated each iteration
- `--restart-delay <SECS>` — delay between restarts (default: 10)
- `-q, --quiet` — suppress the countdown line

### Examples

```sh
# Run a server until 5pm
rununtil --time 17:00 -- ./my-server

# Keep restarting a flaky job until 02:30, waiting 30s between runs
rununtil --time 02:30 --restart --restart-delay 30 -- ./worker
```

Logging is via `env_logger`; set `RUST_LOG=info` for details.

## Build

```sh
cargo build --release
```
