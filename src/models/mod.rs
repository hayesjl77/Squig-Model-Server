pub mod huggingface;
pub mod registry;

pub use huggingface::HfClient;
pub use registry::{ModelInfo, ModelRegistry};
