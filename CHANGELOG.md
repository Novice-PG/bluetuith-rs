# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-11-15

Initial release: a clean-room Rust/ratatui reimplementation of the
[bluetuith](https://github.com/bluetuith-org/bluetuith) feature set, with
extras.

### Added

- Adapter control: power, discoverable, pairable, scan, adapter switching.
- Device actions: connect/disconnect, pair/cancel, trust, block, remove,
  device info and service UUID listing.
- OBEX Object Push **send and receive**: file picker (select / invert / all /
  hidden / refresh), per-transfer progress, suspend / resume / cancel, an
  incoming-file authorization dialog and a configurable `receive-dir` (via
  `org.bluez.obex.Agent1`).
- Media player (AVRCP / `org.bluez.MediaPlayer1`): play/pause, previous/next,
  seek, stop, volume, shuffle/repeat and a mini-player bar.
- PAN networking (PANU / NAP; DUN reserved) and audio profile views.
- Pairing agent: PIN, passkey, numeric comparison and authorization dialogs.
- Live device list with signal strength, battery, device class and state
  badges; search/filter and six sort modes; a details side panel.
- Nine built-in colour palettes, runtime theme switching and custom accent.
- Configurable keybindings with conflict detection, mouse support and
  right-click context menus, command palette, event log and help view.
- Bilingual UI (English / 中文), auto-detected from `$LANG`.
- `--demo` simulated adapter and `--list-adapters` non-interactive mode.
- Static binaries and `.deb` packages for all Linux architectures.

[0.1.0]: https://github.com/Novice-PG/bluetuith-rs/releases/tag/v0.1.0
