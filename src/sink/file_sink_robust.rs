use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use gstreamer as gst;
use gstreamer::prelude::*;
use tracing::{debug, error, info, warn};

use crate::core::{DslError, DslResult, RecoveryAction, Sink, StreamMetrics, StreamState};

#[derive(Debug, Clone)]
pub struct RotationConfig {
    pub enable_size_rotation: bool,
    pub max_file_size: u64, // bytes
    pub enable_time_rotation: bool,
    pub rotation_interval: Duration,
    pub max_files: Option<usize>,
    pub base_filename: String,
    pub directory: PathBuf,
}

impl Default for RotationConfig {
    fn default() -> Self {
        Self {
            enable_size_rotation: true,
            max_file_size: 100 * 1024 * 1024, // 100MB
            enable_time_rotation: false,
            rotation_interval: Duration::from_secs(3600), // 1 hour
            max_files: Some(10),
            base_filename: "recording".to_string(),
            directory: PathBuf::from("."),
        }
    }
}

pub struct FileSinkRobust {
    name: String,
    config: RotationConfig,
    filesink: gst::Element,
    mux: gst::Element,
    state: Arc<Mutex<StreamState>>,
    metrics: Arc<Mutex<StreamMetrics>>,
    current_file: Arc<Mutex<Option<PathBuf>>>,
    current_file_size: Arc<Mutex<u64>>,
    rotation_start_time: Arc<Mutex<Instant>>,
    file_count: Arc<Mutex<u32>>,
    bytes_written: Arc<Mutex<u64>>,
}

impl FileSinkRobust {
    pub fn new(name: String, config: RotationConfig) -> DslResult<Self> {
        // Ensure directory exists
        fs::create_dir_all(&config.directory)
            .map_err(|e| DslError::FileIo(format!("Failed to create directory: {e}")))?;

        // Create filesink element
        let filesink = gst::ElementFactory::make("filesink")
            .name(format!("{name}_filesink"))
            .property("sync", false)
            .property("async", false)
            .build()
            .map_err(|_| DslError::Sink("Failed to create filesink".to_string()))?;

        // Create muxer (MP4 by default)
        let mux = gst::ElementFactory::make("mp4mux")
            .name(format!("{name}_mux"))
            .property("fragment-duration", 1000u32) // 1 second fragments
            .property("streamable", true)
            .build()
            .map_err(|_| DslError::Sink("Failed to create mp4mux".to_string()))?;

        Ok(Self {
            name,
            config,
            filesink,
            mux,
            state: Arc::new(Mutex::new(StreamState::Idle)),
            metrics: Arc::new(Mutex::new(StreamMetrics::default())),
            current_file: Arc::new(Mutex::new(None)),
            current_file_size: Arc::new(Mutex::new(0)),
            rotation_start_time: Arc::new(Mutex::new(Instant::now())),
            file_count: Arc::new(Mutex::new(0)),
            bytes_written: Arc::new(Mutex::new(0)),
        })
    }

    fn generate_filename(&self) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let count = *self.file_count.lock().unwrap();
        let filename = format!(
            "{}_{}_{}_{}.mp4",
            self.config.base_filename, self.name, timestamp, count
        );

        self.config.directory.join(filename)
    }

    async fn rotate_file(&mut self) -> DslResult<()> {
        info!("Rotating file for sink {}", self.name);

        // Stop current recording
        self.filesink
            .set_state(gst::State::Ready)
            .map_err(|_| DslError::Sink("Failed to pause filesink for rotation".to_string()))?;

        // Clean up old files if max_files is set
        if let Some(max_files) = self.config.max_files {
            self.cleanup_old_files(max_files).await?;
        }

        // Generate new filename
        let new_file = self.generate_filename();

        // Set new location
        self.filesink
            .set_property("location", new_file.to_str().unwrap());

        // Update state
        *self.current_file.lock().unwrap() = Some(new_file.clone());
        *self.current_file_size.lock().unwrap() = 0;
        *self.rotation_start_time.lock().unwrap() = Instant::now();
        *self.file_count.lock().unwrap() += 1;

        // Restart recording
        self.filesink
            .set_state(gst::State::Playing)
            .map_err(|_| DslError::Sink("Failed to restart filesink after rotation".to_string()))?;

        info!("Rotated to new file: {:?}", new_file);
        Ok(())
    }

    async fn cleanup_old_files(&self, max_files: usize) -> DslResult<()> {
        let pattern = format!("{}_{}_*.mp4", self.config.base_filename, self.name);
        let mut files = Vec::new();

        // Find all matching files
        if let Ok(entries) = fs::read_dir(&self.config.directory) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if let Some(filename) = path.file_name() {
                    let filename_str = filename.to_string_lossy();
                    if filename_str
                        .starts_with(&format!("{}_{}", self.config.base_filename, self.name))
                        && filename_str.ends_with(".mp4")
                    {
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(created) = metadata.created() {
                                files.push((path, created));
                            }
                        }
                    }
                }
            }
        }

        // Sort by creation time (oldest first)
        files.sort_by(|a, b| a.1.cmp(&b.1));

        // Remove oldest files if we exceed max_files
        while files.len() > max_files {
            let (path, _) = files.remove(0);
            info!("Removing old recording: {:?}", path);
            let _ = fs::remove_file(path);
        }

        Ok(())
    }

    async fn check_rotation_needed(&self) -> bool {
        let mut needs_rotation = false;

        // Check size-based rotation
        if self.config.enable_size_rotation {
            let current_size = *self.current_file_size.lock().unwrap();
            if current_size >= self.config.max_file_size {
                debug!(
                    "File size {current_size} exceeds max {}, rotating",
                    self.config.max_file_size
                );
                needs_rotation = true;
            }
        }

        // Check time-based rotation
        if self.config.enable_time_rotation {
            let elapsed = self.rotation_start_time.lock().unwrap().elapsed();
            if elapsed >= self.config.rotation_interval {
                debug!(
                    "Time elapsed {elapsed:?} exceeds interval {:?}, rotating",
                    self.config.rotation_interval
                );
                needs_rotation = true;
            }
        }

        needs_rotation
    }

    async fn check_disk_space(&self) -> DslResult<()> {
        // Platform-specific disk space check would go here
        // For now, just ensure directory is writable
        let test_file = self.config.directory.join(".write_test");
        match fs::File::create(&test_file) {
            Ok(_) => {
                let _ = fs::remove_file(test_file);
                Ok(())
            }
            Err(e) => Err(DslError::FileIo(format!("Cannot write to directory: {e}"))),
        }
    }

    pub fn get_current_file(&self) -> Option<PathBuf> {
        self.current_file.lock().unwrap().clone()
    }

    pub fn get_bytes_written(&self) -> u64 {
        *self.bytes_written.lock().unwrap()
    }

    async fn handle_write_error(&mut self, error: &str) -> DslResult<()> {
        error!("Write error for sink {}: {error}", self.name);

        // Check if it's a disk space issue
        if error.contains("space") || error.contains("full") {
            return Err(DslError::ResourceExhaustion(
                "Disk space exhausted".to_string(),
            ));
        }

        // Try to recover by creating a new file
        self.rotate_file().await?;
        Ok(())
    }
}

#[async_trait]
impl Sink for FileSinkRobust {
    fn name(&self) -> &str {
        &self.name
    }

    fn element(&self) -> &gst::Element {
        &self.filesink
    }

    async fn prepare(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Starting;

        // Check disk space
        self.check_disk_space().await?;

        // Set initial filename
        let filename = self.generate_filename();
        self.filesink
            .set_property("location", filename.to_str().unwrap());
        *self.current_file.lock().unwrap() = Some(filename.clone());

        // Start the sink
        self.filesink
            .set_state(gst::State::Playing)
            .map_err(|_| DslError::Sink("Failed to start file sink".to_string()))?;

        *self.state.lock().unwrap() = StreamState::Running;
        info!(
            "File sink {} prepared, writing to {:?}",
            self.name, filename
        );

        Ok(())
    }

    async fn cleanup(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Stopped;

        // Stop the sink
        self.filesink
            .set_state(gst::State::Null)
            .map_err(|_| DslError::Sink("Failed to stop file sink".to_string()))?;

        // Finalize current file
        if let Some(current) = self.current_file.lock().unwrap().as_ref() {
            info!("Finalized recording: {:?}", current);
        }

        Ok(())
    }

    fn state(&self) -> StreamState {
        *self.state.lock().unwrap()
    }

    fn metrics(&self) -> StreamMetrics {
        let mut metrics = self.metrics.lock().unwrap().clone();
        metrics.bitrate =
            (*self.bytes_written.lock().unwrap() * 8) / (metrics.uptime.as_secs() + 1); // Avoid division by zero
        metrics
    }

    async fn handle_error(&mut self, error: DslError) -> DslResult<RecoveryAction> {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.errors += 1;
        }

        match error {
            DslError::FileIo(ref msg) => {
                if let Ok(()) = self.handle_write_error(msg).await {
                    Ok(RecoveryAction::Ignore)
                } else {
                    Ok(RecoveryAction::Restart)
                }
            }
            DslError::ResourceExhaustion(_) => {
                warn!("Resource exhaustion, removing sink {}", self.name);
                Ok(RecoveryAction::Remove)
            }
            _ => Ok(RecoveryAction::Retry),
        }
    }
}

impl Drop for FileSinkRobust {
    fn drop(&mut self) {
        let _ = self.filesink.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_sink_creation() {
        gst::init().ok();

        let dir = tempdir().unwrap();
        let config = RotationConfig {
            directory: dir.path().to_path_buf(),
            ..Default::default()
        };

        let sink = FileSinkRobust::new("test_sink".to_string(), config);
        assert!(sink.is_ok());

        let sink = sink.unwrap();
        assert_eq!(sink.name(), "test_sink");
        assert_eq!(sink.state(), StreamState::Idle);
    }

    #[test]
    fn test_filename_generation() {
        gst::init().ok();

        let config = RotationConfig::default();
        let sink = FileSinkRobust::new("test".to_string(), config).unwrap();

        let filename1 = sink.generate_filename();
        // Increment the file counter to ensure different filenames
        *sink.file_count.lock().unwrap() += 1;
        let filename2 = sink.generate_filename();

        assert_ne!(filename1, filename2);
        assert!(filename1.to_string_lossy().contains("recording_test"));
    }

    #[tokio::test]
    async fn test_disk_space_check() {
        gst::init().ok();

        let dir = tempdir().unwrap();
        let mut config = RotationConfig {
            directory: dir.path().to_path_buf(),
            ..Default::default()
        };

        let sink = FileSinkRobust::new("test".to_string(), config).unwrap();
        let result = sink.check_disk_space().await;
        assert!(result.is_ok());
    }
}
