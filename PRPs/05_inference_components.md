# PRP: DSL-RS Inference Components (GIE, TIS, Segmentation)

## Executive Summary

This PRP implements the AI inference components for dsl-rs, enabling deep learning model execution for object detection, classification, segmentation, and custom inference tasks. These components integrate TensorRT-optimized models into the video pipeline, providing the intelligence layer for video analytics applications.

## Problem Statement

### Current State
- Pipeline can process video but has no AI capabilities
- No object detection or classification
- No integration with TensorRT or Triton
- Cannot run neural network models
- No support for cascaded inference (primary + secondary)

### Desired State
- Primary inference for object detection
- Secondary inference for classification/attributes
- Support for TensorRT and Triton Inference Server
- Custom model integration with pre/post-processing
- Segmentation visualization
- Multi-model inference pipelines

### Business Value
- Enables intelligent video analytics applications
- Supports state-of-the-art AI models
- Provides hardware acceleration through TensorRT
- Allows cloud/edge deployment flexibility with Triton
- Enables custom model integration for specialized use cases

## Requirements

### Functional Requirements

1. **Primary GIE**: Object detection inference (YOLO, SSD, FasterRCNN)
2. **Secondary GIE**: Classification on detected objects
3. **TIS Support**: Triton Inference Server integration
4. **Model Management**: Load TensorRT engines and ONNX models
5. **Batch Processing**: Efficient batched inference
6. **Custom Processing**: Pre/post-processing callbacks
7. **Segmentation**: Semantic and instance segmentation
8. **Model Switching**: Dynamic model updates without restart
9. **Multi-GPU**: Distribute inference across GPUs

### Non-Functional Requirements

1. **Performance**: <10ms inference latency for real-time
2. **Accuracy**: Maintain model accuracy from training
3. **Scalability**: Support 32+ stream batching
4. **Reliability**: Graceful handling of inference failures
5. **Monitoring**: Inference metrics and profiling

### Context and Research

DeepStream's nvinfer (GIE) and nvinferserver (TIS) plugins provide optimized inference using TensorRT. The implementation must handle model configuration, tensor preparation, and metadata generation while maintaining compatibility with various model architectures.

### Documentation & References
```yaml
# MUST READ - Include these in your context window
- file: ..\prominenceai--deepstream-services-library\src\DslInferBintr.h
  why: Inference component base classes and configuration

- url: https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvinfer.html
  why: nvinfer plugin details and configuration parameters

- url: https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvinferserver.html
  why: Triton Inference Server integration

- url: https://github.com/NVIDIA-AI-IOT/deepstream_reference_apps/tree/master/deepstream-bodypose-3d
  why: Example of cascaded inference pipeline

- file: ..\prominenceai--deepstream-services-library\src\DslSegVisualBintr.h
  why: Segmentation visualization component

- url: https://docs.nvidia.com/deeplearning/tensorrt/developer-guide/index.html
  why: TensorRT optimization and deployment
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
CREATE src/infer/mod.rs:
  - TRAIT Infer with inference operations
  - ENUM InferType (Primary, Secondary, Custom)
  - STRUCT InferConfig for model settings
  - RESULT types for inference output

Task 2:
CREATE src/infer/gie.rs:
  - STRUCT Gie for TensorRT inference
  - CONFIG file parsing (model, labels, etc.)
  - MODEL loading (engine, ONNX, UFF)
  - INTERVAL setting for skip frames
  - GPU_ID selection for multi-GPU

Task 3:
CREATE src/infer/primary.rs:
  - STRUCT PrimaryGie extends Gie
  - DETECTION parsing from tensor output
  - NMS (Non-Maximum Suppression) settings
  - CLUSTER mode for DBSCAN
  - THRESHOLD configuration

Task 4:
CREATE src/infer/secondary.rs:
  - STRUCT SecondaryGie extends Gie
  - OPERATE_ON settings (classifier, detector)
  - CLASSIFICATION result parsing
  - UNIQUE_ID tracking per object
  - CASCADE from primary detector

Task 5:
CREATE src/infer/tis.rs:
  - STRUCT TritonInferServer
  - GRPC/HTTP backend configuration
  - MODEL repository management
  - ENSEMBLE model support
  - DYNAMIC batching configuration

Task 6:
CREATE src/infer/config.rs:
  - STRUCT ModelConfig with parameters
  - NETWORK input/output specifications
  - PREPROCESSING settings (mean, scale)
  - POSTPROCESSING parsers
  - CUSTOM library loading

Task 7:
CREATE src/infer/metadata.rs:
  - STRUCT ObjectMeta for detections
  - STRUCT ClassifierMeta for attributes
  - STRUCT SegmentationMeta for masks
  - CONFIDENCE scores and labels
  - BBOX coordinates and tracking ID

Task 8:
CREATE src/infer/custom.rs:
  - TRAIT CustomInferParser
  - CALLBACK for tensor parsing
  - LIBRARY loading with dlopen
  - FUNCTION signatures for parse_func
  - ERROR handling for custom code

Task 9:
CREATE src/segmentation/mod.rs:
  - STRUCT SegVisual for visualization
  - COLORMAP configuration
  - ALPHA blending settings
  - CLASS filtering
  - GPU/CPU processing selection

Task 10:
CREATE src/infer/engine.rs:
  - STRUCT EngineBuilder for TensorRT
  - ONNX to engine conversion
  - OPTIMIZATION profiles
  - PRECISION modes (FP32, FP16, INT8)
  - CALIBRATION for INT8

Task 11:
CREATE src/infer/batch.rs:
  - STRUCT BatchManager
  - DYNAMIC batch accumulation
  - TIMEOUT handling
  - STREAM synchronization
  - TENSOR memory management

Task 12:
CREATE tests/inference_integration.rs:
  - TEST detection with sample model
  - TEST classification cascade
  - TEST Triton server connection
  - TEST custom parser
  - BENCHMARK inference performance
```

### Out of Scope
- Model training
- Model conversion tools (use TensorRT tools)
- Custom CUDA kernel implementation
- Model optimization/quantization

## Success Criteria

- [x] Primary detection works with YOLO/SSD models
- [x] Secondary classification runs on detected objects
- [x] Triton server integration successful
- [x] Custom models can be integrated
- [x] Metadata properly propagated downstream
- [x] Performance meets real-time requirements
- [x] Multi-stream batching works efficiently

## Dependencies

### Technical Dependencies
- Previous PRPs (01-04) completed
- TensorRT 8.0+
- Triton Inference Server (optional)
- CUDA 11.0+
- cuDNN 8.0+

### Knowledge Dependencies
- Deep learning concepts
- TensorRT optimization
- ONNX model format
- Triton deployment

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| Model compatibility issues | High | High | Provide model validation and conversion guides |
| Memory exhaustion with large models | Medium | High | Implement memory pooling and model size limits |
| Inference latency spikes | Medium | Medium | Add frame skipping and async inference options |
| Triton connection failures | Low | Medium | Implement fallback to local inference |

## Architecture Decisions

### Decision: Support both GIE and TIS
**Options Considered:**
1. Only TensorRT (GIE)
2. Only Triton (TIS)
3. Support both

**Decision:** Support both GIE and TIS

**Rationale:** GIE provides best performance for edge deployment, TIS enables cloud scaling and model versioning.

### Decision: Config file compatibility
**Options Considered:**
1. New Rust-specific format
2. Maintain DeepStream config format
3. Support both with conversion

**Decision:** Maintain DeepStream format

**Rationale:** Enables reuse of existing model configs, reduces migration friction.

## Validation Strategy

- **Unit Testing**: Test config parsing and metadata generation
- **Model Testing**: Verify inference with reference models
- **Accuracy Testing**: Compare output with ground truth
- **Performance Testing**: Measure FPS and latency
- **Stress Testing**: Multi-model, multi-stream scenarios

## Future Considerations

- Federated learning support
- Model compression/pruning
- AutoML integration
- Edge-cloud hybrid inference
- Explainable AI visualizations

## References

- [DeepStream nvinfer Guide](https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvinfer.html)
- [TensorRT Documentation](https://docs.nvidia.com/deeplearning/tensorrt/developer-guide/index.html)
- [Triton Inference Server](https://github.com/triton-inference-server/server)
- [NVIDIA TAO Toolkit](https://developer.nvidia.com/tao-toolkit)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-28
- **Last Modified**: 2025-08-28
- **Status**: Draft
- **Confidence Level**: 7/10 - Complex area with many model-specific considerations, may need iteration for custom models