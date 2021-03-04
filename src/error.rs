// -----------------------------------------------------------------------------

use std::fmt;
use std::io;
use std::path;

// -----------------------------------------------------------------------------

pub type Return = Result<(), Error>;

// -----------------------------------------------------------------------------

/// Error structure
#[derive(Clone, Debug)]
pub struct Error {
    /// Description string
    description: String,

    /// Kind of error
    kind: ErrorKind,
}

/// List of error kinds
#[derive(Clone, Debug)]
pub enum ErrorKind {
    Command(String),
    Filesystem(path::PathBuf),
    Generic,
    InvalidValue(String),
    Io(String),
    Json(String),
    Process(std::process::ExitStatus),
}

impl Error {
    pub fn command(command_name: &str, error: io::Error) -> Self {
        Self {
            description: error.to_string(),
            kind: ErrorKind::Command(command_name.to_string())
        }
    }

    pub fn filesystem(path: path::PathBuf, error: io::Error) -> Self {
        Self {
            description: error.to_string(),
            kind: ErrorKind::Filesystem(path),
        }
    }

    pub fn generic(description: &str) -> Self {
        Self {
            description: description.to_string(),
            kind: ErrorKind::Generic,
        }
    }

    pub fn invalid_value(field: &str) -> Self {
        Self {
            description: "".to_string(),
            kind: ErrorKind::InvalidValue(field.to_string()),
        }
    }

    pub fn io(description: &str, error: io::Error) -> Self {
        Self {
            description: description.to_string(),
            kind: ErrorKind::Io(error.to_string()),
        }
    }

    pub fn json(source: &str, error: serde_json::error::Error) -> Self {
        Self {
            description: error.to_string(),
            kind: ErrorKind::Json(source.to_string()),
        }
    }

    pub fn process(status: std::process::ExitStatus, name: &str) -> Self {
        Self {
            description: name.to_string(),
            kind: ErrorKind::Process(status),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ErrorKind::Command(command_name) => {
                write!(f, "(CMD) {} => {}", command_name, self.description)
            },

            ErrorKind::Filesystem(path) => {
                write!(f, "(FILESYSTEM) {:?} => {}", path, self.description)
            },

            ErrorKind::Generic => {
                write!(f, "(GENERIC) {}", self.description)
            },

            ErrorKind::InvalidValue(field) => {
                write!(f, "(GENERIC) Invalid value for {}", field)
            },

            ErrorKind::Io(error) => {
                write!(f, "(IO) {} => {}", self.description, error)
            },

            ErrorKind::Json(source) => {
                write!(f, "(JSON) {} => {}", source, self.description)
            },

            ErrorKind::Process(status) => {
                match status.code() {
                    Some(c) => write!(
                        f,
                        "(PROCESS) `{}` returned {}",
                        self.description,
                        c),

                    None => write!(
                        f,
                        "(PROCESS) `{}`: no error code",
                        self.description),
                }
            },
        }
    }
}

#[macro_export]
macro_rules! cmd_error {
    ($command: expr, $error: expr) => {
        Err(error::Error::command($command, $error))
    }
}

#[macro_export]
macro_rules! fs_error {
    ($path: expr, $error: expr) => {
        Err(error::Error::filesystem($path, $error))
    }
}

#[macro_export]
macro_rules! generic_error {
    ($description: expr) => { Err(error::Error::generic($description)) }
}

#[macro_export]
macro_rules! inval_error {
    ($field: expr) => { Err(error::Error::invalid_value($field)) }
}

#[macro_export]
macro_rules! io_error {
    ($description: expr, $error: expr) => {
        Err(error::Error::io($description, $error))
    }
}

#[macro_export]
macro_rules! json_error {
    ($source: expr, $error: expr) => {
        Err(error::Error::json($source, $error))
    }
}

#[macro_export]
macro_rules! process_error {
    ($name: expr, $status: expr) => {
        Err(error::Error::process($status, $name))
    }
}

#[macro_export]
macro_rules! unknown_val_error {
    ($description: expr) => {
        Err(error::Error::unknown_value($description))
    }
}

#[macro_export]
macro_rules! Success {
    () => {
        Ok(())
    }
}
