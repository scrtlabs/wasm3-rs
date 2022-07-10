//! Error related functionality of wasm3.
use core::cmp;
use core::fmt;

use crate::utils::cstr_to_str;

/// Result alias that uses [`Error`].
pub type Result<T> = core::result::Result<T, Error>;
/// Result alias that uses [`Trap`].
pub type TrappedResult<T> = core::result::Result<T, Trap>;

/// A wasm trap.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    /// Out of bounds memory access
    OutOfBoundsMemoryAccess,
    /// Division by zero
    DivisionByZero,
    /// Integer overflow
    IntegerOverflow,
    /// Integer conversion
    IntegerConversion,
    /// Indirect call type mismatch
    IndirectCallTypeMismatch,
    /// Table index out of range
    TableIndexOutOfRange,
    /// Exit
    Exit,
    /// Abort
    Abort,
    /// Unreachable
    Unreachable,
    /// Stack overflow
    StackOverflow,
}

impl Trap {
    #[doc(hidden)]
    pub fn as_ptr(self) -> ffi::M3Result {
        unsafe {
            match self {
                Trap::OutOfBoundsMemoryAccess => ffi::m3Err_trapOutOfBoundsMemoryAccess,
                Trap::DivisionByZero => ffi::m3Err_trapDivisionByZero,
                Trap::IntegerOverflow => ffi::m3Err_trapIntegerOverflow,
                Trap::IntegerConversion => ffi::m3Err_trapIntegerConversion,
                Trap::IndirectCallTypeMismatch => ffi::m3Err_trapIndirectCallTypeMismatch,
                Trap::TableIndexOutOfRange => ffi::m3Err_trapTableIndexOutOfRange,
                Trap::Exit => ffi::m3Err_trapExit,
                Trap::Abort => ffi::m3Err_trapAbort,
                Trap::Unreachable => ffi::m3Err_trapUnreachable,
                Trap::StackOverflow => ffi::m3Err_trapStackOverflow,
            }
        }
    }
}

impl cmp::PartialEq<Wasm3Error> for Trap {
    fn eq(&self, &Wasm3Error(err): &Wasm3Error) -> bool {
        self.as_ptr() == err
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Trap {}
impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(unsafe { cstr_to_str(self.as_ptr()) }, f)
    }
}

/// Error returned by wasm3.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Wasm3Error(*const cty::c_char);

impl Wasm3Error {
    /// Check whether this error is the specified trap.
    pub fn is_trap(self, trap: Trap) -> bool {
        trap.as_ptr() == self.0
    }
}

impl cmp::PartialEq<Trap> for Wasm3Error {
    fn eq(&self, trap: &Trap) -> bool {
        trap.as_ptr() == self.0
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Wasm3Error {}
impl fmt::Debug for Wasm3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(unsafe { cstr_to_str(self.0) }, f)
    }
}
impl fmt::Display for Wasm3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(unsafe { cstr_to_str(self.0) }, f)
    }
}

impl From<Trap> for Wasm3Error {
    fn from(trap: Trap) -> Self {
        Self(trap.as_ptr())
    }
}

impl From<Wasm3Error> for Trap {
    fn from(wasm3: Wasm3Error) -> Self {
        unsafe {
            match wasm3.0 {
                e if e == ffi::m3Err_trapOutOfBoundsMemoryAccess => Trap::OutOfBoundsMemoryAccess,
                e if e == ffi::m3Err_trapDivisionByZero => Trap::DivisionByZero,
                e if e == ffi::m3Err_trapIntegerOverflow => Trap::IntegerOverflow,
                e if e == ffi::m3Err_trapIntegerConversion => Trap::IntegerConversion,
                e if e == ffi::m3Err_trapIndirectCallTypeMismatch => Trap::IndirectCallTypeMismatch,
                e if e == ffi::m3Err_trapTableIndexOutOfRange => Trap::TableIndexOutOfRange,
                e if e == ffi::m3Err_trapExit => Trap::Exit,
                e if e == ffi::m3Err_trapAbort => Trap::Abort,
                e if e == ffi::m3Err_trapUnreachable => Trap::Unreachable,
                e if e == ffi::m3Err_trapStackOverflow => Trap::StackOverflow,
                _ => Trap::Abort,
            }
        }
    }
}

/// Error returned by wasm3-rs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// An error originating from wasm3 itself may or may not be a trap.
    Wasm3(Wasm3Error),
    /// A function has been found but its signature didn't match.
    InvalidFunctionSignature,
    /// The specified function could not be found.
    FunctionNotFound,
    /// The specified module could not be found.
    ModuleNotFound,
    /// The modules environment did not match the runtime's environment.
    ModuleLoadEnvMismatch,
    /// The runtime is active and running, and modules can not be linked to it.
    RuntimeIsActive,
}

impl Error {
    /// Convert the error to a trap.
    ///
    /// If the error is not a Wasm3 error, this function returns Trap::Abort
    pub fn into_trap(self) -> Trap {
        let wasm3_err = match self {
            Error::Wasm3(wasm3) => wasm3,
            _ => unsafe { Wasm3Error(ffi::m3Err_trapAbort) },
        };
        wasm3_err.into()
    }
}

impl Error {
    pub(crate) fn from_ffi_res(ptr: ffi::M3Result) -> Result<()> {
        if ptr.is_null() {
            Ok(())
        } else if unsafe { ptr == ffi::m3Err_functionLookupFailed } {
            Err(Error::FunctionNotFound)
        } else {
            Err(Error::Wasm3(Wasm3Error(ptr)))
        }
    }

    pub(crate) fn malloc_error() -> Self {
        Error::Wasm3(Wasm3Error(unsafe { ffi::m3Err_mallocFailed }))
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Wasm3(err) => fmt::Display::fmt(err, f),
            Error::InvalidFunctionSignature => {
                write!(f, "the found function had an unexpected signature")
            }
            Error::FunctionNotFound => write!(f, "the function could not be found"),
            Error::ModuleNotFound => write!(f, "the module could not be found"),
            Error::ModuleLoadEnvMismatch => {
                write!(f, "the module and runtime environments were not the same")
            }
            Error::RuntimeIsActive => write!(
                f,
                "the runtime is active and running, and modules can not be linked to it."
            ),
        }
    }
}

impl From<Wasm3Error> for Error {
    fn from(error: Wasm3Error) -> Self {
        Error::Wasm3(error)
    }
}

impl From<Trap> for Error {
    fn from(trap: Trap) -> Self {
        Self::from(Wasm3Error::from(trap))
    }
}
