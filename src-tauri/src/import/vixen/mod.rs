mod constants;
mod effects;
mod importer;
pub mod preview;
mod types;

// Re-export public API — all existing `crate::import::vixen::*` paths continue to work.
pub use importer::VixenImporter;
pub use types::{
    VixenDiscovery, VixenImportConfig, VixenImportResult, VixenMediaInfo, VixenSequenceInfo,
};

// Backward compatibility: ImportError lives in the parent module now,
// but callers that used `crate::import::vixen::ImportError` still work.
pub use crate::import::ImportError;
