# TODO

## Port deepstream-services-library

The deepstream-services-library is a c-language library that provides a higher level API for nvidia deepstream.

The library is designed to simplify the process of developing deepstream applications by providing a more user-friendly interface and higher-level abstractions.

It also provides a set of utilities and helper functions to streamline common tasks, making it easier for developers to focus on building their applications without getting bogged down in the details of the underlying deepstream framework.

## Implementation Details

The deepstream-services-library is implemented in C and is designed to be lightweight and efficient. It leverages the existing deepstream APIs and provides a more convenient interface for common tasks such as:

- Managing deepstream pipelines
- Handling metadata and events
- Interfacing with other GStreamer elements

The library is intended to be used as a building block for higher-level applications and frameworks that require deepstream functionality.

The dsl (deepstream-services-library) crate is a Rust wrapper around the deepstream-services-library, providing a more idiomatic interface for Rust developers.

The original deepstream-services-library is written in C, and the dsl crate aims to provide a safe and ergonomic API for Rust developers while maintaining compatibility with the underlying C library.

The original library's source code is available for inspection at ../prominenceai--deepstream-services-library.

