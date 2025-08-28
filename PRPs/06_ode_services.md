# PRP: DSL-RS Object Detection Event (ODE) Services

## Executive Summary

This PRP implements the Object Detection Event (ODE) services for dsl-rs, providing a powerful rule-based event system that triggers actions based on object detection criteria. ODE services enable complex analytics scenarios like line crossing, area intrusion, object counting, and anomaly detection without writing custom code for each use case.

## Problem Statement

### Current State
- Inference provides raw detections but no event logic
- No framework for defining detection-based rules
- No automated actions based on analytics events
- Each analytics use case requires custom implementation
- No persistence or accumulation of events

### Desired State
- Comprehensive event trigger system
- Rich set of predefined triggers (line cross, area, count, etc.)
- Flexible action system (record, email, display, custom)
- Event accumulation and statistics
- Heat mapping for traffic analysis
- Complex event logic with AND/OR combinations

### Business Value
- Rapidly implement analytics use cases without coding
- Enable business rules through configuration
- Automate responses to security/safety events
- Generate insights from object behavior patterns
- Reduce development time for analytics applications

## Requirements

### Functional Requirements

1. **Triggers**: Occurrence, absence, count, persistence, intersection
2. **Areas**: Line, polygon, inclusion/exclusion zones
3. **Actions**: Record, capture, email, display, custom callbacks
4. **Accumulators**: Event counting and statistics
5. **Heat Maps**: Spatial density visualization
6. **Criteria**: Class, confidence, dimensions, tracking
7. **Logic**: AND/OR combinations of triggers
8. **Scheduling**: Time-based enable/disable

### Non-Functional Requirements

1. **Performance**: <1ms per frame for rule evaluation
2. **Scalability**: Support 100+ concurrent rules
3. **Reliability**: No false negatives for configured events
4. **Flexibility**: Custom triggers and actions via API
5. **Persistence**: Event history and statistics storage

### Context and Research

ODE services process object metadata from inference and tracking to detect complex events. The system uses a handler attached to pad probes, evaluating triggers for each frame and executing associated actions when conditions are met.

### Documentation & References
```yaml
# MUST READ - Include these in your context window
- file: ..\prominenceai--deepstream-services-library\src\DslOdeTrigger.h
  why: Trigger base classes and evaluation logic

- file: ..\prominenceai--deepstream-services-library\src\DslOdeAction.h
  why: Action types and execution patterns

- file: ..\prominenceai--deepstream-services-library\src\DslOdeArea.h
  why: Spatial area definitions and point-in-polygon

- file: ..\prominenceai--deepstream-services-library\src\DslOdeAccumulator.h
  why: Event accumulation and statistics

- file: ..\prominenceai--deepstream-services-library\src\DslOdeHeatMapper.h
  why: Heat map generation from events

- file: ..\prominenceai--deepstream-services-library\docs\api-ode-trigger.md
  why: Complete trigger API documentation

- url: https://github.com/prominenceai/deepstream-services-library/blob/master/examples/python/ode_line_cross_object_capture_overlay_image.py
  why: Example of line crossing detection with capture
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
CREATE src/ode/mod.rs:
  - TRAIT OdeTrigger with check() method
  - TRAIT OdeAction with execute() method
  - STRUCT OdeHandler for pad probe attachment
  - EVENT structure for trigger results

Task 2:
CREATE src/ode/trigger/occurrence.rs:
  - STRUCT OccurrenceTrigger
  - CLASS filter configuration
  - CONFIDENCE threshold
  - MIN/MAX dimension criteria
  - INFERENCE component filter

Task 3:
CREATE src/ode/trigger/absence.rs:
  - STRUCT AbsenceTrigger  
  - TIMEOUT configuration
  - LAST_SEEN tracking
  - RESET on reappearance

Task 4:
CREATE src/ode/trigger/count.rs:
  - STRUCT CountTrigger
  - MINIMUM/MAXIMUM thresholds
  - PER_CLASS counting
  - DIRECTIONAL counting

Task 5:
CREATE src/ode/trigger/persistence.rs:
  - STRUCT PersistenceTrigger
  - MINIMUM frames threshold
  - TRACKING_ID monitoring
  - CONSECUTIVE vs TOTAL frames

Task 6:
CREATE src/ode/trigger/intersection.rs:
  - STRUCT IntersectionTrigger
  - PAIR-WISE checking
  - CLASS_A and CLASS_B filters
  - OVERLAP percentage

Task 7:
CREATE src/ode/area/line.rs:
  - STRUCT LineArea
  - CROSSING detection algorithm
  - DIRECTION (in/out/any)
  - MULTI-LINE support

Task 8:
CREATE src/ode/area/polygon.rs:
  - STRUCT PolygonArea
  - POINT_IN_POLYGON algorithm
  - INCLUSION/EXCLUSION mode
  - CONVEX/CONCAVE support

Task 9:
CREATE src/ode/action/capture.rs:
  - STRUCT CaptureAction
  - FRAME encoding to JPEG
  - OBJECT cropping
  - ANNOTATION overlay
  - FILE naming with timestamp

Task 10:
CREATE src/ode/action/display.rs:
  - STRUCT DisplayAction
  - BOUNDING box modification
  - COLOR override
  - TEXT annotation
  - FILL/BORDER options

Task 11:
CREATE src/ode/action/record.rs:
  - STRUCT RecordAction
  - START/STOP recording
  - DURATION settings
  - CACHE for pre-event
  - SMART record integration

Task 12:
CREATE src/ode/action/custom.rs:
  - STRUCT CustomAction
  - CALLBACK registration
  - CLIENT_DATA passing
  - ASYNC execution option

Task 13:
CREATE src/ode/accumulator.rs:
  - STRUCT OdeAccumulator
  - EVENT counting per trigger
  - TIME_WINDOW statistics
  - EXPORT to CSV/JSON
  - RESET functionality

Task 14:
CREATE src/ode/heat_mapper.rs:
  - STRUCT HeatMapper
  - GRID resolution
  - COLOR mapping
  - DECAY over time
  - OVERLAY generation

Task 15:
CREATE tests/ode_integration.rs:
  - TEST line crossing detection
  - TEST area intrusion
  - TEST complex trigger logic
  - TEST action execution
  - BENCHMARK performance
```

### Out of Scope
- Machine learning-based anomaly detection
- Complex event processing (CEP) engines
- Cloud-based rule management
- Video analytics dashboards

## Success Criteria

- [x] All trigger types detect events correctly
- [x] Actions execute reliably when triggered
- [x] Complex rules with AND/OR logic work
- [x] Heat maps accurately represent activity
- [x] Performance handles 30+ FPS video
- [x] No memory leaks during long runs
- [x] Examples demonstrate common use cases

## Dependencies

### Technical Dependencies
- Previous PRPs (01-05) completed
- GStreamer pad probe mechanism
- OpenCV for image operations (optional)
- Email/SMTP library for alerts

### Knowledge Dependencies
- Computational geometry (point-in-polygon)
- Event-driven architecture
- State machine design
- Image processing basics

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| Performance degradation with many rules | High | High | Implement early-exit optimization, spatial indexing |
| False positives/negatives | Medium | High | Extensive testing, configurable sensitivity |
| Memory growth from event accumulation | Medium | Medium | Implement circular buffers, time-based cleanup |
| Complex rule debugging | High | Medium | Comprehensive logging, visualization tools |

## Architecture Decisions

### Decision: Rule evaluation in pad probe
**Options Considered:**
1. Separate thread with queue
2. In-line pad probe evaluation
3. GPU-accelerated rule engine

**Decision:** In-line pad probe

**Rationale:** Lowest latency, simplest synchronization, matches DSL C++ design.

### Decision: Trigger composition model
**Options Considered:**
1. Inheritance hierarchy
2. Composition with builder
3. Plugin architecture

**Decision:** Composition with builder pattern

**Rationale:** Maximum flexibility, runtime configuration, easier testing.

## Validation Strategy

- **Unit Testing**: Test each trigger/action independently
- **Integration Testing**: Complex rule scenarios
- **Accuracy Testing**: Verify event detection accuracy
- **Performance Testing**: Measure rule evaluation time
- **Stress Testing**: Many rules with high object count

## Future Considerations

- ML-based anomaly detection triggers
- Distributed ODE processing
- Rule learning from examples
- Visual rule builder UI
- Cloud synchronization of rules

## References

- [DSL ODE Services Documentation](https://github.com/prominenceai/deepstream-services-library/blob/master/docs/api-ode-trigger.md)
- [Computational Geometry Algorithms](https://www.cs.princeton.edu/~rs/AlgsDS07/16Geometric.pdf)
- [Complex Event Processing](https://en.wikipedia.org/wiki/Complex_event_processing)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-28
- **Last Modified**: 2025-08-28
- **Status**: Draft
- **Confidence Level**: 8/10 - Well-documented in DSL, clear patterns to follow, main complexity in optimization