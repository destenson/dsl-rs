pub mod core;
pub mod health;
pub mod isolation;
pub mod pipeline;
pub mod recovery;
pub mod sink;
pub mod source;
pub mod stream;

pub use gstreamer::glib;

pub use core::{init_gstreamer, init_logging, DslError, DslResult};
pub use pipeline::robust_pipeline::RobustPipeline;
pub use stream::stream_manager::StreamManager;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
