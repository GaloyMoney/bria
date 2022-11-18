#![cfg_attr(feature = "fail-on-warnings", deny(warnings))]
#![cfg_attr(feature = "fail-on-warnings", deny(clippy::all))]

mod account;
mod admin;
mod api;
mod app;
pub mod cli;
mod error;
mod macros;
mod primitives;
mod tracing;
