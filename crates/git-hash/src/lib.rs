//! Hash computation and object identity for the gitr git implementation.
//!
//! This crate provides the core `ObjectId` type, hash computation, hex
//! encoding/decoding, and specialized OID collections used throughout gitr.

mod error;
pub mod hex;
mod algorithm;
mod oid;
pub mod hasher;
pub mod collections;
pub mod fanout;

pub use algorithm::HashAlgorithm;
pub use error::HashError;
pub use oid::ObjectId;
