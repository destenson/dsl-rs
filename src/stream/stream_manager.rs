use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use dashmap::DashMap;
use gstreamer as gst;
use gstreamer::prelude::*;
use tracing::{debug, error, info, warn};

use crate::core::{
    DslError, DslResult, Source, Sink, StreamState, StreamHealth
};
use crate::pipeline::robust_pipeline::RobustPipeline;

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub name: String,
    pub buffer_size: usize,
    pub max_latency: Option<u64>,
    pub enable_isolation: bool,
    pub queue_properties: QueueConfig,
}

#[derive(Debug, Clone)]
pub struct QueueConfig {
    pub max_size_buffers: u32,
    pub max_size_bytes: u32,
    pub max_size_time: u64,
    pub min_threshold_buffers: u32,
    pub leaky: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size_buffers: 200,
            max_size_bytes: 10 * 1024 * 1024, // 10MB
            max_size_time: gst::ClockTime::SECOND.nseconds(),
            min_threshold_buffers: 10,
            leaky: true,
        }
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            name: "stream".to_string(),
            buffer_size: 100,
            max_latency: Some(1000),
            enable_isolation: true,
            queue_properties: QueueConfig::default(),
        }
    }
}

pub struct StreamHandle {
    pub name: String,
    pub bin: gst::Bin,
    pub source_queue: gst::Element,
    pub sink_queue: gst::Element,
    pub health: Arc<Mutex<StreamHealth>>,
}

pub struct StreamManager {
    pipeline: Arc<RobustPipeline>,
    streams: Arc<DashMap<String, StreamHandle>>,
    active_sources: Arc<DashMap<String, Box<dyn Source>>>,
    active_sinks: Arc<DashMap<String, Box<dyn Sink>>>,
}

impl StreamManager {
    pub fn new(pipeline: Arc<RobustPipeline>) -> Self {
        Self {
            pipeline,
            streams: Arc::new(DashMap::new()),
            active_sources: Arc::new(DashMap::new()),
            active_sinks: Arc::new(DashMap::new()),
        }
    }

    pub async fn add_source(
        &self,
        mut source: Box<dyn Source>,
        config: StreamConfig,
    ) -> DslResult<String> {
        let stream_name = format!("{}_{}", config.name, uuid::Uuid::new_v4());
        
        // Create isolated bin for this stream
        let bin = gst::Bin::builder()
            .name(&stream_name)
            .build();

        // Add source element to bin
        let source_element = source.element();
        bin.add(source_element)
            .map_err(|_| DslError::Stream("Failed to add source to bin".to_string()))?;

        // Create input queue for decoupling
        let source_queue = gst::ElementFactory::make("queue")
            .name(format!("{}_queue_in", stream_name))
            .property("max-size-buffers", config.queue_properties.max_size_buffers)
            .property("max-size-bytes", config.queue_properties.max_size_bytes)
            .property("max-size-time", config.queue_properties.max_size_time)
            .property("min-threshold-buffers", config.queue_properties.min_threshold_buffers)
            .property("leaky", if config.queue_properties.leaky { 2i32 } else { 0i32 })
            .build()
            .map_err(|_| DslError::Stream("Failed to create source queue".to_string()))?;

        bin.add(&source_queue)
            .map_err(|_| DslError::Stream("Failed to add source queue to bin".to_string()))?;

        // Create output queue for sink decoupling
        let sink_queue = gst::ElementFactory::make("queue")
            .name(format!("{}_queue_out", stream_name))
            .property("max-size-buffers", config.queue_properties.max_size_buffers)
            .property("max-size-bytes", config.queue_properties.max_size_bytes)
            .property("max-size-time", config.queue_properties.max_size_time)
            .build()
            .map_err(|_| DslError::Stream("Failed to create sink queue".to_string()))?;

        bin.add(&sink_queue)
            .map_err(|_| DslError::Stream("Failed to add sink queue to bin".to_string()))?;

        // Link elements: source -> source_queue -> sink_queue
        gst::Element::link_many(&[source_element, &source_queue, &sink_queue])
            .map_err(|_| DslError::Stream("Failed to link stream elements".to_string()))?;

        // Create ghost pads for bin connectivity
        let src_pad = sink_queue.static_pad("src")
            .ok_or_else(|| DslError::Stream("No src pad on sink queue".to_string()))?;
        
        let ghost_pad = gst::GhostPad::with_target(&src_pad)
            .map_err(|_| DslError::Stream("Failed to create ghost pad".to_string()))?;
        
        ghost_pad.set_active(true)
            .map_err(|_| DslError::Stream("Failed to activate ghost pad".to_string()))?;
        
        bin.add_pad(&ghost_pad)
            .map_err(|_| DslError::Stream("Failed to add ghost pad to bin".to_string()))?;

        // Connect the source
        source.connect().await?;

        // Add to pipeline
        self.pipeline.add_stream(stream_name.clone(), bin.clone())?;

        // Create and store stream handle
        let handle = StreamHandle {
            name: stream_name.clone(),
            bin: bin.clone(),
            source_queue,
            sink_queue,
            health: Arc::new(Mutex::new(StreamHealth::new())),
        };

        self.streams.insert(stream_name.clone(), handle);
        self.active_sources.insert(stream_name.clone(), source);

        // Start the bin
        let _ = bin.set_state(gst::State::Playing);

        info!("Added source stream: {}", stream_name);
        Ok(stream_name)
    }

    pub async fn add_sink(
        &self,
        mut sink: Box<dyn Sink>,
        stream_name: &str,
    ) -> DslResult<()> {
        let stream = self.streams.get(stream_name)
            .ok_or_else(|| DslError::Stream(format!("Stream {} not found", stream_name)))?;

        // Prepare the sink
        sink.prepare().await?;

        // Add sink element to the stream's bin
        let sink_element = sink.element().clone();
        let sink_name = sink.name().to_string();
        
        stream.bin.add(&sink_element)
            .map_err(|_| DslError::Stream("Failed to add sink to bin".to_string()))?;

        // Link sink queue to sink
        stream.sink_queue.link(&sink_element)
            .map_err(|_| DslError::Stream("Failed to link sink to queue".to_string()))?;

        // Store the sink
        self.active_sinks.insert(format!("{}_{}", stream_name, sink_name), sink);

        // Sync sink state with bin
        sink_element.sync_state_with_parent()
            .map_err(|_| DslError::Stream("Failed to sync sink state".to_string()))?;

        info!("Added sink to stream: {}", stream_name);
        Ok(())
    }

    pub async fn remove_source(&self, stream_name: &str) -> DslResult<()> {
        // Get and remove the source
        let source = self.active_sources.remove(stream_name)
            .map(|(_, s)| s);

        if let Some(mut source) = source {
            // Disconnect the source
            source.disconnect().await?;
        }

        // Remove stream from pipeline
        self.pipeline.remove_stream(stream_name)?;

        // Remove from our tracking
        self.streams.remove(stream_name);

        info!("Removed source stream: {}", stream_name);
        Ok(())
    }

    pub async fn remove_sink(&self, sink_name: &str) -> DslResult<()> {
        let sink = self.active_sinks.remove(sink_name)
            .map(|(_, s)| s);

        if let Some(mut sink) = sink {
            // Cleanup the sink
            sink.cleanup().await?;
            
            // Remove sink element from pipeline
            // Note: In production, would need to properly unlink and remove
        }

        info!("Removed sink: {}", sink_name);
        Ok(())
    }

    pub fn get_stream_health(&self, stream_name: &str) -> Option<StreamHealth> {
        self.streams.get(stream_name)
            .and_then(|stream| Some(stream.health.lock().unwrap().clone()))
    }

    pub fn list_streams(&self) -> Vec<String> {
        self.streams.iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn get_stream_state(&self, stream_name: &str) -> Option<StreamState> {
        self.active_sources.get(stream_name)
            .map(|source| source.state())
    }

    pub async fn pause_stream(&self, stream_name: &str) -> DslResult<()> {
        if let Some(stream) = self.streams.get(stream_name) {
            stream.bin.set_state(gst::State::Paused)
                .map_err(|_| DslError::Stream("Failed to pause stream".to_string()))?;
            
            let mut health = stream.health.lock().unwrap();
            health.state = StreamState::Paused;
            
            info!("Paused stream: {}", stream_name);
            Ok(())
        } else {
            Err(DslError::Stream(format!("Stream {} not found", stream_name)))
        }
    }

    pub async fn resume_stream(&self, stream_name: &str) -> DslResult<()> {
        if let Some(stream) = self.streams.get(stream_name) {
            stream.bin.set_state(gst::State::Playing)
                .map_err(|_| DslError::Stream("Failed to resume stream".to_string()))?;
            
            let mut health = stream.health.lock().unwrap();
            health.state = StreamState::Running;
            
            info!("Resumed stream: {}", stream_name);
            Ok(())
        } else {
            Err(DslError::Stream(format!("Stream {} not found", stream_name)))
        }
    }

    pub async fn reconnect_source(&self, stream_name: &str) -> DslResult<()> {
        if let Some(mut source) = self.active_sources.get_mut(stream_name) {
            // Disconnect and reconnect
            source.disconnect().await?;
            source.connect().await?;
            
            info!("Reconnected source: {}", stream_name);
            Ok(())
        } else {
            Err(DslError::Stream(format!("Source {} not found", stream_name)))
        }
    }

    pub fn update_queue_config(&self, stream_name: &str, config: QueueConfig) -> DslResult<()> {
        if let Some(stream) = self.streams.get(stream_name) {
            // Update source queue properties
            stream.source_queue.set_property("max-size-buffers", config.max_size_buffers);
            stream.source_queue.set_property("max-size-bytes", config.max_size_bytes);
            stream.source_queue.set_property("max-size-time", config.max_size_time);
            stream.source_queue.set_property("min-threshold-buffers", config.min_threshold_buffers);
            
            // Update sink queue properties
            stream.sink_queue.set_property("max-size-buffers", config.max_size_buffers);
            stream.sink_queue.set_property("max-size-bytes", config.max_size_bytes);
            stream.sink_queue.set_property("max-size-time", config.max_size_time);
            
            debug!("Updated queue configuration for stream: {}", stream_name);
            Ok(())
        } else {
            Err(DslError::Stream(format!("Stream {} not found", stream_name)))
        }
    }

    pub async fn handle_stream_error(&self, stream_name: &str, error: DslError) -> DslResult<()> {
        warn!("Handling error for stream {}: {:?}", stream_name, error);
        
        if let Some(stream) = self.streams.get(stream_name) {
            let mut health = stream.health.lock().unwrap();
            health.last_error = Some(error.clone());
            health.consecutive_errors += 1;
            
            // Check if we should attempt recovery
            if health.consecutive_errors < 5 {
                health.state = StreamState::Recovering;
                drop(health); // Release lock
                
                // Attempt to reconnect the source
                if let Err(e) = self.reconnect_source(stream_name).await {
                    error!("Failed to reconnect source {}: {:?}", stream_name, e);
                    
                    let mut health = stream.health.lock().unwrap();
                    health.state = StreamState::Failed;
                    return Err(e);
                }
                
                let mut health = stream.health.lock().unwrap();
                health.state = StreamState::Running;
                health.recovery_attempts += 1;
                
                info!("Successfully recovered stream: {}", stream_name);
                Ok(())
            } else {
                health.state = StreamState::Failed;
                error!("Stream {} has failed after too many errors", stream_name);
                Err(DslError::RecoveryFailed(
                    format!("Stream {} exceeded maximum error count", stream_name)
                ))
            }
        } else {
            Err(DslError::Stream(format!("Stream {} not found", stream_name)))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_config_defaults() {
        let config = QueueConfig::default();
        assert_eq!(config.max_size_buffers, 200);
        assert_eq!(config.max_size_bytes, 10 * 1024 * 1024);
        assert!(config.leaky);
    }

    #[test]
    fn test_stream_config_defaults() {
        let config = StreamConfig::default();
        assert_eq!(config.name, "stream");
        assert_eq!(config.buffer_size, 100);
        assert!(config.enable_isolation);
    }
}