#![cfg_attr(feature = "fail-on-warnings", deny(warnings))]
#![cfg_attr(feature = "fail-on-warnings", deny(clippy::all))]

mod admin;
mod api;
pub mod cli;
pub mod config;
mod macros;
mod primitives;
mod tracing;
