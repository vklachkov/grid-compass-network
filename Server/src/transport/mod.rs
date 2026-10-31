pub mod tcp;

use std::io::{Read, Write};

pub trait Transport: Read + Write + Send + 'static {}

impl<T: Read + Write + Send + 'static> Transport for T {}
