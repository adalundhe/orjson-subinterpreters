// SPDX-License-Identifier: (Apache-2.0 OR MIT)
// Copyright ijl (2025)

// NOTE: The global allocator using PyMem_* has been removed for subinterpreter compatibility.
// PyMem_* functions are interpreter-specific, and using them from a global allocator would
// cause memory to be allocated in the wrong interpreter's heap, leading to crashes or
// undefined behavior in subinterpreter environments.
//
// The standard Rust allocator is now used instead. This is safe because:
// 1. Rust's standard allocator is not interpreter-specific
// 2. Memory allocated by Rust can be safely used across interpreters
// 3. Python objects are still managed through PyObject APIs which are called with
//    the correct interpreter context
//
// If PyMem_* allocation is needed in the future, it should be done explicitly
// with interpreter context, not through a global allocator.
