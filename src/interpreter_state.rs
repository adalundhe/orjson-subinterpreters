// SPDX-License-Identifier: (Apache-2.0 OR MIT)
// Copyright ijl (2018-2025)

//! Per-interpreter state management for subinterpreter support.
//!
//! This module manages interpreter-specific state to support Python 3.14 subinterpreters.
//! Each interpreter has its own instance of all PyObject pointers and caches.

use core::ffi::CStr;
use core::ptr::{NonNull, null_mut};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::thread::LocalKey;

use crate::deserialize::cache::KeyMap;
use crate::ffi::{
    Py_DECREF, Py_False, Py_INCREF, Py_None, Py_True, Py_XDECREF, PyBool_Type, PyByteArray_Type,
    PyBytes_Type, PyDict_Type, PyErr_Clear, PyErr_NewException, PyExc_TypeError, PyFloat_Type,
    PyImport_ImportModule, PyList_Type, PyLong_Type, PyMapping_GetItemString, PyMemoryView_Type,
    PyObject, PyObject_GenericGetDict, PyTuple_Type, PyTypeObject, PyUnicode_InternFromString,
    PyUnicode_New, PyUnicode_Type, orjson_fragmenttype_new,
};

/// Per-interpreter state containing all interpreter-specific PyObject pointers and caches.
/// This struct is Send + Sync because:
/// - PyObject pointers are only accessed when the GIL is held (single-threaded within interpreter)
/// - The HashMap is protected by a Mutex
/// - UnsafeCell for key_map is safe because GIL ensures single-threaded access
unsafe impl Send for InterpreterState {}
unsafe impl Sync for InterpreterState {}

pub(crate) struct InterpreterState {
    // Keyword argument strings
    pub default: *mut PyObject,
    pub option: *mut PyObject,

    // Builtin objects
    pub none: *mut PyObject,
    pub true_: *mut PyObject,
    pub false_: *mut PyObject,
    pub empty_unicode: *mut PyObject,

    // Type objects
    pub bytes_type: *mut PyTypeObject,
    pub bytearray_type: *mut PyTypeObject,
    pub memoryview_type: *mut PyTypeObject,
    pub str_type: *mut PyTypeObject,
    pub int_type: *mut PyTypeObject,
    pub bool_type: *mut PyTypeObject,
    pub none_type: *mut PyTypeObject,
    pub float_type: *mut PyTypeObject,
    pub list_type: *mut PyTypeObject,
    pub dict_type: *mut PyTypeObject,
    pub tuple_type: *mut PyTypeObject,
    pub datetime_type: *mut PyTypeObject,
    pub date_type: *mut PyTypeObject,
    pub time_type: *mut PyTypeObject,
    pub uuid_type: *mut PyTypeObject,
    pub enum_type: *mut PyTypeObject,
    pub field_type: *mut PyTypeObject,
    pub fragment_type: *mut PyTypeObject,
    pub zoneinfo_type: *mut PyTypeObject,

    // Interned strings
    pub utcoffset_method_str: *mut PyObject,
    pub normalize_method_str: *mut PyObject,
    pub convert_method_str: *mut PyObject,
    pub dst_str: *mut PyObject,
    pub dict_str: *mut PyObject,
    pub dataclass_fields_str: *mut PyObject,
    pub slots_str: *mut PyObject,
    pub field_type_str: *mut PyObject,
    pub array_struct_str: *mut PyObject,
    pub dtype_str: *mut PyObject,
    pub descr_str: *mut PyObject,
    pub value_str: *mut PyObject,
    pub int_attr_str: *mut PyObject,

    // Exception types
    pub json_encode_error: *mut PyObject,
    pub json_decode_error: *mut PyObject,

    // Cache - per-interpreter (using UnsafeCell for interior mutability)
    // Safe because GIL ensures single-threaded access within an interpreter
    #[cfg(not(Py_GIL_DISABLED))]
    pub key_map: core::cell::UnsafeCell<KeyMap>,
}

unsafe fn look_up_type_object(module_name: &CStr, member_name: &CStr) -> *mut PyTypeObject {
    unsafe {
        let module = PyImport_ImportModule(module_name.as_ptr());
        let module_dict = PyObject_GenericGetDict(module, null_mut());
        let ptr = PyMapping_GetItemString(module_dict, member_name.as_ptr()).cast::<PyTypeObject>();
        Py_DECREF(module_dict);
        Py_DECREF(module);
        ptr
    }
}

#[cfg(not(PyPy))]
unsafe fn look_up_datetime(
    datetime_type: &mut *mut PyTypeObject,
    date_type: &mut *mut PyTypeObject,
    time_type: &mut *mut PyTypeObject,
    zoneinfo_type: &mut *mut PyTypeObject,
) {
    unsafe {
        crate::ffi::PyDateTime_IMPORT();
        let datetime_capsule = crate::ffi::PyCapsule_Import(c"datetime.datetime_CAPI".as_ptr(), 1)
            .cast::<crate::ffi::PyDateTime_CAPI>();
        debug_assert!(!datetime_capsule.is_null());

        *datetime_type = (*datetime_capsule).DateTimeType;
        *date_type = (*datetime_capsule).DateType;
        *time_type = (*datetime_capsule).TimeType;
        *zoneinfo_type = (*datetime_capsule).TZInfoType;
    }
}

#[cfg(PyPy)]
unsafe fn look_up_datetime(
    datetime_type: &mut *mut PyTypeObject,
    date_type: &mut *mut PyTypeObject,
    time_type: &mut *mut PyTypeObject,
    zoneinfo_type: &mut *mut PyTypeObject,
) {
    unsafe {
        *datetime_type = look_up_type_object(c"datetime", c"datetime");
        *date_type = look_up_type_object(c"datetime", c"date");
        *time_type = look_up_type_object(c"datetime", c"time");
        *zoneinfo_type = look_up_type_object(c"zoneinfo", c"ZoneInfo");
    }
}

impl InterpreterState {
    /// Initialize a new interpreter state for the current interpreter.
    #[cold]
    #[cfg_attr(feature = "optimize", optimize(size))]
    pub(crate) unsafe fn new() -> Self {
        unsafe {
            debug_assert!(crate::opt::MAX_OPT < i32::from(u16::MAX));

            let mut state = InterpreterState {
                default: null_mut(),
                option: null_mut(),
                none: Py_None(),
                true_: Py_True(),
                false_: Py_False(),
                empty_unicode: PyUnicode_New(0, 255),
                bytes_type: &raw mut PyBytes_Type,
                bytearray_type: &raw mut PyByteArray_Type,
                memoryview_type: &raw mut PyMemoryView_Type,
                str_type: &raw mut PyUnicode_Type,
                int_type: &raw mut PyLong_Type,
                bool_type: &raw mut PyBool_Type,
                none_type: null_mut(),
                float_type: &raw mut PyFloat_Type,
                list_type: &raw mut PyList_Type,
                dict_type: &raw mut PyDict_Type,
                tuple_type: &raw mut PyTuple_Type,
                datetime_type: null_mut(),
                date_type: null_mut(),
                time_type: null_mut(),
                uuid_type: null_mut(),
                enum_type: null_mut(),
                field_type: null_mut(),
                fragment_type: null_mut(),
                zoneinfo_type: null_mut(),
                utcoffset_method_str: null_mut(),
                normalize_method_str: null_mut(),
                convert_method_str: null_mut(),
                dst_str: null_mut(),
                dict_str: null_mut(),
                dataclass_fields_str: null_mut(),
                slots_str: null_mut(),
                field_type_str: null_mut(),
                array_struct_str: null_mut(),
                dtype_str: null_mut(),
                descr_str: null_mut(),
                value_str: null_mut(),
                int_attr_str: null_mut(),
                json_encode_error: null_mut(),
                json_decode_error: null_mut(),
                #[cfg(not(Py_GIL_DISABLED))]
                key_map: core::cell::UnsafeCell::new(KeyMap::default()),
            };

            state.none_type = unsafe { (*state.none).ob_type };

            look_up_datetime(
                &mut state.datetime_type,
                &mut state.date_type,
                &mut state.time_type,
                &mut state.zoneinfo_type,
            );

            state.uuid_type = look_up_type_object(c"uuid", c"UUID");
            state.enum_type = look_up_type_object(c"enum", c"EnumMeta");
            state.field_type = look_up_type_object(c"dataclasses", c"_FIELD");

            state.fragment_type = orjson_fragmenttype_new();

            state.int_attr_str = PyUnicode_InternFromString(c"int".as_ptr());
            state.utcoffset_method_str = PyUnicode_InternFromString(c"utcoffset".as_ptr());
            state.normalize_method_str = PyUnicode_InternFromString(c"normalize".as_ptr());
            state.convert_method_str = PyUnicode_InternFromString(c"convert".as_ptr());
            state.dst_str = PyUnicode_InternFromString(c"dst".as_ptr());
            state.dict_str = PyUnicode_InternFromString(c"__dict__".as_ptr());
            state.dataclass_fields_str = PyUnicode_InternFromString(c"__dataclass_fields__".as_ptr());
            state.slots_str = PyUnicode_InternFromString(c"__slots__".as_ptr());
            state.field_type_str = PyUnicode_InternFromString(c"_field_type".as_ptr());
            state.array_struct_str = PyUnicode_InternFromString(c"__array_struct__".as_ptr());
            state.dtype_str = PyUnicode_InternFromString(c"dtype".as_ptr());
            state.descr_str = PyUnicode_InternFromString(c"descr".as_ptr());
            state.value_str = PyUnicode_InternFromString(c"value".as_ptr());
            state.default = PyUnicode_InternFromString(c"default".as_ptr());
            state.option = PyUnicode_InternFromString(c"option".as_ptr());

            state.json_encode_error = PyExc_TypeError;
            Py_INCREF(state.json_encode_error);
            let json_jsondecodeerror =
                look_up_type_object(c"json", c"JSONDecodeError").cast::<PyObject>();
            debug_assert!(!json_jsondecodeerror.is_null());
            state.json_decode_error = PyErr_NewException(
                c"hyperjson.JSONDecodeError".as_ptr(),
                json_jsondecodeerror,
                null_mut(),
            );
            debug_assert!(!state.json_decode_error.is_null());
            Py_XDECREF(json_jsondecodeerror);

            state
        }
    }
}

/// Global registry of interpreter states, keyed by module pointer (as usize for Send+Sync).
/// Each interpreter has its own module instance, so we use the module pointer as the key.
/// Using usize is safe because we only compare pointers, never dereference them.
static INTERPRETER_STATES: OnceLock<Mutex<HashMap<usize, Box<InterpreterState>>>> =
    OnceLock::new();

/// Get or create the interpreter state for the given module.
/// The module pointer uniquely identifies the interpreter.
#[inline(always)]
pub(crate) unsafe fn get_or_init_state(module: *mut PyObject) -> *const InterpreterState {
    unsafe {
        let states = INTERPRETER_STATES.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = states.lock().unwrap();

        // Use entry API for efficient lookup/insert
        // Convert pointer to usize for HashMap key (safe for comparison only)
        let module_key = module as usize;
        let state_ptr = guard
            .entry(module_key)
            .or_insert_with(|| Box::new(InterpreterState::new()))
            .as_ref() as *const InterpreterState;

        // Leak the pointer - the state lives as long as the interpreter
        state_ptr
    }
}

/// Thread-local cache for the current interpreter's state pointer.
/// This avoids repeated module imports for performance.
thread_local! {
    static CACHED_STATE: std::cell::Cell<(*mut PyObject, *const InterpreterState)> = 
        std::cell::Cell::new((null_mut(), null_mut()));
}

/// Get the current interpreter's state, using thread-local cache for performance.
/// This imports the orjson module if not cached.
#[inline(always)]
pub(crate) unsafe fn get_current_state() -> *const InterpreterState {
    unsafe {
        // Try to get from cache first
        let cached = CACHED_STATE.with(|cell| {
            let (cached_module, cached_state) = cell.get();
            if !cached_module.is_null() && !cached_state.is_null() {
                Some((cached_module, cached_state))
            } else {
                None
            }
        });

        if let Some((_cached_module, cached_state)) = cached {
            // Verify the module is still valid by checking if it's the same interpreter
            // For now, we'll just use it - in practice, the module pointer should be stable
            // within a thread for the same interpreter
            return cached_state;
        }

        // Cache miss - import module and cache it
        let module = PyImport_ImportModule(c"hyperjson".as_ptr());
        if module.is_null() {
            // This shouldn't happen, but if it does, we'll crash
            core::hint::unreachable_unchecked();
        }
        let state = get_or_init_state(module);
        
        // Cache it
        CACHED_STATE.with(|cell| {
            cell.set((module, state));
        });
        
        // Don't DECREF the module - we're keeping it alive for the cache
        // The module will be cleaned up when the interpreter is destroyed
        state
    }
}

