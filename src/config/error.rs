use crate::error::Errorbpm;
use thiserror::Error;

#[derive(Error, Debug)]
pub(super) enum process_manager_error {
    #[error("unforeseen error happened, couldnt figure out what error")]
    UnknownError,
}
