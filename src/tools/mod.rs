pub mod search_docs;
pub mod crate_info;

pub use search_docs::{RustDocsSearcher, DocSearchResult};
pub use crate_info::{CrateInfoProvider, CrateInfo};