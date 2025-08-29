use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_rtsp as gst_rtsp;
use gstreamer_rtsp_server as gst_rtsp_server;
use gstreamer_rtsp_server::prelude::*;
use tracing::{debug, error, info, warn};

use crate::core::{DslError, DslResult, RecoveryAction, Sink, StreamMetrics, StreamState};

#[derive(Debug, Clone)]
pub struct RtspServerConfig {
    pub port: u16,
    pub mount_point: String,
    pub protocols: u32,
    pub max_clients: Option<u32>,
    pub enable_authentication: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub multicast_address: Option<String>,
    pub enable_rate_adaptation: bool,
    pub key_frame_interval: u32, // seconds
}

impl Default for RtspServerConfig {
    fn default() -> Self {
        Self {
            port: 8554,
            mount_point: "/stream".to_string(),
            protocols: 0x00000007, // TCP + UDP + UDP_MCAST
            max_clients: None,
            enable_authentication: false,
            username: None,
            password: None,
            multicast_address: None,
            enable_rate_adaptation: true,
            key_frame_interval: 2,
        }
    }
}

#[derive(Debug, Clone)]
struct ClientInfo {
    id: String,
    connected_at: Instant,
    address: String,
    protocol: String,
    bytes_sent: u64,
}

pub struct RtspSinkRobust {
    name: String,
    config: RtspServerConfig,
    server: Option<gst_rtsp_server::RTSPServer>,
    factory: Option<gst_rtsp_server::RTSPMediaFactory>,
    state: Arc<Mutex<StreamState>>,
    metrics: Arc<Mutex<StreamMetrics>>,
    clients: Arc<Mutex<HashMap<String, ClientInfo>>>,
    total_clients_served: Arc<Mutex<u32>>,
    sink_element: gst::Element,
}

impl RtspSinkRobust {
    pub fn new(name: String, config: RtspServerConfig) -> DslResult<Self> {
        // Create RTSP sink element
        let rtsp_sink = gst::ElementFactory::make("rtspclientsink")
            .name(format!("{name}_rtspsink"))
            .property(
                "location",
                format!("rtsp://127.0.0.1:{}{}", config.port, config.mount_point),
            )
            .build()
            .map_err(|_| DslError::Sink("Failed to create rtspclientsink".to_string()))?;

        // Set protocols using string representation for enum
        // 0x7 = TCP + UDP + UDP_MCAST, so we use combined string
        rtsp_sink.set_property_from_str("protocols", "tcp+udp+udp-mcast");

        Ok(Self {
            name,
            config,
            server: None,
            factory: None,
            state: Arc::new(Mutex::new(StreamState::Idle)),
            metrics: Arc::new(Mutex::new(StreamMetrics::default())),
            clients: Arc::new(Mutex::new(HashMap::new())),
            total_clients_served: Arc::new(Mutex::new(0)),
            sink_element: rtsp_sink,
        })
    }

    async fn setup_server(&mut self) -> DslResult<()> {
        // Create RTSP server
        let server = gst_rtsp_server::RTSPServer::new();
        server.set_service(&self.config.port.to_string());

        // Create media factory
        let factory = gst_rtsp_server::RTSPMediaFactory::new();

        // Configure factory properties
        factory.set_shared(true); // Allow multiple clients

        if let Some(max_clients) = self.config.max_clients {
            factory.set_max_mcast_ttl(max_clients);
        }

        // Set up pipeline launch string
        let launch_str = self.build_launch_string();
        factory.set_launch(&launch_str);

        // Add authentication if enabled
        if self.config.enable_authentication {
            self.setup_authentication(&server)?;
        }

        // Connect signals for client management
        self.setup_client_signals(&factory);

        // Mount the factory
        let mounts = server
            .mount_points()
            .ok_or_else(|| DslError::Sink("Failed to get mount points".to_string()))?;
        mounts.add_factory(&self.config.mount_point, factory.clone());

        // Attach server to main context
        let server_id = server.attach(None);
        if server_id.is_err() {
            return Err(DslError::Sink("Failed to attach RTSP server".to_string()));
        }

        self.server = Some(server);
        self.factory = Some(factory);

        info!(
            "RTSP server started on port {} at {}",
            self.config.port, self.config.mount_point
        );

        Ok(())
    }

    fn build_launch_string(&self) -> String {
        // Basic pipeline for receiving and serving video
        let mut launch = String::from("( ");

        // Add test source for now (in production, would receive from upstream)
        launch.push_str("videotestsrc is-live=true ! ");
        launch.push_str("video/x-raw,width=1920,height=1080,framerate=30/1 ! ");

        // Add encoder
        launch.push_str("x264enc tune=zerolatency bitrate=4000 ");
        launch.push_str(&format!(
            "key-int-max={} ! ",
            self.config.key_frame_interval * 30
        ));

        // Add RTP payloader
        launch.push_str("rtph264pay name=pay0 pt=96 ");

        launch.push(')');

        launch
    }

    fn setup_authentication(&self, _server: &gst_rtsp_server::RTSPServer) -> DslResult<()> {
        // Simplified authentication - would need proper implementation
        if self.config.enable_authentication {
            info!("Authentication requested but not implemented yet");
        }
        Ok(())
    }

    fn setup_client_signals(&self, factory: &gst_rtsp_server::RTSPMediaFactory) {
        let clients = Arc::clone(&self.clients);
        let total_served = Arc::clone(&self.total_clients_served);
        let name = self.name.clone();

        // Connect media-configure signal to track clients
        factory.connect_media_configure(move |_factory, media| {
            let clients = Arc::clone(&clients);
            let total = Arc::clone(&total_served);
            let name = name.clone();

            // Track when clients connect
            media.connect_new_stream(move |_media, stream| {
                let client_id = uuid::Uuid::new_v4().to_string();
                let client_info = ClientInfo {
                    id: client_id.clone(),
                    connected_at: Instant::now(),
                    address: "unknown".to_string(),
                    protocol: "unknown".to_string(),
                    bytes_sent: 0,
                };

                clients
                    .lock()
                    .unwrap()
                    .insert(client_id.clone(), client_info);
                *total.lock().unwrap() += 1;

                info!("New client connected to {name}: {client_id}");
            });
        });
    }

    async fn handle_client_disconnect(&self, client_id: &str) {
        if let Some(client) = self.clients.lock().unwrap().remove(client_id) {
            let duration = client.connected_at.elapsed();
            info!(
                "Client {client_id} disconnected from {} after {duration:?}",
                self.name
            );
        }
    }

    async fn adapt_bandwidth(&self) -> DslResult<()> {
        if !self.config.enable_rate_adaptation {
            return Ok(());
        }

        let client_count = self.clients.lock().unwrap().len();

        // Simple bandwidth adaptation based on client count
        if client_count > 10 {
            // Reduce quality for many clients
            debug!("Adapting bandwidth for {} clients", client_count);
            // In production, would adjust encoder bitrate
        }

        Ok(())
    }

    pub fn get_client_count(&self) -> usize {
        self.clients.lock().unwrap().len()
    }

    pub fn get_total_clients_served(&self) -> u32 {
        *self.total_clients_served.lock().unwrap()
    }

    async fn force_key_frame(&self) -> DslResult<()> {
        // Force IDR frame generation for new clients
        if let Some(factory) = &self.factory {
            debug!("Forcing key frame generation");
            // In production, would send force-key-unit event
        }
        Ok(())
    }
}

#[async_trait]
impl Sink for RtspSinkRobust {
    fn name(&self) -> &str {
        &self.name
    }

    fn element(&self) -> &gst::Element {
        &self.sink_element
    }

    async fn prepare(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Starting;

        // Setup RTSP server
        self.setup_server().await?;

        // Start sink element
        self.sink_element
            .set_state(gst::State::Playing)
            .map_err(|_| DslError::Sink("Failed to start RTSP sink".to_string()))?;

        *self.state.lock().unwrap() = StreamState::Running;

        info!(
            "RTSP sink {} ready at rtsp://localhost:{}{}",
            self.name, self.config.port, self.config.mount_point
        );

        Ok(())
    }

    async fn cleanup(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Stopped;

        // Disconnect all clients gracefully
        let client_ids: Vec<String> = self.clients.lock().unwrap().keys().cloned().collect();

        for client_id in client_ids {
            self.handle_client_disconnect(&client_id).await;
        }

        // Stop sink element
        self.sink_element
            .set_state(gst::State::Null)
            .map_err(|_| DslError::Sink("Failed to stop RTSP sink".to_string()))?;

        // Stop server
        if let Some(_server) = self.server.take() {
            // Server cleanup
            info!("RTSP server stopped for {}", self.name);
        }

        Ok(())
    }

    fn state(&self) -> StreamState {
        *self.state.lock().unwrap()
    }

    fn metrics(&self) -> StreamMetrics {
        let mut metrics = self.metrics.lock().unwrap().clone();

        // Update metrics based on client info
        let clients = self.clients.lock().unwrap();
        metrics.frames_processed = clients.len() as u64; // Using as proxy for active connections

        metrics
    }

    async fn handle_error(&mut self, error: DslError) -> DslResult<RecoveryAction> {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.errors += 1;
        }

        match error {
            DslError::Network(_) => {
                // Try to force key frame for recovery
                if let Ok(()) = self.force_key_frame().await {
                    Ok(RecoveryAction::Ignore)
                } else {
                    Ok(RecoveryAction::Restart)
                }
            }
            DslError::Sink(ref msg) if msg.contains("client") => {
                // Client-specific error, adapt bandwidth
                if let Ok(()) = self.adapt_bandwidth().await {
                    Ok(RecoveryAction::Ignore)
                } else {
                    Ok(RecoveryAction::Retry)
                }
            }
            _ => Ok(RecoveryAction::Restart),
        }
    }
}

impl Drop for RtspSinkRobust {
    fn drop(&mut self) {
        let _ = self.sink_element.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn test_rtsp_sink_creation() {
        gst::init().ok();

        let config = RtspServerConfig::default();
        let sink = RtspSinkRobust::new("test_rtsp_sink".to_string(), config);

        assert!(sink.is_ok());
        let sink = sink.unwrap();
        assert_eq!(sink.name(), "test_rtsp_sink");
        assert_eq!(sink.state(), StreamState::Idle);
    }

    #[ignore]
    #[test]
    fn test_launch_string_generation() {
        gst::init().ok();

        let config = RtspServerConfig::default();
        let sink = RtspSinkRobust::new("test".to_string(), config).unwrap();

        let launch = sink.build_launch_string();
        assert!(launch.contains("videotestsrc"));
        assert!(launch.contains("x264enc"));
        assert!(launch.contains("rtph264pay"));
    }

    #[ignore]
    #[test]
    fn test_client_tracking() {
        gst::init().ok();

        let config = RtspServerConfig::default();
        let sink = RtspSinkRobust::new("test".to_string(), config).unwrap();

        assert_eq!(sink.get_client_count(), 0);
        assert_eq!(sink.get_total_clients_served(), 0);

        // Simulate client connection
        let client_info = ClientInfo {
            id: "test_client".to_string(),
            connected_at: Instant::now(),
            address: "127.0.0.1".to_string(),
            protocol: "TCP".to_string(),
            bytes_sent: 0,
        };

        sink.clients
            .lock()
            .unwrap()
            .insert("test_client".to_string(), client_info);
        assert_eq!(sink.get_client_count(), 1);
    }
}
