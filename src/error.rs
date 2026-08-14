use std::fmt;

#[derive(Debug)]
pub enum AppError {
    ConfigResolve(String),
    ConfigRead {
        path: String,
        source: std::io::Error,
    },
    ConfigParse {
        path: String,
        message: String,
    },
    ConfigWrite {
        path: String,
        source: std::io::Error,
    },
    ConfigEncode(String),
    InvalidValue {
        key: &'static str,
        message: String,
    },
    RenderFailed(String),
    CommandFailed {
        command: String,
        message: String,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigResolve(msg) => write!(f, "{msg}"),
            Self::ConfigRead { path, source } => {
                write!(f, "failed to read config file {path}: {source}")
            }
            Self::ConfigParse { path, message } => {
                write!(f, "failed to parse config file {path}: {message}")
            }
            Self::ConfigWrite { path, source } => {
                write!(f, "failed to write config file {path}: {source}")
            }
            Self::ConfigEncode(msg) => write!(f, "failed to encode config: {msg}"),
            Self::InvalidValue { key, message } => write!(f, "invalid value for {key}: {message}"),
            Self::RenderFailed(msg) => write!(f, "{msg}"),
            Self::CommandFailed { command, message } => {
                write!(f, "command `{command}` failed: {message}")
            }
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConfigRead { source, .. } | Self::ConfigWrite { source, .. } => Some(source),
            _ => None,
        }
    }
}
