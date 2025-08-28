use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::panic;
use std::thread;

use dashmap::DashMap;
use gstreamer as gst;
use tracing::{debug, error, info, warn};

use crate::core::{DslError, DslResult, StreamState};

#[derive(Debug, Clone)]
pub struct ResourceQuota {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f32,
    pub max_threads: usize,
    pub max_file_handles: usize,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_memory_mb: 512, // 512MB per stream
            max_cpu_percent: 25.0, // 25% CPU per stream
            max_threads: 4,
            max_file_handles: 10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IsolationConfig {
    pub enable_resource_limits: bool,
    pub enable_panic_isolation: bool,
    pub enable_cpu_throttling: bool,
    pub default_quota: ResourceQuota,
    pub thread_pool_size: usize,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            enable_resource_limits: true,
            enable_panic_isolation: true,
            enable_cpu_throttling: false,
            default_quota: ResourceQuota::default(),
            thread_pool_size: 8,
        }
    }
}

#[derive(Debug)]
struct IsolatedStream {
    name: String,
    bin: gst::Bin,
    quota: ResourceQuota,
    thread_id: Option<thread::ThreadId>,
    memory_usage: Arc<Mutex<u64>>,
    cpu_usage: Arc<Mutex<f32>>,
    panic_count: Arc<Mutex<u32>>,
    last_activity: Arc<Mutex<Instant>>,
}

pub struct StreamIsolator {
    config: IsolationConfig,
    streams: Arc<DashMap<String, Arc<Mutex<IsolatedStream>>>>,
    thread_pools: Arc<DashMap<String, Vec<thread::JoinHandle<()>>>>,
    resource_monitor: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    running: Arc<Mutex<bool>>,
}

impl StreamIsolator {
    pub fn new(config: IsolationConfig) -> Self {
        // Set panic hook for isolation
        if config.enable_panic_isolation {
            Self::setup_panic_hook();
        }
        
        Self {
            config,
            streams: Arc::new(DashMap::new()),
            thread_pools: Arc::new(DashMap::new()),
            resource_monitor: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
        }
    }
    
    fn setup_panic_hook() {
        let original_hook = panic::take_hook();
        
        panic::set_hook(Box::new(move |panic_info| {
            let thread = thread::current();
            let thread_name = thread.name().unwrap_or("unknown");
            
            error!("Panic in thread '{}': {:?}", thread_name, panic_info);
            
            // Check if this is a stream thread
            if thread_name.starts_with("stream_") {
                warn!("Isolated stream panic, preventing cascade");
                // In production, would trigger recovery for just this stream
            } else {
                // Call original hook for non-stream panics
                original_hook(panic_info);
            }
        }));
    }
    
    pub fn isolate_stream(&self, name: String, bin: gst::Bin) -> DslResult<()> {
        if self.streams.contains_key(&name) {
            return Err(DslError::Other(format!("Stream {} already isolated", name)));
        }
        
        let isolated = Arc::new(Mutex::new(IsolatedStream {
            name: name.clone(),
            bin,
            quota: self.config.default_quota.clone(),
            thread_id: None,
            memory_usage: Arc::new(Mutex::new(0)),
            cpu_usage: Arc::new(Mutex::new(0.0)),
            panic_count: Arc::new(Mutex::new(0)),
            last_activity: Arc::new(Mutex::new(Instant::now())),
        }));
        
        // Create dedicated thread pool for this stream
        if self.config.enable_resource_limits {
            self.create_thread_pool(&name)?;
        }
        
        self.streams.insert(name.clone(), isolated);
        
        info!("Stream {} isolated with resource quota: {:?}", 
            name, self.config.default_quota);
        
        Ok(())
    }
    
    fn create_thread_pool(&self, stream_name: &str) -> DslResult<()> {
        let mut threads = Vec::new();
        let pool_size = self.config.default_quota.max_threads;
        
        for i in 0..pool_size {
            let name = format!("stream_{}_worker_{}", stream_name, i);
            let stream_name = stream_name.to_string();
            let streams = Arc::clone(&self.streams);
            
            let handle = thread::Builder::new()
                .name(name.clone())
                .stack_size(2 * 1024 * 1024) // 2MB stack
                .spawn(move || {
                    info!("Thread {} started", name);
                    
                    // Thread would handle stream processing tasks
                    loop {
                        thread::sleep(Duration::from_millis(100));
                        
                        // Check if stream still exists
                        if !streams.contains_key(&stream_name) {
                            break;
                        }
                    }
                    
                    info!("Thread {} terminated", name);
                })
                .map_err(|e| DslError::Other(format!("Failed to create thread: {}", e)))?;
            
            threads.push(handle);
        }
        
        self.thread_pools.insert(stream_name.to_string(), threads);
        Ok(())
    }
    
    pub fn remove_stream(&self, name: &str) -> DslResult<()> {
        // Remove stream
        let stream = self.streams.remove(name);
        
        if stream.is_none() {
            return Err(DslError::Other(format!("Stream {} not found", name)));
        }
        
        // Terminate thread pool
        if let Some((_, threads)) = self.thread_pools.remove(name) {
            // Threads will terminate when they detect stream removal
            debug!("Waiting for {} threads to terminate", threads.len());
        }
        
        info!("Stream {} removed from isolation", name);
        Ok(())
    }
    
    pub fn enforce_memory_quota(&self, stream_name: &str) -> DslResult<()> {
        if !self.config.enable_resource_limits {
            return Ok(());
        }
        
        if let Some(stream) = self.streams.get(stream_name) {
            let stream = stream.lock().unwrap();
            let usage = *stream.memory_usage.lock().unwrap();
            let limit_bytes = stream.quota.max_memory_mb * 1_048_576;
            
            if usage > limit_bytes {
                warn!("Stream {} exceeds memory quota: {}MB > {}MB",
                    stream_name, usage / 1_048_576, stream.quota.max_memory_mb);
                
                // In production, would implement actual memory limiting
                // For now, just log the violation
                return Err(DslError::ResourceExhaustion(
                    format!("Stream {} memory quota exceeded", stream_name)
                ));
            }
        }
        
        Ok(())
    }
    
    pub fn throttle_cpu(&self, stream_name: &str) -> DslResult<()> {
        if !self.config.enable_cpu_throttling {
            return Ok(());
        }
        
        if let Some(stream) = self.streams.get(stream_name) {
            let stream = stream.lock().unwrap();
            let usage = *stream.cpu_usage.lock().unwrap();
            
            if usage > stream.quota.max_cpu_percent {
                debug!("Throttling CPU for stream {}: {:.1}% > {:.1}%",
                    stream_name, usage, stream.quota.max_cpu_percent);
                
                // In production, would implement actual CPU throttling
                // using cgroups or platform-specific APIs
            }
        }
        
        Ok(())
    }
    
    pub fn handle_panic(&self, stream_name: &str) -> DslResult<RecoveryAction> {
        if let Some(stream) = self.streams.get(stream_name) {
            let stream = stream.lock().unwrap();
            let mut panic_count = stream.panic_count.lock().unwrap();
            *panic_count += 1;
            
            error!("Stream {} panicked (count: {})", stream_name, *panic_count);
            
            if *panic_count > 3 {
                // Too many panics, remove the stream
                return Ok(RecoveryAction::Remove);
            } else {
                // Try to restart
                return Ok(RecoveryAction::Restart);
            }
        }
        
        Ok(RecoveryAction::Ignore)
    }
    
    pub fn start_monitoring(&self) {
        *self.running.lock().unwrap() = true;
        
        let streams = Arc::clone(&self.streams);
        let running = Arc::clone(&self.running);
        let config = self.config.clone();
        
        let handle = thread::spawn(move || {
            while *running.lock().unwrap() {
                thread::sleep(Duration::from_secs(1));
                
                for entry in streams.iter() {
                    let stream = entry.value().lock().unwrap();

                    let mut memory = stream.memory_usage.lock().unwrap();
                    let mut cpu = stream.cpu_usage.lock().unwrap();

                    // Update last activity
                    *stream.last_activity.lock().unwrap() = Instant::now();

                    todo!("Update memory & cpu usage metrics");

                    // Implement resource monitoring logic
                    let memory = *memory;
                    let cpu = *cpu;

                    debug!("Stream {} resources - Memory: {}MB, CPU: {:.1}%",
                        entry.key(), memory / 1_048_576, cpu);
                }
            }
        });
        
        *self.resource_monitor.lock().unwrap() = Some(handle);
        info!("Resource monitoring started");
    }
    
    pub fn stop_monitoring(&self) {
        *self.running.lock().unwrap() = false;
        
        if let Some(handle) = self.resource_monitor.lock().unwrap().take() {
            let _ = handle.join();
        }
        
        info!("Resource monitoring stopped");
    }
    
    pub fn get_stream_resources(&self, name: &str) -> Option<(u64, f32)> {
        self.streams.get(name).map(|stream| {
            let stream = stream.lock().unwrap();
            let memory = *stream.memory_usage.lock().unwrap();
            let cpu = *stream.cpu_usage.lock().unwrap();
            (memory, cpu)
        })
    }
    
    pub fn set_stream_quota(&self, name: &str, quota: ResourceQuota) -> DslResult<()> {
        if let Some(stream) = self.streams.get(name) {
            let mut stream = stream.lock().unwrap();
            stream.quota = quota;
            info!("Updated resource quota for stream {}", name);
            Ok(())
        } else {
            Err(DslError::Other(format!("Stream {} not found", name)))
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RecoveryAction {
    Restart,
    Remove,
    Ignore,
}

impl Drop for StreamIsolator {
    fn drop(&mut self) {
        self.stop_monitoring();
        
        // Clean up all thread pools
        for entry in self.thread_pools.iter() {
            // Threads will terminate when they detect removal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stream_isolator_creation() {
        let config = IsolationConfig::default();
        let isolator = StreamIsolator::new(config);
        assert_eq!(isolator.streams.len(), 0);
    }
    
    #[test]
    fn test_stream_isolation() {
        gst::init().ok();
        
        let isolator = StreamIsolator::new(IsolationConfig::default());
        let bin = gst::Bin::new();
        
        let result = isolator.isolate_stream("test_stream".to_string(), bin);
        assert!(result.is_ok());
        assert_eq!(isolator.streams.len(), 1);
        
        // Try to isolate same stream again
        let bin2 = gst::Bin::new();
        let result = isolator.isolate_stream("test_stream".to_string(), bin2);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_resource_quota() {
        gst::init().ok();
        
        let isolator = StreamIsolator::new(IsolationConfig::default());
        let bin = gst::Bin::new();
        
        isolator.isolate_stream("test".to_string(), bin).unwrap();
        
        let new_quota = ResourceQuota {
            max_memory_mb: 1024,
            max_cpu_percent: 50.0,
            max_threads: 8,
            max_file_handles: 20,
        };
        
        let result = isolator.set_stream_quota("test", new_quota);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_panic_handling() {
        gst::init().ok();
        
        let isolator = StreamIsolator::new(IsolationConfig::default());
        let bin = gst::Bin::new();
        
        isolator.isolate_stream("panic_test".to_string(), bin).unwrap();
        
        // Simulate multiple panics
        for i in 1..=4 {
            let action = isolator.handle_panic("panic_test").unwrap();
            if i <= 3 {
                assert!(matches!(action, RecoveryAction::Restart));
            } else {
                assert!(matches!(action, RecoveryAction::Remove));
            }
        }
    }
}
