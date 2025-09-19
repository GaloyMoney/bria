#![cfg_attr(feature = "fail-on-warnings", deny(warnings))]
#![cfg_attr(feature = "fail-on-warnings", deny(clippy::all))]

pub mod account;
pub mod address;
pub mod admin;
mod api;
pub mod app;
pub mod batch;
mod batch_inclusion;
pub mod bdk;
pub mod cli;
pub mod descriptor;
mod dev_constants;
pub mod fees;
mod job;
pub mod ledger;
mod outbox;
pub mod payout;
pub mod payout_queue;
pub mod primitives;
pub mod profile;
pub mod signing_session;
mod token_store;
mod tracing;
pub mod utxo;
pub mod wallet;
pub mod xpub;
