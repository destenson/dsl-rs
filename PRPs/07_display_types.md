# PRP: DSL-RS Display Types and Visualization

## Executive Summary

This PRP implements the display type system for dsl-rs, providing a rich set of visual elements for annotating video streams with graphics, text, and metadata. Display types are the building blocks for creating informative overlays that transform raw video analytics into actionable visual information for operators and systems.

## Problem Statement

### Current State
- OSD component exists but lacks high-level display abstractions
- No reusable visual elements for common annotations
- Manual RGBA color management
- Complex coordinate calculations for shapes
- No dynamic text formatting for metadata

### Desired State
- Complete set of display types (text, shapes, colors)
- Color palettes and predefined colors
- Dynamic text with metadata interpolation
- Geometric primitives (lines, circles, polygons)
- Source information overlays
- Reusable display type instances across ODE actions

### Business Value
- Accelerates development of visual analytics applications
- Provides consistent visual language across deployments
- Enables operators to quickly interpret analytics results
- Supports branding and customization requirements
- Improves debugging and system monitoring

## Requirements

### Functional Requirements

1. **Colors**: RGBA colors, palettes, predefined sets
2. **Fonts**: TrueType font support with size/style
3. **Text**: Static and dynamic text with formatting
4. **Lines**: Single and multi-segment lines
5. **Shapes**: Rectangles, circles, polygons, arrows
6. **Source Info**: Stream ID, dimensions, FPS overlays
7. **Shadows**: Text shadows for readability
8. **Animation**: Color cycling and fading effects

### Non-Functional Requirements

1. **Performance**: Zero-copy where possible
2. **Memory**: Efficient reuse of display types
3. **Quality**: Anti-aliased rendering
4. **Flexibility**: Runtime modification of properties
5. **Thread Safety**: Safe concurrent access

### Context and Research

Display types in DSL are reference-counted objects that can be shared across multiple ODE actions and OSD configurations. They provide a high-level API over the low-level drawing operations, handling coordinate systems, color formats, and text rendering complexities.

### Documentation & References
```yaml
# MUST READ - Include these in your context window
- file: ..\prominenceai--deepstream-services-library\src\DslDisplayTypes.h
  why: Display type class hierarchy and interfaces

- file: ..\prominenceai--deepstream-services-library\docs\api-display-type.md
  why: Complete API documentation for all display types

- url: https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvdsosd.html#metadata-structure
  why: Understanding OSD metadata structures

- file: ..\prominenceai--deepstream-services-library\examples\python\ode_occurrence_polygon_area_inclussion_exclusion.py
  why: Example using various display types

- url: https://github.com/Miastodwa/randomcolor-rs
  why: Random color generation algorithms

- url: https://docs.rs/palette/latest/palette/
  why: Color space conversions and palettes
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
CREATE src/display/mod.rs:
  - TRAIT DisplayType with set_meta() method
  - ENUM DisplayTypeKind for runtime typing
  - BASE struct with common properties
  - REFERENCE counting with Arc

Task 2:
CREATE src/display/color.rs:
  - STRUCT RgbaColor with r,g,b,a fields
  - PREDEFINED colors (RED, BLUE, etc.)
  - FROM implementations for various formats
  - HSL/HSV conversion utilities
  - ALPHA blending calculations

Task 3:
CREATE src/display/palette.rs:
  - STRUCT ColorPalette with color array
  - PREDEFINED palettes (spectral, heat, etc.)
  - RANDOM palette generation
  - INDEX management for cycling
  - INTERPOLATION between colors

Task 4:
CREATE src/display/font.rs:
  - STRUCT RgbaFont with face/size
  - TRUETYPE font loading
  - STYLE options (bold, italic)
  - METRICS calculation
  - CACHE for font handles

Task 5:
CREATE src/display/text.rs:
  - STRUCT RgbaText for static text
  - POSITION (x, y offsets)
  - BACKGROUND color option
  - SHADOW support
  - ALIGNMENT options

Task 6:
CREATE src/display/dynamic_text.rs:
  - STRUCT DynamicText with format string
  - METADATA interpolation
  - CALLBACK for custom formatting
  - TIME/DATE formatting
  - COUNTER variables

Task 7:
CREATE src/display/line.rs:
  - STRUCT RgbaLine for single line
  - START/END coordinates
  - WIDTH configuration
  - ARROW head option
  - DASHED line support

Task 8:
CREATE src/display/multi_line.rs:
  - STRUCT RgbaMultiLine for polyline
  - COORDINATE array
  - CONNECTED vs separate segments
  - SMOOTH curves option

Task 9:
CREATE src/display/rectangle.rs:
  - STRUCT RgbaRectangle
  - BORDER width and color
  - FILL color option
  - ROUNDED corners
  - GRADIENT fills

Task 10:
CREATE src/display/circle.rs:
  - STRUCT RgbaCircle
  - CENTER and radius
  - FILL and border options
  - ELLIPSE support
  - ARC segments

Task 11:
CREATE src/display/polygon.rs:
  - STRUCT RgbaPolygon
  - VERTEX array
  - CONVEX/CONCAVE handling
  - FILL rules (even-odd, winding)
  - TRIANGULATION for complex shapes

Task 12:
CREATE src/display/source_info.rs:
  - STRUCT SourceUniqueId overlay
  - STRUCT SourceStreamId overlay
  - STRUCT SourceName overlay
  - STRUCT SourceDimensions overlay
  - STRUCT SourceFrameRate overlay
  - METADATA extraction from stream

Task 13:
CREATE src/display/builder.rs:
  - BUILDER pattern for complex types
  - VALIDATION of parameters
  - DEFAULT values
  - FLUENT API

Task 14:
CREATE tests/display_integration.rs:
  - TEST color conversions
  - TEST text rendering
  - TEST shape drawing
  - TEST metadata overlay
  - VISUAL tests with output
```

### Out of Scope
- 3D graphics rendering
- Video effects and filters
- Custom shader support
- SVG rendering
- Bitmap/image overlays

## Success Criteria

- [x] All display types render correctly
- [x] Colors match expected RGB values
- [x] Text is readable on various backgrounds
- [x] Shapes draw without artifacts
- [x] Metadata updates dynamically
- [x] Memory usage stable with many display types
- [x] Thread-safe sharing works correctly

## Dependencies

### Technical Dependencies
- Previous PRPs (01-06) completed
- FreeType or similar for fonts
- Color manipulation library
- OSD component from PRP-04

### Knowledge Dependencies
- Computer graphics basics
- Color theory and spaces
- Typography concepts
- Computational geometry

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| Font rendering issues | Medium | Medium | Use proven font library, test multiple fonts |
| Color space confusion | Low | Low | Clear documentation, consistent API |
| Performance with many overlays | Medium | High | Batch rendering, caching, GPU acceleration |
| Platform font differences | High | Low | Bundle default fonts, fallback chain |

## Architecture Decisions

### Decision: Reference-counted display types
**Options Considered:**
1. Copy on use
2. Reference counting with Arc
3. Global registry

**Decision:** Arc for sharing

**Rationale:** Enables efficient reuse across actions, thread-safe, matches DSL C++ design.

### Decision: Builder pattern for construction
**Options Considered:**
1. Many constructor parameters
2. Builder pattern
3. Configuration structs

**Decision:** Builder pattern with defaults

**Rationale:** Provides good ergonomics, allows partial configuration, compile-time safety.

## Validation Strategy

- **Unit Testing**: Test each display type creation
- **Visual Testing**: Render output verification
- **Property Testing**: Random color/coordinate generation
- **Integration Testing**: Use with OSD and ODE
- **Performance Testing**: Rendering benchmarks

## Future Considerations

- SVG support for complex graphics
- Animation frameworks
- GPU-accelerated rendering
- Custom shader effects
- AR/VR overlays

## References

- [Cairo Graphics Library](https://www.cairographics.org/)
- [Color Space Conversions](https://en.wikipedia.org/wiki/Color_space)
- [Typography Basics](https://practicaltypography.com/)
- [Computational Geometry](https://www.cs.princeton.edu/~rs/AlgsDS07/16Geometric.pdf)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-28
- **Last Modified**: 2025-08-28
- **Status**: Draft
- **Confidence Level**: 9/10 - Well-defined scope, clear patterns from DSL C++, standard graphics concepts