# PRP: DSL-RS Processing Components (Tracker, OSD, Tiler, Demuxer)

## Executive Summary

This PRP implements the core video processing components for dsl-rs including object tracking, on-screen display (OSD), video tiling, and stream demuxing/splitting. These components transform and annotate video streams between sources and sinks, enabling rich visualization and multi-stream processing capabilities essential for video analytics applications.

## Problem Statement

### Current State
- Basic pipeline with sources and sinks exists
- No object tracking across frames
- No visual annotations or overlays
- Cannot combine multiple streams into tiled displays
- Cannot split streams for parallel processing

### Desired State
- Multiple tracker algorithms (IOU, NvDCF, NvDeepSORT)
- Rich OSD with bounding boxes, labels, and custom graphics
- Flexible tiler for multi-stream visualization
- Demuxer and splitter for stream branching
- Preprocessing for video transformations

### Business Value
- Enables object tracking for analytics and counting
- Provides visual feedback for debugging and monitoring
- Allows security operators to view multiple cameras simultaneously
- Enables parallel processing paths for different analytics

## Requirements

### Functional Requirements

1. **Object Tracker**: Track objects across frames with unique IDs
2. **On-Screen Display**: Draw bounding boxes, labels, and metadata
3. **Tiler**: Combine multiple streams into grid layout
4. **Demuxer**: Split batched streams back to individual streams
5. **Splitter/Tee**: Branch single stream to multiple paths
6. **Preprocessor**: Video transformations (crop, scale, color convert)
7. **Stream Muxer**: Batch multiple streams for inference
8. **Remuxer**: Recombine demuxed streams

### Non-Functional Requirements

1. **Performance**: Minimal CPU overhead, GPU acceleration where possible
2. **Accuracy**: Tracker must maintain ID consistency >95%
3. **Flexibility**: OSD must support custom drawing callbacks
4. **Scalability**: Tiler must handle 16+ streams
5. **Quality**: No visual artifacts in processing

### Context and Research

DeepStream provides specialized elements for these operations that leverage GPU acceleration. The tracker uses AI models for object association, OSD uses hardware acceleration for drawing, and the tiler efficiently composites multiple streams on GPU.

### Documentation & References
```yaml
# MUST READ - Include these in your context window
- file: ..\prominenceai--deepstream-services-library\src\DslTrackerBintr.h
  why: Tracker configuration and algorithm selection

- file: ..\prominenceai--deepstream-services-library\src\DslOsdBintr.h
  why: OSD setup and drawing operations

- file: ..\prominenceai--deepstream-services-library\src\DslTilerBintr.h
  why: Tiler properties and layout management

- url: https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvtracker.html
  why: DeepStream tracker algorithms and configuration

- url: https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvdsosd.html
  why: OSD capabilities and metadata structure

- file: ..\prominenceai--deepstream-services-library\src\DslPreprocBintr.h
  why: Preprocessing configuration for inference optimization

- file: ..\prominenceai--deepstream-services-library\src\DslRemuxerBintr.h
  why: Stream remuxing patterns
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
CREATE src/tracker/mod.rs:
  - TRAIT Tracker with track() method
  - ENUM TrackerType (IOU, DCF, DeepSORT)
  - CONFIG structures for each algorithm

Task 2:
CREATE src/tracker/nvtracker.rs:
  - STRUCT NvTracker wrapping nvtracker element
  - LOAD config files (YAML format)
  - PROPERTIES for enabling/disabling tracking
  - TENSOR meta output configuration

Task 3:
CREATE src/tracker/config.rs:
  - STRUCT TrackerConfig with algorithm parameters
  - IOU threshold settings
  - DCF filter parameters
  - DEEPSORT feature extraction settings
  - VALIDATION of parameter ranges

Task 4:
CREATE src/osd/mod.rs:
  - STRUCT Osd for on-screen display
  - CLOCK display configuration
  - BBOX display settings (color, thickness)
  - TEXT label properties (font, size)
  - MASK display for segmentation

Task 5:
CREATE src/osd/draw.rs:
  - TRAIT Drawable for custom graphics
  - CALLBACKS for frame processing
  - METADATA parsing from upstream
  - COLOR management with RGBA
  - COORDINATE transformation

Task 6:
CREATE src/tiler/mod.rs:
  - STRUCT Tiler for multi-stream display
  - ROWS and columns configuration
  - TILE size calculation
  - SHOW_SOURCE property for labels
  - COMPUTE_HW selection (GPU, VIC)

Task 7:
CREATE src/demuxer/mod.rs:
  - STRUCT Demuxer for stream splitting
  - SOURCE pad creation per stream
  - STREAM ID mapping
  - METADATA preservation
  - PAD probe handlers

Task 8:
CREATE src/splitter/mod.rs:
  - STRUCT Splitter (Tee) for branching
  - BRANCH management (add/remove)
  - QUEUE elements for decoupling
  - REQUEST pad handling
  - BRANCH linking utilities

Task 9:
CREATE src/preprocessor/mod.rs:
  - STRUCT Preprocessor for transformations
  - ROI (Region of Interest) configuration
  - TENSOR shape preparation
  - SCALING algorithms
  - COLOR format conversion

Task 10:
CREATE src/muxer/mod.rs:
  - STRUCT StreamMuxer for batching
  - BATCH size configuration
  - WIDTH/HEIGHT settings
  - BUFFER pool configuration
  - NVBUF memory type selection

Task 11:
CREATE src/remuxer/mod.rs:
  - STRUCT Remuxer for stream combination
  - BRANCH to batch mapping
  - STREAM synchronization
  - METADATA aggregation

Task 12:
CREATE tests/processing_integration.rs:
  - TEST tracker with mock detections
  - TEST OSD drawing operations
  - TEST tiler with multiple sources
  - TEST demuxer stream splitting
  - BENCHMARK processing performance
```

### Out of Scope
- Custom tracker algorithm implementation
- 3D graphics rendering
- Video effects and filters
- Audio processing

## Success Criteria

- [x] Tracker maintains consistent IDs across frames
- [x] OSD displays all metadata correctly
- [x] Tiler handles 16 streams without artifacts
- [x] Demuxer correctly splits batched streams
- [x] All components work in pipeline together
- [x] Performance meets real-time requirements
- [x] Memory usage remains stable

## Dependencies

### Technical Dependencies
- PRP-01, PRP-02, PRP-03 completed
- DeepStream SDK 6.0+
- CUDA toolkit for GPU operations
- TensorRT for DeepSORT features

### Knowledge Dependencies
- Object tracking algorithms
- Computer vision concepts
- GPU programming basics
- GStreamer pad negotiation

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| Tracker ID switches | Medium | High | Tune algorithm parameters, implement ID recovery |
| OSD performance impact | Medium | Medium | Use hardware acceleration, batch draw operations |
| Tiler memory usage | Low | High | Implement dynamic resolution scaling |
| Metadata loss in demuxer | Low | High | Careful metadata copying in pad probes |

## Architecture Decisions

### Decision: Use DeepStream native elements
**Options Considered:**
1. Implement custom tracking in Rust
2. Use DeepStream's nvtracker element
3. Use OpenCV tracking

**Decision:** DeepStream nvtracker

**Rationale:** Hardware accelerated, production tested, supports multiple algorithms.

### Decision: Configuration file support
**Options Considered:**
1. Programmatic configuration only
2. Support YAML config files
3. Custom configuration format

**Decision:** YAML configuration with programmatic override

**Rationale:** Matches DeepStream conventions, allows easy tuning, maintains compatibility.

## Validation Strategy

- **Unit Testing**: Test each component in isolation
- **Integration Testing**: Full pipeline with all components
- **Visual Testing**: Manual verification of OSD output
- **Performance Testing**: FPS and latency measurements
- **Accuracy Testing**: Tracker ID consistency metrics

## Future Considerations

- Custom OSD shapes and animations
- 3D tracking support
- Multi-camera tracking correlation
- Advanced tiler layouts (PiP, custom)
- ML-based preprocessing

## References

- [DeepStream Tracker Documentation](https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvtracker.html)
- [nvdsosd Plugin Guide](https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvdsosd.html)
- [Object Tracking Algorithms](https://github.com/foolwood/benchmark_results)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-28
- **Last Modified**: 2025-08-28
- **Status**: Draft
- **Confidence Level**: 7/10 - Well documented in DSL, some complexity in metadata handling between components