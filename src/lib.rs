#![cfg_attr(feature = "fail-on-warnings", deny(warnings))]
#![cfg_attr(feature = "fail-on-warnings", deny(clippy::all))]

pub mod account;
pub mod address;
pub mod admin;
mod api;
pub mod app;
pub mod batch;
pub mod batch_group;
pub mod bdk;
pub mod cli;
mod entity;
mod error;
pub mod fee_estimation;
mod job;
pub mod ledger;
pub mod payout;
pub mod primitives;
pub mod profile;
pub mod signing_session;
mod tracing;
pub mod utxo;
pub mod wallet;
pub mod xpub;
