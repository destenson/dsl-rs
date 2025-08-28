//! Common test utilities for DSL-RS
//! 
//! This module provides shared utilities for testing including:
//! - GStreamer initialization and cleanup
//! - Mock sources and sinks
//! - Fixture data generators
//! - Async assertion helpers

use std::sync::{Arc, Mutex, Once};
use std::time::Duration;
use std::path::PathBuf;
use std::collections::HashMap;

use gstreamer as gst;
use gstreamer::prelude::*;
use dsl_rs::core::*;
use dsl_rs::pipeline::*;
use dsl_rs::source::*;
use dsl_rs::sink::*;
use async_trait::async_trait;
use uuid::Uuid;

static INIT: Once = Once::new();

/// Initialize GStreamer once for all tests
pub fn init_gstreamer() {
    INIT.call_once(|| {
        gst::init().expect("Failed to initialize GStreamer");
        // Set GST_DEBUG for test debugging if needed
        std::env::set_var("GST_DEBUG", "2");
    });
}

/// Clean up function to ensure proper resource cleanup
pub fn cleanup_pipeline(pipeline: &gst::Pipeline) {
    let _ = pipeline.set_state(gst::State::Null);
    // Wait for state change to complete
    let _ = pipeline.state(Some(gst::ClockTime::from_seconds(1)));
}

/// Test fixture for creating temporary test files
pub struct TestFixture {
    pub temp_dir: tempfile::TempDir,
    pub files: Vec<PathBuf>,
}

impl TestFixture {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        Self {
            temp_dir,
            files: Vec::new(),
        }
    }

    pub fn create_test_file(&mut self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        std::fs::write(&path, content).expect("Failed to write test file");
        self.files.push(path.clone());
        path
    }

    pub fn create_test_video(&mut self, name: &str) -> PathBuf {
        // Create a simple test video using GStreamer videotestsrc
        let path = self.temp_dir.path().join(name);
        
        let pipeline_str = format!(
            "videotestsrc num-buffers=100 ! x264enc ! mp4mux ! filesink location={}",
            path.display()
        );
        
        let pipeline = gst::parse::launch(&pipeline_str)
            .expect("Failed to create test video pipeline");
        
        pipeline.set_state(gst::State::Playing).expect("Failed to start pipeline");
        
        // Wait for EOS
        let bus = pipeline.bus().unwrap();
        for msg in bus.iter_timed(gst::ClockTime::from_seconds(10)) {
            if let gst::MessageView::Eos(..) = msg.view() {
                break;
            }
        }
        
        pipeline.set_state(gst::State::Null).expect("Failed to stop pipeline");
        
        self.files.push(path.clone());
        path
    }
}

/// Mock source for testing
pub struct MockSource {
    pub name: String,
    pub state: Arc<Mutex<SourceState>>,
    pub connect_count: Arc<Mutex<usize>>,
    pub should_fail: Arc<Mutex<bool>>,
    pub fail_after: Arc<Mutex<Option<usize>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SourceState {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

impl MockSource {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: Arc::new(Mutex::new(SourceState::Disconnected)),
            connect_count: Arc::new(Mutex::new(0)),
            should_fail: Arc::new(Mutex::new(false)),
            fail_after: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }

    pub fn set_fail_after(&self, count: usize) {
        *self.fail_after.lock().unwrap() = Some(count);
    }

    pub fn get_connect_count(&self) -> usize {
        *self.connect_count.lock().unwrap()
    }
}

#[async_trait]
impl Source for MockSource {
    async fn connect(&self) -> DslResult<()> {
        let mut count = self.connect_count.lock().unwrap();
        *count += 1;
        
        let should_fail = *self.should_fail.lock().unwrap();
        let fail_after = *self.fail_after.lock().unwrap();
        
        if should_fail || fail_after.map_or(false, |fa| *count > fa) {
            *self.state.lock().unwrap() = SourceState::Failed;
            return Err(DslError::Connection(format!("Mock source {} failed", self.name)));
        }
        
        *self.state.lock().unwrap() = SourceState::Connected;
        Ok(())
    }

    async fn disconnect(&self) -> DslResult<()> {
        *self.state.lock().unwrap() = SourceState::Disconnected;
        Ok(())
    }

    fn create_element(&self) -> DslResult<gst::Element> {
        let src = gst::ElementFactory::make("videotestsrc")
            .name(&format!("mock_source_{}", self.name))
            .build()
            .map_err(|e| DslError::GStreamer(e.to_string()))?;
        
        src.set_property("is-live", true);
        src.set_property("pattern", 0i32); // SMPTE test pattern
        
        Ok(src)
    }

    fn handle_error(&self, _error: &DslError) -> DslResult<()> {
        *self.state.lock().unwrap() = SourceState::Failed;
        Ok(())
    }
}

/// Mock sink for testing
pub struct MockSink {
    pub name: String,
    pub state: Arc<Mutex<SinkState>>,
    pub frames_received: Arc<Mutex<usize>>,
    pub should_fail: Arc<Mutex<bool>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SinkState {
    Idle,
    Preparing,
    Ready,
    Failed,
}

impl MockSink {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: Arc::new(Mutex::new(SinkState::Idle)),
            frames_received: Arc::new(Mutex::new(0)),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }

    pub fn get_frames_received(&self) -> usize {
        *self.frames_received.lock().unwrap()
    }
}

#[async_trait]
impl Sink for MockSink {
    async fn prepare(&self) -> DslResult<()> {
        if *self.should_fail.lock().unwrap() {
            *self.state.lock().unwrap() = SinkState::Failed;
            return Err(DslError::Sink(format!("Mock sink {} failed to prepare", self.name)));
        }
        
        *self.state.lock().unwrap() = SinkState::Ready;
        Ok(())
    }

    async fn cleanup(&self) -> DslResult<()> {
        *self.state.lock().unwrap() = SinkState::Idle;
        Ok(())
    }

    fn create_element(&self) -> DslResult<gst::Element> {
        let sink = gst::ElementFactory::make("fakesink")
            .name(&format!("mock_sink_{}", self.name))
            .build()
            .map_err(|e| DslError::GStreamer(e.to_string()))?;
        
        sink.set_property("sync", false);
        
        // Add probe to count frames
        let frames = Arc::clone(&self.frames_received);
        if let Some(pad) = sink.static_pad("sink") {
            pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                *frames.lock().unwrap() += 1;
                gst::PadProbeReturn::Ok
            });
        }
        
        Ok(sink)
    }

    fn handle_error(&self, _error: &DslError) -> DslResult<()> {
        *self.state.lock().unwrap() = SinkState::Failed;
        Ok(())
    }
}

/// Async assertion helpers
pub mod assertions {
    use std::time::{Duration, Instant};
    use std::future::Future;
    use futures::future;

    /// Wait for a condition to become true with timeout
    pub async fn wait_for<F>(mut condition: F, timeout: Duration) -> bool 
    where
        F: FnMut() -> bool,
    {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    /// Assert that an async operation completes within timeout
    pub async fn assert_completes_within<F>(future: F, timeout: Duration) -> F::Output
    where
        F: Future,
    {
        tokio::time::timeout(timeout, future)
            .await
            .expect("Operation timed out")
    }

    /// Assert that a condition eventually becomes true
    pub async fn assert_eventually<F>(condition: F, timeout: Duration, message: &str)
    where
        F: FnMut() -> bool,
    {
        assert!(
            wait_for(condition, timeout).await,
            "{}", message
        );
    }

    /// Assert that a condition remains true for a duration
    pub async fn assert_remains<F>(mut condition: F, duration: Duration, message: &str)
    where
        F: FnMut() -> bool,
    {
        let start = Instant::now();
        while start.elapsed() < duration {
            assert!(condition(), "{}", message);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// Test configuration builder
pub struct TestConfigBuilder {
    config: HashMap<String, serde_json::Value>,
}

impl TestConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: HashMap::new(),
        }
    }

    pub fn with_retry_config(mut self, max_attempts: u32, initial_delay: u64) -> Self {
        let retry = serde_json::json!({
            "max_attempts": max_attempts,
            "initial_delay_ms": initial_delay,
            "max_delay_ms": 60000,
            "exponential_base": 2.0
        });
        self.config.insert("retry".to_string(), retry);
        self
    }

    pub fn with_watchdog(mut self, timeout_secs: u64) -> Self {
        self.config.insert("watchdog_timeout_secs".to_string(), serde_json::json!(timeout_secs));
        self
    }

    pub fn with_circuit_breaker(mut self, failure_threshold: u32, reset_timeout: u64) -> Self {
        let circuit_breaker = serde_json::json!({
            "failure_threshold": failure_threshold,
            "reset_timeout_secs": reset_timeout,
            "half_open_success_threshold": 2
        });
        self.config.insert("circuit_breaker".to_string(), circuit_breaker);
        self
    }

    pub fn build_pipeline_config(self) -> PipelineConfig {
        PipelineConfig {
            watchdog_timeout: self.config
                .get("watchdog_timeout_secs")
                .and_then(|v| v.as_u64())
                .map(Duration::from_secs),
            ..Default::default()
        }
    }

    pub fn build_retry_config(self) -> RetryConfig {
        if let Some(retry) = self.config.get("retry") {
            let max_attempts = retry.get("max_attempts")
                .and_then(|v| v.as_u64())
                .unwrap_or(3) as u32;
            let initial_delay = retry.get("initial_delay_ms")
                .and_then(|v| v.as_u64())
                .map(Duration::from_millis)
                .unwrap_or_else(|| Duration::from_millis(1000));
            
            RetryConfig {
                max_attempts,
                initial_delay,
                max_delay: Duration::from_secs(60),
                exponential_base: 2.0,
            }
        } else {
            RetryConfig::default()
        }
    }
}

/// Performance measurement utilities
pub struct PerformanceMonitor {
    start_time: Instant,
    checkpoints: Vec<(String, Instant)>,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            checkpoints: Vec::new(),
        }
    }

    pub fn checkpoint(&mut self, name: &str) {
        self.checkpoints.push((name.to_string(), Instant::now()));
    }

    pub fn report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!("Total time: {:?}\n", self.start_time.elapsed()));
        
        let mut last_time = self.start_time;
        for (name, time) in &self.checkpoints {
            report.push_str(&format!("{}: {:?}\n", name, time.duration_since(last_time)));
            last_time = *time;
        }
        
        report
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Generate test stream configurations
pub fn generate_test_streams(count: usize, prefix: &str) -> Vec<(String, MockSource, MockSink)> {
    (0..count)
        .map(|i| {
            let name = format!("{}_{}", prefix, i);
            let source = MockSource::new(&format!("{}_source", name));
            let sink = MockSink::new(&format!("{}_sink", name));
            (name, source, sink)
        })
        .collect()
}

/// Create a test pipeline with mock elements
pub fn create_test_pipeline(name: &str) -> DslResult<gst::Pipeline> {
    init_gstreamer();
    
    let pipeline = gst::Pipeline::builder()
        .name(name)
        .build();
    
    // Add a simple test source and sink
    let source = gst::ElementFactory::make("videotestsrc")
        .name(&format!("{}_source", name))
        .property("is-live", true)
        .property("num-buffers", 100i32)
        .build()
        .map_err(|e| DslError::GStreamer(e.to_string()))?;
    
    let sink = gst::ElementFactory::make("fakesink")
        .name(&format!("{}_sink", name))
        .property("sync", false)
        .build()
        .map_err(|e| DslError::GStreamer(e.to_string()))?;
    
    pipeline.add_many(&[&source, &sink])
        .map_err(|e| DslError::GStreamer(e.to_string()))?;
    
    source.link(&sink)
        .map_err(|e| DslError::GStreamer(e.to_string()))?;
    
    Ok(pipeline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gstreamer_init() {
        init_gstreamer();
        assert!(gst::version() >= (1, 0, 0, 0));
    }

    #[test]
    fn test_mock_source() {
        let source = MockSource::new("test");
        assert_eq!(source.get_connect_count(), 0);
        
        // Test that element can be created
        let element = source.create_element();
        assert!(element.is_ok());
    }

    #[test]
    fn test_mock_sink() {
        let sink = MockSink::new("test");
        assert_eq!(sink.get_frames_received(), 0);
        
        // Test that element can be created
        let element = sink.create_element();
        assert!(element.is_ok());
    }

    #[test]
    fn test_fixture_creation() {
        let mut fixture = TestFixture::new();
        let path = fixture.create_test_file("test.txt", b"test content");
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), b"test content");
    }

    #[tokio::test]
    async fn test_async_assertions() {
        use assertions::*;
        
        let mut counter = 0;
        let result = wait_for(
            || {
                counter += 1;
                counter > 5
            },
            Duration::from_millis(100)
        ).await;
        
        assert!(result);
        assert!(counter > 5);
    }

    #[test]
    fn test_config_builder() {
        let config = TestConfigBuilder::new()
            .with_retry_config(5, 1000)
            .with_watchdog(30)
            .build_pipeline_config();
        
        assert_eq!(config.watchdog_timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_performance_monitor() {
        let mut monitor = PerformanceMonitor::new();
        std::thread::sleep(Duration::from_millis(10));
        monitor.checkpoint("step1");
        std::thread::sleep(Duration::from_millis(10));
        monitor.checkpoint("step2");
        
        let report = monitor.report();
        assert!(report.contains("Total time"));
        assert!(report.contains("step1"));
        assert!(report.contains("step2"));
        assert!(monitor.elapsed() >= Duration::from_millis(20));
    }
}