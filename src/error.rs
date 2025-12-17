//! Error trait extensions

#![allow(dead_code)] // Utility trait for future use

use std::error::Error;

pub trait ErrorExt: Error {
    fn print_error_stack(&self) {
        eprintln!("Error: {}", self);

        let mut source = self.source();
        while let Some(cause) = source {
            eprintln!("  Caused by: {}", cause);
            source = cause.source();
        }
    }
}

impl<T: Error> ErrorExt for T {}
