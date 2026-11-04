# bluetuith-rs

A full-featured Bluetooth manager for the terminal, written from scratch in
**Rust** with [ratatui](https://ratatui.rs/).

![bluetuith-rs main view](assets/screenshot.png)

`bluetuith-rs` is a clean-room reimplementation of the *idea* behind
[bluetuith](https://github.com/bluetuith-org/bluetuith) (a Go/tview Bluetooth
TUI). It contains no Go code and no code copied from bluetuith: the whole
application — models, BlueZ D-Bus backend, event loop and every widget — is
Rust. It keeps feature parity with the original and adds a few extras.

---

## Highlights

- **Complete Bluetooth management over BlueZ D-Bus** — adapter power,
  discoverable/pairable modes, discovery, device connect/disconnect, pairing,
  trust, block and remove.
- **Live device list** with signal strength, battery level, device class,
  paired/connected/trusted/blocked badges, search/filter and six sort modes.
- **OBEX Object Push file transfers** with a file picker, per-transfer progress,
  suspend / resume / cancel, and incoming transfer monitoring from `obexd`.
- **Media player** (AVRCP / `org.bluez.MediaPlayer1`) with play/pause,
  next/previous, seek, stop, volume, track metadata and a mini-player bar.
- **PAN networking** (PANU / NAP / DUN) and **audio profile** views.
- **Pairing agent** — PIN, passkey, numeric comparison and authorization
  prompts are real dialogs, not decorative ones.
- **Nine built-in palettes**, runtime theme switching, custom accent colour,
  configurable keybindings with conflict detection, mouse support,
  right-click context menus, command palette, event log and help view.
- **`--demo` mode** — a fully interactive simulated adapter, so the whole
  interface works (and is testable) on machines without Bluetooth hardware.
- **Single static binary** — `.deb` packages for all Linux architectures.

## Feature parity

| Area | bluetuith | bluetuith-rs |
|---|---|---|
| Adapter selector + status indicators | ✓ | ✓ |
| Power / Discoverable / Pairable / Scan | ✓ | ✓ |
| Connect / Disconnect / Pair / Cancel pairing | ✓ | ✓ |
| Trust / Untrust / Block / Unblock / Remove | ✓ | ✓ |
| Device info + service UUIDs | ✓ | ✓ |
| File picker (select / invert / all / hidden / refresh) | ✓ | ✓ |
| OBEX Object Push send + transfer progress view | ✓ | ✓ |
| Transfers suspend / resume / cancel | ✓ | ✓ |
| Pairing + file-transfer authentication views | ✓ | ✓ |
| Media player (play/pause/next/prev/seek/stop) | ✓ | ✓ |
| Network options (PAN/NAP/DUN) | ✓ | ✓ |
| Audio profiles | ✓ | ✓ |
| Mouse support | ✓ | ✓ |
| Configurable theme + keybindings | ✓ | ✓ |
| Help / keybinding view | ✓ | ✓ |
| Suspend (`Ctrl+Z`) / quit confirmation | ✓ | ✓ |
| CLI flags + config file + env vars | ✓ | ✓ |
| Device search / filter | — | ✓ |
| Six sort modes | — | ✓ |
| Battery + RSSI + device-kind icons | — | ✓ |
| Details side panel | — | ✓ |
| Nine palettes + live theme picker | — | ✓ |
| Command palette, event log | — | ✓ |
| `--demo` simulated adapter | — | ✓ |
| `--list-adapters` non-interactive mode | ✓ | ✓ |
| Static multi-arch `.deb` packages | — | ✓ |

## Install

### Debian / Ubuntu package

`.deb` packages for every Linux architecture are produced by
`packaging/build-debs.sh` (locally) and by the CI matrix; pushing a `v*` tag
attaches them all to the GitHub Release. To build them yourself:

```sh
./packaging/build-debs.sh
sudo dpkg -i packaging/out/bluetuith-rs_0.1.0_amd64.deb   # or arm64, armhf,
                                                          # armel, i386, ppc64el,
                                                          # riscv64, loong64, ppc64
```

The package installs `/usr/bin/bluetuith-rs`, a man page, a desktop entry,
shell completions and an example config. It has no runtime dependencies besides
a system D-Bus and BlueZ (both optional in `--demo` mode).

### From source

```sh
cargo build --release --bin bluetuith-rs
sudo install -m755 target/release/bluetuith-rs /usr/local/bin/
```

Requires Rust 1.85+. On Linux the only system requirement is a running D-Bus
system bus; BlueZ is optional (without it, use `--demo`).

## Usage

```sh
bluetuith-rs                 # attach to BlueZ
bluetuith-rs --demo          # run against a simulated adapter
bluetuith-rs --theme Nord    # pick a palette
bluetuith-rs --list-adapters # print adapters and exit
bluetuith-rs --generate      # write a default config file
bluetuith-rs --help
```

### Main keybindings

| Key | Action | Key | Action |
|---|---|---|---|
| `↑` `↓` | navigate | `c` | connect / disconnect |
| `p` | pair | `t` | trust / untrust |
| `b` | block / unblock | `d` | remove device |
| `o` `S` `P` `s` | power / discoverable / pairable / scan | `a` | change adapter |
| `f` | send files | `v` | transfers |
| `m` | media player | `M` | hide player |
| `n` | network options | `A` | audio profiles |
| `i` | device info | `/` | search / filter |
| `F2` | toggle details | `F3` | cycle sort |
| `F4` | theme picker | `F6` | event log |
| `Ctrl+p` | command palette | `Alt+m` | menu bar |
| `?` | help | `Q` | quit |
| `Ctrl+z` | suspend | `Ctrl+x` | cancel |

Double-click a device row to connect; right-click opens the device context
menu. `Esc` closes any overlay. Every binding can be overridden — see below.

### Configuration

`~/.config/bluetuith-rs/config.toml` (or `$XDG_CONFIG_HOME`). Generate a
commented default with `--generate`:

```toml
adapter = "hci0"
receive-dir = ""
gsm-number = "*99#"
adapter-states = "powered:yes,discoverable:no,scan:no"
connect-bdaddr = ""
confirm-on-quit = true
no-help-display = false
theme = "Midnight"
accent = "#5ec4ff"
default-sort = "Name"
show-hidden-files = false
scan-on-start = false

[keybindings]
# Quit = "Ctrl+q"
# Connect = "c"
```

All options also have `--flag` equivalents and `BLUETUITH_RS_*` environment
variables. Built-in palettes: **Midnight**, **Nord**, **Catppuccin**,
**Dracula**, **Gruvbox**, **Tokyo Night**, **Rosé Pine**, **Solarized**,
**Daylight**.

## Architecture

```
src/
├── model.rs          plain data types (no terminal, no D-Bus)
├── backend/
│   ├── mod.rs        Backend trait, Command/Event, service names
│   ├── bluez.rs      BlueZ over zbus (blocking D-Bus on a worker thread)
│   └── demo.rs       deterministic simulator used by --demo and the tests
├── app.rs            application state + all input handling
├── config.rs         TOML config, CLI/env overrides
├── keybind.rs        configurable bindings + conflict detection
├── theme.rs          colour palettes
├── ui/               ratatui rendering (chrome, views, overlays)
└── main.rs           CLI entry, terminal setup, event loop
```

The UI never touches BlueZ directly: it sends `Command`s and reacts to
`Event`s. The BlueZ worker polls
`org.freedesktop.DBus.ObjectManager.GetManagedObjects` on `org.bluez` (and
`org.bluez.obex` when available), executes operations through `org.bluez.*`
methods, and registers an `org.bluez.Agent1` that blocks until the user answers
a pairing prompt. Polling instead of signal subscriptions keeps the state
machine simple and resilient to BlueZ's many optional interfaces.

Feature availability is reported at runtime: if `obexd` or a media player is
absent, the corresponding controls stay visible but report that the service is
unavailable, matching bluetuith's `appfeatures` behaviour.

## Building `.deb` packages for all Linux architectures

`packaging/build-debs.sh` cross-compiles a **static** binary per target using
`rustup target add` + Rust's bundled `rust-lld` (so no cross C toolchain is
needed) and wraps it with `dpkg-deb`:

```sh
./packaging/build-debs.sh                 # all supported targets
./packaging/build-debs.sh aarch64-unknown-linux-musl   # one target
```

| Debian arch | Rust target | Debian arch | Rust target |
|---|---|---|---|
| `amd64` | `x86_64-unknown-linux-musl` | `arm64` | `aarch64-unknown-linux-musl` |
| `armhf` | `armv7-unknown-linux-musleabihf` | `armel` | `arm-unknown-linux-musleabi` |
| `i386` | `i686-unknown-linux-musl` | `ppc64el` | `powerpc64le-unknown-linux-musl` |
| `riscv64` | `riscv64gc-unknown-linux-musl` | `loong64` | `loongarch64-unknown-linux-musl` |
| `ppc64` | `powerpc64-unknown-linux-musl` | | |

Architectures that Rust only ships `*-linux-gnu` std for (`s390x`,
`sparc64`, `mips*`) are built by the GitHub Actions matrix in
`.github/workflows/debs.yml` using Debian's cross toolchains. Every package is
verified with `dpkg-deb --info`.

## Development

```sh
cargo test                       # unit + snapshot render tests
cargo clippy --all-targets -- -D warnings
cargo run --example snapshot -- 150 44 main /tmp/frame.json
python3 packaging/render-snapshot.py /tmp/frame.json /tmp/frame.png
```

`examples/snapshot.rs` renders any screen into a `TestBackend` and dumps every
cell, which is how the screenshot above is produced without a real terminal.

## Acknowledgements

Inspired by [bluetuith](https://github.com/bluetuith-org/bluetuith) by darkhz
(MIT). This is an independent Rust implementation; no source code was reused.
BlueZ integration uses [zbus](https://github.com/dbus2/zbus); the interface is
built with [ratatui](https://ratatui.rs/) and
[crossterm](https://github.com/crossterm-rs/crossterm).

## License

Apache-2.0. See [LICENSE](LICENSE).
