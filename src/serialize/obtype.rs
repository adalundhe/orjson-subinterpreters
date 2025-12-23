// SPDX-License-Identifier: (Apache-2.0 OR MIT)
// Copyright ijl (2020-2025), Aviram Hassan (2020)

use crate::opt::{
    Opt, PASSTHROUGH_DATACLASS, PASSTHROUGH_DATETIME, PASSTHROUGH_SUBCLASS, SERIALIZE_NUMPY,
};
use crate::serialize::per_type::{is_numpy_array, is_numpy_scalar};
// Type constants now accessed via typeref accessor functions

#[repr(u32)]
pub(crate) enum ObType {
    Str,
    Int,
    Bool,
    None,
    Float,
    List,
    Dict,
    Datetime,
    Date,
    Time,
    Tuple,
    Uuid,
    Dataclass,
    NumpyScalar,
    NumpyArray,
    Enum,
    StrSubclass,
    Fragment,
    Unknown,
}

pub(crate) fn pyobject_to_obtype(obj: *mut crate::ffi::PyObject, opts: Opt) -> ObType {
    let ob_type = ob_type!(obj);
    if is_class_by_type!(ob_type, crate::typeref::get_str_type()) {
        ObType::Str
    } else if is_class_by_type!(ob_type, crate::typeref::get_int_type()) {
        ObType::Int
    } else if is_class_by_type!(ob_type, crate::typeref::get_bool_type()) {
        ObType::Bool
    } else if is_class_by_type!(ob_type, crate::typeref::get_none_type()) {
        ObType::None
    } else if is_class_by_type!(ob_type, crate::typeref::get_float_type()) {
        ObType::Float
    } else if is_class_by_type!(ob_type, crate::typeref::get_list_type()) {
        ObType::List
    } else if is_class_by_type!(ob_type, crate::typeref::get_dict_type()) {
        ObType::Dict
    } else if is_class_by_type!(ob_type, crate::typeref::get_datetime_type()) && opt_disabled!(opts, PASSTHROUGH_DATETIME)
    {
        ObType::Datetime
    } else {
        pyobject_to_obtype_unlikely(ob_type, opts)
    }
}

#[cfg_attr(feature = "optimize", optimize(size))]
#[inline(never)]
pub(crate) fn pyobject_to_obtype_unlikely(
    ob_type: *mut crate::ffi::PyTypeObject,
    opts: Opt,
) -> ObType {
    if is_class_by_type!(ob_type, crate::typeref::get_uuid_type()) {
        return ObType::Uuid;
    } else if is_class_by_type!(ob_type, crate::typeref::get_tuple_type()) {
        return ObType::Tuple;
    } else if is_class_by_type!(ob_type, crate::typeref::get_fragment_type()) {
        return ObType::Fragment;
    }

    if opt_disabled!(opts, PASSTHROUGH_DATETIME) {
        if is_class_by_type!(ob_type, crate::typeref::get_date_type()) {
            return ObType::Date;
        } else if is_class_by_type!(ob_type, crate::typeref::get_time_type()) {
            return ObType::Time;
        }
    }

    let tp_flags = tp_flags!(ob_type);

    if opt_disabled!(opts, PASSTHROUGH_SUBCLASS) {
        if is_subclass_by_flag!(tp_flags, Py_TPFLAGS_UNICODE_SUBCLASS) {
            return ObType::StrSubclass;
        } else if is_subclass_by_flag!(tp_flags, Py_TPFLAGS_LONG_SUBCLASS) {
            return ObType::Int;
        } else if is_subclass_by_flag!(tp_flags, Py_TPFLAGS_LIST_SUBCLASS) {
            return ObType::List;
        } else if is_subclass_by_flag!(tp_flags, Py_TPFLAGS_DICT_SUBCLASS) {
            return ObType::Dict;
        }
    }

    if is_subclass_by_type!(ob_type, crate::typeref::get_enum_type()) {
        return ObType::Enum;
    }

    if opt_disabled!(opts, PASSTHROUGH_DATACLASS) && pydict_contains!(ob_type, crate::typeref::get_dataclass_fields_str())
    {
        return ObType::Dataclass;
    }

    if opt_enabled!(opts, SERIALIZE_NUMPY) {
        cold_path!();
        if is_numpy_scalar(ob_type) {
            return ObType::NumpyScalar;
        } else if is_numpy_array(ob_type) {
            return ObType::NumpyArray;
        }
    }

    ObType::Unknown
}
