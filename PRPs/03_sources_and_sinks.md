# PRP: DSL-RS Sources and Sinks Implementation

## Executive Summary

This PRP implements the source and sink components for dsl-rs, enabling pipelines to ingest video from various inputs (files, RTSP, USB cameras, CSI cameras) and output processed streams to displays, files, or network endpoints. These components form the input/output boundaries of any video analytics pipeline.

## Problem Statement

### Current State
- Pipeline infrastructure exists but cannot ingest or output video
- No abstractions for different source types (file, stream, camera)
- No sink implementations for display or recording
- Cannot build functional end-to-end pipelines

### Desired State
- Complete set of source types matching DSL functionality
- Multiple sink types for different output scenarios
- Automatic format negotiation and conversion
- Dynamic source switching and sink management
- Smart recording capabilities with triggered start/stop

### Business Value
- Enables real-world video analytics applications
- Supports diverse input sources from files to live cameras
- Provides flexible output options for different use cases
- Allows recording based on analytics events

## Requirements

### Functional Requirements

1. **File Sources**: Support for MP4, MKV, H264, H265 video files
2. **Stream Sources**: RTSP and HTTP stream ingestion
3. **Camera Sources**: USB (V4L2) and CSI camera support
4. **App Sources**: Custom application data injection
5. **Window Sinks**: Native window rendering (X11, Wayland)
6. **File Sinks**: Encoding and saving to video files
7. **Stream Sinks**: RTSP server and WebRTC output
8. **Smart Recording**: Event-triggered recording with pre/post buffers
9. **Fake Sinks**: For testing and benchmarking

### Non-Functional Requirements

1. **Performance**: Minimal latency for live sources
2. **Reliability**: Automatic reconnection for network sources
3. **Compatibility**: Support common video formats and codecs
4. **Resource Management**: Proper buffer pool management
5. **Error Handling**: Graceful degradation on source failure

### Context and Research

Sources and sinks in DeepStream use specialized elements like nvstreammux for batching and nvvideoconvert for format conversion. The implementation must handle these DeepStream-specific requirements while maintaining compatibility with standard GStreamer elements.

### Documentation & References
```yaml
# MUST READ - Include these in your context window
- file: ..\prominenceai--deepstream-services-library\src\DslSourceBintr.h
  why: Source base class and common patterns
  
- file: ..\prominenceai--deepstream-services-library\src\DslSinkBintr.h
  why: Sink base class and encoding configurations

- url: https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvstreammux.html
  why: Understanding DeepStream's stream multiplexer for batching

- url: https://gstreamer.freedesktop.org/documentation/tutorials/playback/playbin-usage.html
  why: Decodebin and uridecodebin for automatic format handling

- file: ..\prominenceai--deepstream-services-library\src\DslRecordMgr.h
  why: Smart recording implementation patterns

- url: https://docs.rs/gstreamer-app/latest/gstreamer_app/
  why: AppSrc and AppSink for custom data injection/extraction
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
CREATE src/source/mod.rs:
  - TRAIT Source with common operations
  - ENUM SourceType for runtime identification
  - BASE struct SourceBase with shared fields

Task 2:
CREATE src/source/file.rs:
  - STRUCT FileSource with URI handling
  - IMPLEMENT loop playback option
  - METHOD repeat_count() for finite loops
  - HANDLE EOS events for seamless looping

Task 3:
CREATE src/source/rtsp.rs:
  - STRUCT RtspSource with connection management
  - IMPLEMENT reconnection logic with backoff
  - TIMEOUT handling for network issues
  - CREDENTIAL management for authenticated streams

Task 4:
CREATE src/source/v4l2.rs:
  - STRUCT V4L2Source for USB cameras
  - DEVICE enumeration and selection
  - FORMAT negotiation (resolution, framerate)
  - PROPERTY control (brightness, contrast, etc.)

Task 5:
CREATE src/source/csi.rs:
  - STRUCT CsiSource for MIPI CSI cameras
  - SENSOR mode configuration
  - NVIDIA-specific properties
  - MULTI-camera synchronization

Task 6:
CREATE src/source/app.rs:
  - STRUCT AppSource for custom data
  - BUFFER push API with timestamps
  - CAPS negotiation
  - CALLBACK for need-data signal

Task 7:
CREATE src/sink/mod.rs:
  - TRAIT Sink with common operations
  - ENUM SinkType for identification
  - BASE struct SinkBase

Task 8:
CREATE src/sink/window.rs:
  - STRUCT WindowSink for display
  - PLATFORM detection (X11, Wayland)
  - OVERLAY support for OSD
  - FULLSCREEN and windowed modes

Task 9:
CREATE src/sink/file.rs:
  - STRUCT FileSink with encoding
  - CODEC selection (H264, H265)
  - CONTAINER format (MP4, MKV)
  - BITRATE and quality settings

Task 10:
CREATE src/sink/rtsp.rs:
  - STRUCT RtspServerSink
  - PORT configuration
  - CLIENT management
  - AUTHENTICATION support

Task 11:
CREATE src/sink/smart_record.rs:
  - STRUCT SmartRecordSink
  - CIRCULAR buffer for pre-event recording
  - TRIGGER API for start/stop
  - DURATION configuration
  - FILE naming with timestamps

Task 12:
CREATE tests/source_sink_integration.rs:
  - TEST file source to file sink
  - TEST RTSP source to window sink
  - TEST camera source with smart recording
  - TEST source switching during playback
```

### Out of Scope
- WebRTC sink (complex, separate PRP)
- Image sources (JPEG, PNG) 
- Message sinks (Kafka, MQTT)
- 3D sinks (separate PRP)

## Success Criteria

- [x] All source types can ingest video successfully
- [x] All sink types can output/save video
- [x] Smart recording triggers on demand
- [x] Sources handle disconnection gracefully
- [x] Format conversion happens automatically
- [x] Memory usage remains stable during long runs
- [x] Examples demonstrate each source/sink type

## Dependencies

### Technical Dependencies
- PRP-01 foundation module
- PRP-02 pipeline management
- DeepStream SDK for CSI camera support
- Platform-specific libraries (X11, V4L2)

### Knowledge Dependencies
- Video codec knowledge (H264, H265)
- Streaming protocols (RTSP, HTTP)
- Linux video subsystems (V4L2, CSI)

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| Platform-specific sink issues | High | Medium | Abstract platform differences, provide fallbacks |
| Network source reliability | High | Medium | Implement robust reconnection with exponential backoff |
| Format incompatibility | Medium | High | Use decodebin for automatic format handling |
| Smart recording buffer overflow | Low | High | Implement ring buffer with size limits |

## Architecture Decisions

### Decision: Use Bins for source/sink encapsulation
**Options Considered:**
1. Individual elements directly in pipeline
2. Bins containing multiple elements
3. Custom GStreamer elements

**Decision:** Bins for complex sources/sinks

**Rationale:** Bins hide complexity, enable hot-swapping, and match the DSL C++ architecture.

### Decision: Trait-based design for sources/sinks
**Options Considered:**
1. Inheritance hierarchy
2. Trait-based with dynamic dispatch
3. Generic types with static dispatch

**Decision:** Traits with dynamic dispatch

**Rationale:** Allows runtime source/sink selection while maintaining type safety.

## Validation Strategy

- **Unit Testing**: Test each source/sink type individually
- **Format Testing**: Verify support for various codecs
- **Network Testing**: Test RTSP with simulated network issues
- **Performance Testing**: Measure latency and throughput
- **Endurance Testing**: Run for extended periods to check stability

## Future Considerations

- Hardware encoding support (NVENC)
- Multi-stream synchronization
- Adaptive bitrate streaming
- Cloud storage sinks (S3, Azure Blob)
- WebRTC peer-to-peer streaming

## References

- [GStreamer Source Elements](https://gstreamer.freedesktop.org/documentation/tutorials/basic/concepts.html)
- [DeepStream Source Components](https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvarguscamerasrc.html)
- [V4L2 Documentation](https://www.kernel.org/doc/html/v4.9/media/uapi/v4l/v4l2.html)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-28
- **Last Modified**: 2025-08-28
- **Status**: Draft
- **Confidence Level**: 8/10 - Well understood patterns from DSL C++, main complexity in platform-specific code