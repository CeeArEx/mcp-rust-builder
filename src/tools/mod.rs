pub mod search_docs;
pub mod crate_info;
pub mod cargo_check; // New
pub mod explain;     // New
pub mod project;     // New

pub use search_docs::RustDocsSearcher;
pub use crate_info::CrateInfoProvider;
pub use cargo_check::CargoChecker;
pub use explain::ErrorExplainer;
pub use project::ProjectManager;