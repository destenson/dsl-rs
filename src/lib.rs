pub mod core;
pub mod health;
pub mod isolation;
pub mod pipeline;
pub mod recovery;
pub mod sink;
pub mod source;
pub mod stream;

pub use gstreamer::glib;

pub use core::{DslError, DslResult, init_gstreamer, init_logging};
pub use pipeline::robust_pipeline::RobustPipeline;
pub use stream::stream_manager::StreamManager;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
