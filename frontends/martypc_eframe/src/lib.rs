#![warn(clippy::all, rust_2018_idioms)]

pub mod app;
pub mod counter;
pub mod emulator;
pub mod event_loop;
pub mod input;

#[cfg(feature = "sound")]
pub mod sound;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

pub use app::MartyApp;

// Embed default icon
pub const MARTY_ICON: &[u8] = include_bytes!("../../../assets/martypc_icon_small.png");
