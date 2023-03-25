use std::ffi::OsString;

#[derive(Debug)]
pub enum HookError {
    ProtectFailed,
    LoadLibFailed(String),
    GetProcAddressFailed(String),
    CLRInitFailed(String),
    NulError(std::ffi::NulError),
    BadStateError(String),
    GlobalStateCopyFailed,
    Direct3D9InstanceNotFound,
    CreateDeviceFailed(i32),
    ConfReadFailed(String),
    FailedToConvertString(OsString),
    WinApiError(String),
    ModuleNameError(String),
    UnableToLocatedManagedDLL(String),
    D3D9HookFailed,
    D3D9DeviceHookFailed,
    GlobalLockError,
    IOError(std::io::Error),
    DInputCreateFailed(String),
    DInputError(String),
    TimeConversionError(std::time::SystemTimeError),
    CStrConvertFailed(std::str::Utf8Error),
    SnapshotFailed(String),
    CaptureFailed(String),
    SnapshotPluginError(String),
    MeshUpdateFailed(String),
    NoShader(),
    SerdeError(String),
    D3D11DeviceHookFailed(String),
    D3D11NoContext,
    D3D11Unsupported(String),
}

impl std::convert::From<std::ffi::NulError> for HookError {
    fn from(error: std::ffi::NulError) -> Self {
        HookError::NulError(error)
    }
}

impl std::convert::From<std::ffi::OsString> for HookError {
    fn from(error: std::ffi::OsString) -> Self {
        HookError::FailedToConvertString(error)
    }
}

impl std::convert::From<std::io::Error> for HookError {
    fn from(error: std::io::Error) -> Self {
        HookError::IOError(error)
    }
}

impl std::convert::From<std::time::SystemTimeError> for HookError {
    fn from(error: std::time::SystemTimeError) -> Self {
        HookError::TimeConversionError(error)
    }
}

impl std::convert::From<std::str::Utf8Error> for HookError {
    fn from(error: std::str::Utf8Error) -> Self {
        HookError::CStrConvertFailed(error)
    }
}

pub type Result<T> = std::result::Result<T, HookError>;
