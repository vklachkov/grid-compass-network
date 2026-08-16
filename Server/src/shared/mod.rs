//! Helpers that carry no protocol meaning of their own: reading and writing the
//! integer and length-prefixed forms the wire is built from, walking the two
//! TLV dialects, and the error every one of them reports through.

mod error;

pub mod io;
pub mod tlv;

pub use error::FrameError;
pub use tlv::{Tlv, TlvEntry};
