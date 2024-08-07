// during development we don't want these
#![allow(dead_code)]

//! skunk - 🦨 A person-in-the-middle proxy
//!
//! This is the library, which can be used to program applications using skunk.
//! If you just want to run the program, take a look at `skunk-cli`.
//!
//! # Example
//!
//! TODO

#[doc(hidden)]
pub mod address;
pub mod connect;
pub mod protocol;
pub mod proxy;
pub mod rule;
pub mod util;

// hack to get the proc-macro working from this crate
extern crate self as skunk;

/// skunk's error type
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "tls")]
    #[error("tls error")]
    Tls(#[from] self::protocol::tls::Error),

    #[cfg(feature = "http")]
    #[error("http error")]
    Http(#[from] self::protocol::http::Error),
}
