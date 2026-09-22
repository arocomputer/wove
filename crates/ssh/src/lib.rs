//! Authenticated SSH transport for independent Wove applications.
#![forbid(unsafe_code)]
mod runtime;
mod server;
pub use russh::keys::{PrivateKey, PublicKey};
pub use server::{App, Peer, Server};
/// Transport or application failure, preserving its original error type.
pub type Error = Box<dyn std::error::Error + Send + Sync>;
