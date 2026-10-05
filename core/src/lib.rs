//! Core, UI-independent logic for Nspawn Studio.
//!
//! This crate contains everything that can be reasoned about and tested
//! without a graphical session: the container data model, validation, the
//! configuration store and the systemd-nspawn launcher/unit generator.

pub mod command;
pub mod generator;
pub mod model;
pub mod security;
pub mod store;
pub mod validate;

pub use model::*;
