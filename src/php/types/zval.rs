//! The base value in PHP. A Zval can contain any PHP type, and the type that it contains is
//! determined by a property inside the struct. The content of the Zval is stored in a union.

use core::slice;
use std::{convert::TryFrom, ptr};

use crate::bindings::{
    _call_user_function_impl, _zval_struct__bindgen_ty_1, _zval_struct__bindgen_ty_2,
    ext_php_rs_zend_string_release, zend_is_callable, zend_object, zend_resource, zend_value, zval,
    IS_INTERNED_STRING_EX, IS_STRING_EX,
};

use crate::php::{
    enums::DataType,
    types::{long::ZendLong, string::ZendString},
};

use super::array::ZendHashTable;

/// Zend value. Represents most data types that are in the Zend engine.
pub type Zval = zval;

impl<'a> Zval {
    /// Creates a new, empty zval.
    pub(crate) fn new() -> Self {
        Self {
            value: zend_value {
                ptr: ptr::null_mut(),
            },
            u1: _zval_struct__bindgen_ty_1 {
                type_info: DataType::Null as u32,
            },
            u2: _zval_struct__bindgen_ty_2 { next: 0 },
        }
    }

    /// Returns the value of the zval if it is a long.
    pub fn long(&self) -> Option<ZendLong> {
        if self.is_long() {
            Some(unsafe { self.value.lval })
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a bool.
    pub fn bool(&self) -> Option<bool> {
        if self.is_true() {
            Some(true)
        } else if self.is_false() {
            Some(false)
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a double.
    pub fn double(&self) -> Option<f64> {
        if self.is_double() {
            Some(unsafe { self.value.dval })
        } else {
            self.long().map(|x| x as f64)
        }
    }

    /// Returns the value of the zval if it is a string.
    pub fn string(&self) -> Option<String> {
        if self.is_string() {
            // SAFETY: Zend strings have a length that we know we can read.
            // By reading this many bytes we will not run into any issues.
            //
            // We can safely cast our *const c_char into a *const u8 as both
            // only occupy one byte.
            unsafe {
                let len = (*self.value.str).len;
                let ptr = (*self.value.str).val.as_ptr() as *const u8;
                let _str = std::str::from_utf8(slice::from_raw_parts(ptr, len as usize)).unwrap();

                Some(_str.to_string())
            }
        } else {
            self.double().map(|x| x.to_string())
        }
    }

    /// Returns the value of the zval if it is a resource.
    pub fn resource(&self) -> Option<*mut zend_resource> {
        // TODO: Can we improve this function? I haven't done much research into
        // resources so I don't know if this is the optimal way to return this.
        if self.is_resource() {
            Some(unsafe { self.value.res })
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is an array.
    pub fn array(&self) -> Option<ZendHashTable> {
        if self.is_array() {
            Some(ZendHashTable::from_ptr(unsafe { self.value.arr }))
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is an object.
    pub fn object(&self) -> Option<*mut zend_object> {
        // TODO: Can we improve this function? I haven't done much research into
        // objects so I don't know if this is the optimal way to return this.
        if self.is_object() {
            Some(unsafe { self.value.obj })
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a reference.
    pub fn reference(&self) -> Option<Zval> {
        if self.is_reference() {
            Some(unsafe { (*self.value.ref_).val })
        } else {
            None
        }
    }

    /// Attempts to call the argument as a callable with a list of arguments to pass to the function.
    /// Note that a thrown exception inside the callable is not detectable, therefore you should
    /// check if the return value is valid rather than unwrapping.
    ///
    /// You should not call this function directly, rather through the [`call_user_func`] macro.
    ///
    /// # Parameters
    ///
    /// * `params` - A list of parameters to call the function with.
    ///
    /// # Returns
    ///
    /// * `Some(Zval)` - The result of the function call.
    /// * `None` - The zval was not callable or the call failed.
    pub fn try_call(&self, params: Vec<Zval>) -> Option<Zval> {
        let mut retval = Zval::new();
        let len = params.len();
        let packed = Box::into_raw(params.into_boxed_slice()) as *mut Self;
        let ptr: *const Self = self;

        if !self.is_callable() {
            return None;
        }

        let result = unsafe {
            _call_user_function_impl(
                std::ptr::null_mut(),
                ptr as *mut Self,
                &mut retval,
                len as _,
                packed,
                std::ptr::null_mut(),
            )
        };

        // SAFETY: We just boxed this vector, and the `_call_user_function_impl` does not modify the parameters.
        // We can safely reclaim the memory knowing it will have the same length and size.
        // If any parameters are zend strings, they must be released.
        unsafe {
            let params = Vec::from_raw_parts(packed, len, len);

            for param in params {
                if param.is_string() {
                    ext_php_rs_zend_string_release(param.value.str);
                }
            }
        };

        if result < 0 {
            None
        } else {
            Some(retval)
        }
    }

    /// Returns true if the zval is a long, false otherwise.
    pub fn is_long(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Long as u8 }
    }

    /// Returns true if the zval is null, false otherwise.
    pub fn is_null(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Null as u8 }
    }

    /// Returns true if the zval is true, false otherwise.
    pub fn is_true(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::True as u8 }
    }

    /// Returns true if the zval is false, false otherwise.
    pub fn is_false(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::False as u8 }
    }

    /// Returns true if the zval is a bool, false otherwise.
    pub fn is_bool(&self) -> bool {
        self.is_true() || self.is_false()
    }

    /// Returns true if the zval is a double, false otherwise.
    pub fn is_double(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Double as u8 }
    }

    /// Returns true if the zval is a string, false otherwise.
    pub fn is_string(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::String as u8 }
    }

    /// Returns true if the zval is a resource, false otherwise.
    pub fn is_resource(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Resource as u8 }
    }

    /// Returns true if the zval is an array, false otherwise.
    pub fn is_array(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Array as u8 }
    }

    /// Returns true if the zval is an object, false otherwise.
    pub fn is_object(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Object as u8 }
    }

    /// Returns true if the zval is a reference, false otherwise.
    pub fn is_reference(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Reference as u8 }
    }

    /// Returns true if the zval is callable, false otherwise.
    pub fn is_callable(&self) -> bool {
        let ptr: *const Self = self;
        unsafe { zend_is_callable(ptr as *mut Self, 0, std::ptr::null_mut()) }
    }

    /// Sets the value of the zval as a string.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_string<S>(&mut self, val: S)
    where
        S: AsRef<str>,
    {
        let zend_str = ZendString::new(val, false);
        self.value.str = zend_str;
        self.u1.type_info = IS_STRING_EX;
    }

    /// Sets the value of the zval as a persistent string.
    /// This means that the zend string will persist between
    /// request lifetime.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_persistent_string<S>(&mut self, val: S)
    where
        S: AsRef<str>,
    {
        let zend_str = ZendString::new(val, true);
        self.value.str = zend_str;
        self.u1.type_info = IS_STRING_EX;
    }

    /// Sets the value of the zval as a interned string.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_interned_string<S>(&mut self, val: S)
    where
        S: AsRef<str>,
    {
        let zend_str = ZendString::new_interned(val);
        self.value.str = zend_str;
        self.u1.type_info = IS_INTERNED_STRING_EX;
    }

    /// Sets the value of the zval as a long.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_long(&mut self, val: ZendLong) {
        self.value.lval = val;
        self.u1.type_info = DataType::Long as u32;
    }

    /// Sets the value of the zval as a double.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_double(&mut self, val: f64) {
        self.value.dval = val;
        self.u1.type_info = DataType::Double as u32;
    }

    /// Sets the value of the zval as a boolean.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_bool(&mut self, val: bool) {
        self.u1.type_info = if val {
            DataType::True as u32
        } else {
            DataType::False as u32
        };
    }

    /// Sets the value of the zval as null.
    /// This is the default of a zval.
    pub fn set_null(&mut self) {
        self.u1.type_info = DataType::Null as u32;
    }

    /// Sets the value of the zval as a resource.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_resource(&mut self, val: *mut zend_resource) {
        self.u1.type_info = DataType::Resource as u32;
        self.value.res = val;
    }

    /// Sets the value of the zval as an object.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    /// * `copy` - Whether to copy the object or pass as a reference.
    pub fn set_object(&mut self, val: *mut zend_object, _copy: bool) {
        self.u1.type_info = DataType::Object as u32;
        self.value.obj = val;
    }

    /// Sets the value of the zval as an array.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    pub fn set_array<V>(&mut self, val: V)
    where
        V: Into<ZendHashTable>,
    {
        self.u1.type_info = DataType::Array as u32;
        self.value.arr = val.into().into_ptr();
    }
}

impl TryFrom<&Zval> for ZendLong {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.long() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl TryFrom<&Zval> for bool {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.bool() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl TryFrom<&Zval> for f64 {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.double() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl TryFrom<&Zval> for String {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.string() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl<'a, 'b> TryFrom<&'b Zval> for ZendHashTable {
    type Error = ();
    fn try_from(value: &'b Zval) -> Result<Self, Self::Error> {
        match value.array() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl From<ZendLong> for Zval {
    fn from(val: ZendLong) -> Self {
        let mut zv = Self::new();
        zv.set_long(val);
        zv
    }
}

impl From<bool> for Zval {
    fn from(val: bool) -> Self {
        let mut zv = Self::new();
        zv.set_bool(val);
        zv
    }
}
impl From<f64> for Zval {
    fn from(val: f64) -> Self {
        let mut zv = Self::new();
        zv.set_double(val);
        zv
    }
}

impl From<String> for Zval {
    fn from(val: String) -> Self {
        let mut zv = Self::new();
        zv.set_string(val);
        zv
    }
}

impl From<&str> for Zval {
    fn from(val: &str) -> Self {
        let mut zv = Self::new();
        zv.set_string(val);
        zv
    }
}
