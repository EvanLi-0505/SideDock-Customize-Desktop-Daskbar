use serde::Serialize;

/// Single error type for the whole backend.
///
/// It is intentionally string based: every error ends up either in the log file
/// or serialized back to a webview, so a readable message is all we need.
#[derive(Debug, Clone)]
pub struct AppError(pub String);

pub type Result<T, E = AppError> = std::result::Result<T, E>;

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

macro_rules! impl_from {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for AppError {
                fn from(err: $ty) -> Self {
                    AppError(err.to_string())
                }
            }
        )*
    };
}

impl_from!(
    String,
    &str,
    std::io::Error,
    serde_json::Error,
    windows_core::Error,
    tauri::Error,
    std::string::FromUtf16Error,
);

/// Logs the error (if any) instead of propagating it.
pub trait ResultLogExt {
    fn log_error(self);
}

impl<T, E: std::fmt::Display> ResultLogExt for std::result::Result<T, E> {
    #[track_caller]
    fn log_error(self) {
        if let Err(err) = self {
            let loc = std::panic::Location::caller();
            log::error!("{err} (at {}:{})", loc.file(), loc.line());
        }
    }
}
