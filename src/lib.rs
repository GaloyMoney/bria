#![cfg_attr(feature = "fail-on-warnings", deny(warnings))]
#![cfg_attr(feature = "fail-on-warnings", deny(clippy::all))]

pub mod account;
pub mod admin;
mod api;
pub mod app;
pub mod batch_group;
pub mod bdk;
pub mod cli;
mod error;
mod job;
pub mod ledger;
mod macros;
pub mod payout;
pub mod primitives;
mod tracing;
pub mod wallet;
pub mod xpub;
