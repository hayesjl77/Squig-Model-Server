pub mod engine;
pub mod hardware;
pub mod smart_defaults;
pub mod types;

pub use engine::InferenceManager;
pub use hardware::detect_hardware;
pub use smart_defaults::compute_smart_settings;
pub use types::*;
