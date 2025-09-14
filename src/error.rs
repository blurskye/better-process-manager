use std::error::Error;

pub trait Errorbpm: Error {
    fn print_error_stack(&self) {
        eprintln!("Error: {}", self);

        let mut source = self.source();
        while let Some(cause) = source {
            eprintln!("  Caused by: {}", cause);
            source = cause.source();
        }
    }
}

impl<T: Error> Errorbpm for T {}
