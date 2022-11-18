#![cfg_attr(feature = "fail-on-warnings", deny(warnings))]
#![cfg_attr(feature = "fail-on-warnings", deny(clippy::all))]

pub mod account;
pub mod admin;
mod api;
mod app;
pub mod cli;
mod error;
mod macros;
pub mod primitives;
mod tracing;
pub mod xpub;
