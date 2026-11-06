pub mod serial;
pub mod tcp;

use std::io::{Read, Write};

/// A bidirectional byte stream whose read and write operations must be non-blocking.
pub trait Transport: Read + Write + Send + 'static {}

impl<T: Read + Write + Send + 'static> Transport for T {}
