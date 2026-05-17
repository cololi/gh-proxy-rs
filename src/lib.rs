//! Library facade for the `hub_proxy` crate.
//!
//! The binary entry point lives in `main.rs`; integration tests import the
//! modules through this library target.

pub mod config;
pub mod homepage;
pub mod matcher;
pub mod proxy;
