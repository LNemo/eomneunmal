//! 없는말 core library.
//!
//! The Tauri shell will call into this crate, but raw text and secrets stay in
//! Rust/native boundaries rather than the WebView.

pub mod core;

pub mod platform;
