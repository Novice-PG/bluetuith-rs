//! bluetuith-rs — a full-featured Bluetooth manager for the terminal.
//!
//! The crate is split so the interface can be exercised without hardware:
//! `backend` is the only module that knows about BlueZ (or the simulator),
//! `app` is pure state transitions, and `ui` only reads `&App`.

pub mod app;
pub mod backend;
pub mod cli;
pub mod config;
pub mod i18n;
pub mod keybind;
pub mod model;
pub mod theme;
pub mod ui;
pub mod util;
