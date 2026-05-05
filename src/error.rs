use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

/// Error type returned by all `cacache-ttl` API calls.
#[derive(Error, Debug, Diagnostic)]
pub enum Error {
    /// Returned when an index entry could not be found, or when it was found
    /// but its TTL has expired.
    #[error("Entry not found or expired for key {1:?} in cache {0:?}")]
    #[diagnostic(code(cacache_ttl::entry_not_found_or_expired), url(docsrs))]
    EntryNotFoundOrExpired(PathBuf, String),

    /// Error forwarded from `cacache`.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Cacache(#[from] cacache::Error),
}

/// The result type returned by calls to this library.
pub type Result<T> = std::result::Result<T, Error>;
