use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::pin::Pin;
use core::ptr::{self, NonNull};

use crate::environment::Environment;
use crate::error::{Error, Result};
use crate::function::Function;
use crate::module::{Module, ParsedModule};
use crate::utils::str_to_cstr_owned;

type PinnedAnyClosure = Pin<Box<dyn core::any::Any + 'static>>;

/// A runtime context for wasm3 modules.
#[derive(Debug)]
pub struct Runtime {
    raw: NonNull<ffi::M3Runtime>,
    environment: Option<Environment>,
    // holds all linked closures so that they properly get disposed of when runtime drops
    closure_store: UnsafeCell<Vec<PinnedAnyClosure>>,
    // holds all backing data of loaded modules as they have to be kept alive for the module's lifetime
    module_data: UnsafeCell<Vec<Box<[u8]>>>,
}

impl Runtime {
    /// Creates a new runtime with the given stack size in slots.
    ///
    /// # Errors
    ///
    /// This function will error on memory allocation failure.
    pub fn new(environment: &Environment, stack_size: u32) -> Result<Self> {
        unsafe {
            NonNull::new(ffi::m3_NewRuntime(
                environment.as_ptr(),
                stack_size,
                ptr::null_mut(),
            ))
        }
        .ok_or_else(Error::malloc_error)
        .map(|raw| Runtime {
            raw,
            environment: Some(environment.clone()),
            closure_store: UnsafeCell::new(Vec::new()),
            module_data: UnsafeCell::new(Vec::new()),
        })
    }

    /// Parses and loads a module from bytes.
    pub fn parse_and_load_module<TData: Into<Box<[u8]>>>(&self, bytes: TData) -> Result<Module> {
        match &self.environment {
            Some(env) => Module::parse(env, bytes).and_then(|module| self.load_module(module)),
            None => Err(Error::RuntimeIsActive),
        }
    }

    /// Loads a parsed module returning the module if unsuccessful.
    ///
    /// # Errors
    ///
    /// This function will error if the module's environment differs from the one this runtime uses.
    pub fn load_module(&self, module: ParsedModule) -> Result<Module> {
        if self.environment.is_none() {
            Err(Error::RuntimeIsActive)
        } else if self.environment.as_ref().unwrap() != module.environment() {
            Err(Error::ModuleLoadEnvMismatch)
        } else {
            let raw_mod = module.as_ptr();
            Error::from_ffi_res(unsafe { ffi::m3_LoadModule(self.raw.as_ptr(), raw_mod) })?;
            // SAFETY: Runtime isn't Send, therefor this access is single-threaded and kept alive only for the Vec::push call
            // as such this can not alias.
            unsafe { (*self.module_data.get()).push(module.take_data()) };

            Ok(Module::from_raw(self, raw_mod))
        }
    }

    /// Looks up a function by the given name in the loaded modules of this runtime.
    /// See [`Module::find_function`] for possible error cases.
    ///
    /// [`Module::find_function`]: ../module/struct.Module.html#method.find_function
    pub fn find_function<ARGS, RET>(&self, name: &str) -> Result<Function<ARGS, RET>>
    where
        ARGS: crate::WasmArgs,
        RET: crate::WasmType,
    {
        let mut func_raw: ffi::IM3Function = core::ptr::null_mut();
        let func_name_cstr = str_to_cstr_owned(name);
        let result = unsafe {
            ffi::m3_FindFunction(
                &mut func_raw as *mut ffi::IM3Function,
                self.as_ptr(),
                func_name_cstr.as_ptr(),
            )
        };
        Error::from_ffi_res(result)?;
        let func = NonNull::new(func_raw).ok_or(Error::FunctionNotFound)?;
        Function::from_raw(self, func)
    }

    /// Returns a mutable slice of the memory of this runtime.
    ///
    /// Calling Wasm functions may lead to reallocations which move the
    /// location of the module's memory, so this method takes a mutable
    /// reference to self to ensure that while memory is being manipulated,
    /// no other operation can be done with the runtime.
    pub fn memory(&mut self) -> &mut [u8] {
        unsafe {
            let mut len: u32 = 0;
            let mut data = ffi::m3_GetMemory(self.as_ptr(), &mut len, 0);
            if data.is_null() || (len as isize) > isize::MAX {
                data = NonNull::dangling().as_ptr();
                len = 0;
            }
            std::slice::from_raw_parts_mut(data, len as usize)
        }
    }
}

impl Runtime {
    pub(crate) fn from_raw(raw: NonNull<ffi::M3Runtime>) -> Self {
        Self {
            raw,
            environment: None,
            closure_store: UnsafeCell::default(),
            module_data: UnsafeCell::default(),
        }
    }
    pub(crate) fn push_closure(&self, closure: PinnedAnyClosure) {
        unsafe { (*self.closure_store.get()).push(closure) };
    }

    pub(crate) fn as_ptr(&self) -> ffi::IM3Runtime {
        self.raw.as_ptr()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if self.environment.is_some() {
            unsafe { ffi::m3_FreeRuntime(self.raw.as_ptr()) };
        }
    }
}

#[test]
fn create_and_drop_rt() {
    let env = Environment::new().expect("env alloc failure");
    assert!(Runtime::new(&env, 1024 * 64).is_ok());
}
