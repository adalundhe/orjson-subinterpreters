# Subinterpreter Compatibility Analysis

## Summary

This document identifies the code blocks that make orjson incompatible with Python 3.14 subinterpreters and the patterns that would be unsafe to use in a subinterpreter environment.

## 1. Explicit Incompatibility Marker

**Location**: `src/lib.rs` lines 238-242

```rust
#[cfg(Py_3_12)]
PyModuleDef_Slot {
    slot: crate::ffi::Py_mod_multiple_interpreters,
    value: crate::ffi::Py_MOD_MULTIPLE_INTERPRETERS_NOT_SUPPORTED,
},
```

This is the **primary block** that explicitly marks the module as incompatible with subinterpreters. When Python 3.12+ tries to import the module in a subinterpreter, this slot causes the import to fail.

**Impact**: This prevents the module from being imported in any subinterpreter, causing an immediate failure on import.

---

## 2. Unsafe Patterns for Subinterpreters

The following patterns would be **unsafe** to use in a subinterpreter environment, even if the explicit incompatibility marker were removed:

### 2.1 Static Mutable PyObject Pointers (Interpreter-Specific State)

**Location**: `src/typeref.rs` lines 17-64

All these static mutable variables store PyObject pointers that are **specific to the main interpreter**:

```rust
pub(crate) static mut DEFAULT: *mut PyObject = null_mut();
pub(crate) static mut OPTION: *mut PyObject = null_mut();
pub(crate) static mut NONE: *mut PyObject = null_mut();
pub(crate) static mut TRUE: *mut PyObject = null_mut();
pub(crate) static mut FALSE: *mut PyObject = null_mut();
pub(crate) static mut EMPTY_UNICODE: *mut PyObject = null_mut();

// Type objects (also interpreter-specific)
pub(crate) static mut BYTES_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut BYTEARRAY_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut MEMORYVIEW_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut STR_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut INT_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut BOOL_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut NONE_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut FLOAT_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut LIST_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut DICT_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut DATETIME_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut DATE_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut TIME_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut TUPLE_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut UUID_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut ENUM_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut FIELD_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut FRAGMENT_TYPE: *mut PyTypeObject = null_mut();
pub(crate) static mut ZONEINFO_TYPE: *mut PyTypeObject = null_mut();

// String interned objects
pub(crate) static mut UTCOFFSET_METHOD_STR: *mut PyObject = null_mut();
pub(crate) static mut NORMALIZE_METHOD_STR: *mut PyObject = null_mut();
pub(crate) static mut CONVERT_METHOD_STR: *mut PyObject = null_mut();
pub(crate) static mut DST_STR: *mut PyObject = null_mut();
pub(crate) static mut DICT_STR: *mut PyObject = null_mut();
pub(crate) static mut DATACLASS_FIELDS_STR: *mut PyObject = null_mut();
pub(crate) static mut SLOTS_STR: *mut PyObject = null_mut();
pub(crate) static mut FIELD_TYPE_STR: *mut PyObject = null_mut();
pub(crate) static mut ARRAY_STRUCT_STR: *mut PyObject = null_mut();
pub(crate) static mut DTYPE_STR: *mut PyObject = null_mut();
pub(crate) static mut DESCR_STR: *mut PyObject = null_mut();
pub(crate) static mut VALUE_STR: *mut PyObject = null_mut();
pub(crate) static mut INT_ATTR_STR: *mut PyObject = null_mut();

// Exception types
pub(crate) static mut JsonEncodeError: *mut PyObject = null_mut();
pub(crate) static mut JsonDecodeError: *mut PyObject = null_mut();
```

**Why unsafe**: PyObject pointers are **interpreter-specific**. Each subinterpreter has its own object space, so:
- A PyObject pointer from the main interpreter is invalid in a subinterpreter
- Type objects differ between interpreters
- Using these pointers in a subinterpreter would cause crashes or undefined behavior

**Initialization**: These are initialized once in `init_typerefs()` (called from `orjson_init_exec`), which runs in the main interpreter context. The values stored are specific to that interpreter.

**Usage locations**:
- `src/lib.rs`: Used for keyword argument matching (`typeref::OPTION`, `typeref::DEFAULT`)
- `src/lib.rs`: Used for type checking (`typeref::INT_TYPE`, `typeref::NONE`)
- `src/serialize/per_type/pybool.rs`: Used for boolean comparison (`typeref::TRUE`)
- Throughout the codebase: Type checking, object creation, exception raising

---

### 2.2 Global Key Cache with PyObject Pointers

**Location**: `src/deserialize/cache.rs` line 37

```rust
pub(crate) static mut KEY_MAP: OnceCell<KeyMap> = OnceCell::new();
```

Where `KeyMap` is:
```rust
pub(crate) type KeyMap =
    AssociativeCache<u64, CachedKey, Capacity2048, HashDirectMapped, RoundRobinReplacement>;
```

And `CachedKey` contains:
```rust
pub(crate) struct CachedKey {
    ptr: PyStr,  // This is a PyObject pointer
}
```

**Why unsafe**: 
- The cache stores PyObject pointers (as `PyStr`) that are interpreter-specific
- Once initialized in one interpreter, the cache would contain pointers invalid in other interpreters
- The cache is shared globally across all interpreters, but the PyObject pointers inside are only valid in the interpreter that created them

**Initialization**: Set in `src/typeref.rs` line 116-119:
```rust
#[cfg(not(Py_GIL_DISABLED))]
assert!(
    crate::deserialize::KEY_MAP
        .set(crate::deserialize::KeyMap::default())
        .is_ok()
);
```

**Usage**: `src/deserialize/pyobject.rs` - Used to cache string keys during deserialization to avoid repeated string allocations.

---

### 2.3 Global Allocator Using Python Memory API

**Location**: `src/alloc.rs` lines 11-40

```rust
#[global_allocator]
static ALLOCATOR: PyMemAllocator = PyMemAllocator {};

struct PyMemAllocator {}

unsafe impl GlobalAlloc for PyMemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { PyMem_Malloc(layout.size()).cast::<u8>() }
    }
    // ... dealloc, realloc, etc.
}
```

**Why unsafe**: 
- `PyMem_Malloc`/`PyMem_Free`/`PyMem_Realloc` are **interpreter-specific** memory allocators
- Each subinterpreter has its own memory space
- Using the main interpreter's allocator from a subinterpreter would allocate memory in the wrong interpreter's heap
- This could cause memory corruption, crashes, or incorrect behavior

**Impact**: This affects **all Rust allocations** in the crate, as it's the global allocator. Every `Box`, `Vec`, `String`, etc. would use the wrong interpreter's memory allocator.

---

### 2.4 Static Function Pointers (Runtime Detection)

**Location**: 
- `src/serialize/writer/json.rs` lines 575-585
- `src/str/pystr.rs` lines 29-35

```rust
// In json.rs
static mut STR_FORMATTER_FN: StrFormatter =
    crate::serialize::writer::str::format_escaped_str_impl_sse2_128;

pub(crate) fn set_str_formatter_fn() {
    unsafe {
        #[cfg(all(target_arch = "x86_64", feature = "avx512"))]
        if std::is_x86_feature_detected!("avx512vl") {
            STR_FORMATTER_FN = crate::serialize::writer::str::format_escaped_str_impl_512vl;
        }
    }
}

// In pystr.rs
static mut STR_CREATE_FN: StrDeserializer = super::scalar::str_impl_kind_scalar;

pub fn set_str_create_fn() {
    unsafe {
        #[cfg(feature = "avx512")]
        if std::is_x86_feature_detected!("avx512vl") {
            STR_CREATE_FN = /* ... */;
        }
    }
}
```

**Why potentially unsafe**: 
- These are set once during initialization based on CPU feature detection
- While the functions themselves are interpreter-agnostic, the initialization happens once globally
- If different subinterpreters run on different CPUs (unlikely but possible), this could be an issue
- More importantly, this pattern suggests a global initialization that doesn't account for per-interpreter state

**Note**: This is less critical than the PyObject pointer issues, but still represents global state.

---

### 2.5 OnceLock/OnceBox Initialization Pattern

**Location**: `src/typeref.rs` lines 102-106, 198

```rust
static INIT: OnceLock<bool> = OnceLock::new();

pub(crate) fn init_typerefs() {
    INIT.get_or_init(_init_typerefs_impl);
}

pub(crate) static mut NUMPY_TYPES: OnceBox<Option<NonNull<NumpyTypes>>> = OnceBox::new();
```

**Why unsafe**: 
- `OnceLock`/`OnceBox` ensure initialization happens only once **globally**
- The initialization function `_init_typerefs_impl()` creates PyObject pointers specific to the interpreter that first calls it
- Subsequent subinterpreters would see the already-initialized flag and skip initialization, but would use PyObject pointers from a different interpreter
- This is a **race condition** where the first interpreter to initialize "wins", and all others get invalid pointers

---

### 2.6 Direct Access to Global Type Objects

**Location**: `src/typeref.rs` lines 129-139

```rust
STR_TYPE = &raw mut PyUnicode_Type;
BYTES_TYPE = &raw mut PyBytes_Type;
DICT_TYPE = &raw mut PyDict_Type;
LIST_TYPE = &raw mut PyList_Type;
TUPLE_TYPE = &raw mut PyTuple_Type;
BOOL_TYPE = &raw mut PyBool_Type;
INT_TYPE = &raw mut PyLong_Type;
FLOAT_TYPE = &raw mut PyFloat_Type;
// etc.
```

**Why unsafe**: 
- These are taking addresses of global type objects from the Python C API
- In a subinterpreter environment, each interpreter has its own type objects
- Taking the address in one interpreter and using it in another would point to the wrong type object
- This would cause incorrect type checks and object creation

---

## 3. Summary of Issues

### Critical (Causes Immediate Failure):
1. **Explicit incompatibility marker** (`Py_MOD_MULTIPLE_INTERPRETERS_NOT_SUPPORTED`) - Prevents import

### Critical (Would Cause Crashes/Undefined Behavior):
2. **Static mutable PyObject pointers** - Invalid pointers across interpreters
3. **Global allocator using PyMem_*** - Wrong memory space
4. **Global key cache with PyObject pointers** - Invalid cached pointers
5. **OnceLock initialization pattern** - First interpreter "wins", others get wrong state

### Moderate (Potential Issues):
6. **Static function pointers** - Global state, but functions themselves are safe
7. **Direct access to global type objects** - Type objects differ per interpreter

---

## 4. What Would Need to Change

To make orjson subinterpreter-compatible, the following changes would be needed:

1. **Remove the explicit incompatibility marker** (change `Py_MOD_MULTIPLE_INTERPRETERS_NOT_SUPPORTED` to `Py_MOD_MULTIPLE_INTERPRETERS_SUPPORTED`)

2. **Per-interpreter state management**:
   - Store PyObject pointers in per-interpreter storage (e.g., using `PyInterpreterState` or module-level dictionaries)
   - Initialize type references per-interpreter, not globally
   - Clear caches when interpreters are destroyed

3. **Remove or fix the global allocator**:
   - Either remove the global allocator and use standard Rust allocation
   - Or make it interpreter-aware (complex and likely not worth it)

4. **Per-interpreter caches**:
   - Make `KEY_MAP` per-interpreter
   - Ensure all caches are interpreter-specific

5. **Runtime type lookup**:
   - Instead of caching type objects globally, look them up per-interpreter when needed
   - Or cache them per-interpreter

6. **Initialization per subinterpreter**:
   - Ensure `init_typerefs()` runs per-interpreter, not just once globally
   - Use per-interpreter storage for all initialized values

---

## 5. References

- Python PEP 554 (Subinterpreters): https://peps.python.org/pep-0554/
- PyO3 subinterpreter documentation
- Python C API: Interpreter State and Thread State

