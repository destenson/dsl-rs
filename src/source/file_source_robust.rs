use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use gstreamer as gst;
use gstreamer::prelude::*;
use tracing::{debug, error, info, warn};

use crate::core::{
    DslError, DslResult, Source, StreamState, StreamMetrics, 
    RetryConfig, RecoveryAction
};

pub struct FileSourceRobust {
    name: String,
    path: PathBuf,
    element: gst::Element,
    decodebin: Option<gst::Element>,
    state: Arc<Mutex<StreamState>>,
    metrics: Arc<Mutex<StreamMetrics>>,
    retry_config: RetryConfig,
    loop_on_eof: bool,
    position: Arc<Mutex<Option<gst::ClockTime>>>,
    duration: Option<gst::ClockTime>,
    restart_count: Arc<Mutex<u32>>,
}

impl FileSourceRobust {
    pub fn new(name: String, path: PathBuf) -> DslResult<Self> {
        // Validate file exists
        if !path.exists() {
            return Err(DslError::FileIo(format!(
                "File not found: {}", 
                path.display()
            )));
        }

        // Create filesrc element
        let filesrc = gst::ElementFactory::make("filesrc")
            .name(format!("{}_filesrc", name))
            .property("location", path.to_str().unwrap())
            .build()
            .map_err(|_| DslError::Source("Failed to create filesrc".to_string()))?;

        Ok(Self {
            name,
            path,
            element: filesrc,
            decodebin: None,
            state: Arc::new(Mutex::new(StreamState::Idle)),
            metrics: Arc::new(Mutex::new(StreamMetrics::default())),
            retry_config: RetryConfig::default(),
            loop_on_eof: true,
            position: Arc::new(Mutex::new(None)),
            duration: None,
            restart_count: Arc::new(Mutex::new(0)),
        })
    }

    pub fn set_loop_on_eof(&mut self, enable: bool) {
        self.loop_on_eof = enable;
    }

    async fn validate_file(&self) -> DslResult<()> {
        // Check file still exists
        if !self.path.exists() {
            return Err(DslError::FileIo(format!(
                "File no longer exists: {}", 
                self.path.display()
            )));
        }

        // Check file is readable
        match std::fs::File::open(&self.path) {
            Ok(_) => Ok(()),
            Err(e) => Err(DslError::FileIo(format!(
                "Cannot read file {}: {}", 
                self.path.display(), e
            )))
        }
    }

    async fn setup_decoding(&mut self) -> DslResult<()> {
        // Create decodebin for automatic decoding
        let decodebin = gst::ElementFactory::make("decodebin")
            .name(format!("{}_decodebin", self.name))
            .build()
            .map_err(|_| DslError::Source("Failed to create decodebin".to_string()))?;

        // Connect pad-added signal for dynamic linking
        let name = self.name.clone();
        decodebin.connect_pad_added(move |_dbin, src_pad| {
            debug!("New pad added for {}", name);
            // In production, would link to appropriate downstream element
        });

        self.decodebin = Some(decodebin);
        Ok(())
    }

    async fn handle_eof(&mut self) -> DslResult<()> {
        if self.loop_on_eof {
            info!("EOF reached for {}, restarting from beginning", self.name);
            
            // Increment restart count
            *self.restart_count.lock().unwrap() += 1;
            
            // Seek to beginning
            self.element.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::ZERO
            ).map_err(|_| DslError::Source("Failed to seek to beginning".to_string()))?;
            
            // Update position
            *self.position.lock().unwrap() = Some(gst::ClockTime::ZERO);
            
            Ok(())
        } else {
            info!("EOF reached for {}, stopping", self.name);
            *self.state.lock().unwrap() = StreamState::Stopped;
            Err(DslError::Source("End of file reached".to_string()))
        }
    }

    fn update_position(&self) -> DslResult<()> {
        if let Some(position) = self.element.query_position::<gst::ClockTime>() {
            *self.position.lock().unwrap() = Some(position);
            
            // Update metrics
            let mut metrics = self.metrics.lock().unwrap();
            if let Some(last_time) = metrics.last_frame_time {
                let elapsed = Instant::now().duration_since(last_time);
                if elapsed > Duration::ZERO {
                    metrics.fps = 1.0 / elapsed.as_secs_f64();
                }
            }
            metrics.last_frame_time = Some(Instant::now());
            metrics.frames_processed += 1;
        }
        Ok(())
    }

    async fn recover_from_error(&mut self, error: &DslError) -> DslResult<()> {
        warn!("Attempting to recover from error: {:?}", error);
        
        // Stop current playback
        self.element.set_state(gst::State::Null)
            .map_err(|_| DslError::Source("Failed to stop element".to_string()))?;
        
        // Validate file still exists
        self.validate_file().await?;
        
        // Restart from last position or beginning
        let seek_position = self.position.lock().unwrap().unwrap_or(gst::ClockTime::ZERO);
        
        // Set back to playing
        self.element.set_state(gst::State::Playing)
            .map_err(|_| DslError::Source("Failed to restart element".to_string()))?;
        
        // Seek to position
        self.element.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            seek_position
        ).map_err(|_| DslError::Source("Failed to seek to position".to_string()))?;
        
        info!("Successfully recovered file source {} at position {:?}", 
            self.name, seek_position);
        
        Ok(())
    }

    pub fn get_restart_count(&self) -> u32 {
        *self.restart_count.lock().unwrap()
    }

    pub fn get_position(&self) -> Option<gst::ClockTime> {
        *self.position.lock().unwrap()
    }
}

#[async_trait]
impl Source for FileSourceRobust {
    fn name(&self) -> &str {
        &self.name
    }

    fn element(&self) -> &gst::Element {
        &self.element
    }

    async fn connect(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Starting;
        
        // Validate file before playing
        self.validate_file().await?;
        
        // Setup decoding if needed
        if self.decodebin.is_none() {
            self.setup_decoding().await?;
        }
        
        // Query duration
        if let Some(duration) = self.element.query_duration::<gst::ClockTime>() {
            self.duration = Some(duration);
            info!("File {} duration: {:?}", self.name, duration);
        }
        
        // Set to playing state
        self.element.set_state(gst::State::Playing)
            .map_err(|_| DslError::Source("Failed to start file source".to_string()))?;
        
        *self.state.lock().unwrap() = StreamState::Running;
        info!("File source {} connected and playing", self.name);
        
        Ok(())
    }

    async fn disconnect(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Stopped;
        
        // Stop the element
        self.element.set_state(gst::State::Null)
            .map_err(|_| DslError::Source("Failed to stop file source".to_string()))?;
        
        info!("File source {} disconnected", self.name);
        Ok(())
    }

    fn state(&self) -> StreamState {
        *self.state.lock().unwrap()
    }

    fn metrics(&self) -> StreamMetrics {
        self.metrics.lock().unwrap().clone()
    }

    fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config;
    }

    async fn handle_error(&mut self, error: DslError) -> DslResult<RecoveryAction> {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.errors += 1;
        }
        
        match error {
            DslError::Source(ref msg) if msg.contains("End of file") => {
                if self.loop_on_eof {
                    self.handle_eof().await?;
                    Ok(RecoveryAction::Ignore)
                } else {
                    Ok(RecoveryAction::Remove)
                }
            }
            DslError::FileIo(_) => {
                // File might have been deleted or become unreadable
                Ok(RecoveryAction::Retry)
            }
            _ => {
                // Try to recover from other errors
                if let Ok(()) = self.recover_from_error(&error).await {
                    Ok(RecoveryAction::Ignore)
                } else {
                    Ok(RecoveryAction::Restart)
                }
            }
        }
    }
}

impl Drop for FileSourceRobust {
    fn drop(&mut self) {
        let _ = self.element.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_source_creation() {
        gst::init().ok();
        
        // Create a temporary file
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.mp4");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "test content").unwrap();
        
        let source = FileSourceRobust::new(
            "test_source".to_string(),
            file_path
        );
        
        assert!(source.is_ok());
        let source = source.unwrap();
        assert_eq!(source.name(), "test_source");
        assert_eq!(source.state(), StreamState::Idle);
    }

    #[tokio::test]
    async fn test_file_not_found() {
        gst::init().ok();
        
        let source = FileSourceRobust::new(
            "test_source".to_string(),
            PathBuf::from("/non/existent/file.mp4")
        );
        
        assert!(source.is_err());
    }

    #[test]
    fn test_restart_count() {
        gst::init().ok();
        
        // Create a temporary file
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.mp4");
        File::create(&file_path).unwrap();
        
        let source = FileSourceRobust::new(
            "test_source".to_string(),
            file_path
        ).unwrap();
        
        assert_eq!(source.get_restart_count(), 0);
        *source.restart_count.lock().unwrap() += 1;
        assert_eq!(source.get_restart_count(), 1);
    }
}