pub mod bstring;
pub mod cli;
pub mod collections;
pub mod color;
pub mod date;
pub mod error;
pub mod lockfile;
pub mod mailmap;
pub mod pager;
pub mod path;
pub mod progress;
pub mod subprocess;
pub mod tempfile;
pub mod wildmatch;

// Re-export core types at crate root for convenience
pub use bstr::{BStr, BString, ByteSlice, ByteVec};
pub use error::{LockError, UtilError};

pub type Result<T> = std::result::Result<T, UtilError>;