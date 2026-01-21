use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum DiffCopyError {
    #[error("Source directory '{0}' does not exist")]
    SourceNotFound(PathBuf),

    #[error("Target directory '{0}' does not exist")]
    TargetNotFound(PathBuf),

    #[error("Base directory '{0}' does not exist")]
    BaseNotFound(PathBuf),

    #[error("Output directory '{0}' already exists. Use --force to overwrite.")]
    OutputExists(PathBuf),

    #[error("Cannot use --force on protected path '{0}'")]
    ProtectedPath(PathBuf),

    #[error("Failed to read config file '{path}': {source}")]
    ConfigReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config file '{path}': {message}")]
    ConfigParseError { path: PathBuf, message: String },

    #[error("Missing required field '{0}' in config file")]
    MissingConfigField(String),

    #[error("Invalid glob pattern '{0}'")]
    InvalidGlobPattern(String),

    #[error("Failed to read file '{path}': {message}")]
    FileReadError { path: PathBuf, message: String },

    #[error("Failed to write file '{path}': {message}")]
    FileWriteError { path: PathBuf, message: String },

    #[error("Failed to copy file '{source_path}' to '{dest}': {message}")]
    FileCopyError {
        source_path: PathBuf,
        dest: PathBuf,
        message: String,
    },

    #[error("Failed to create directory '{0}': {1}")]
    DirectoryCreateError(PathBuf, String),

    #[error("Failed to delete directory '{0}': {1}")]
    DirectoryDeleteError(PathBuf, String),

    #[error("Permission denied: '{0}'")]
    PermissionDenied(PathBuf),

    #[error("User cancelled operation")]
    UserCancelled,

    #[error("Invalid filter status: '{0}'")]
    InvalidFilterStatus(String),

    #[error("Invalid check permissions mode: '{0}'")]
    InvalidCheckPermissionsMode(String),

    #[error("Invalid merge style: '{0}'")]
    InvalidMergeStyle(String),

    #[error("Invalid color mode: '{0}'")]
    InvalidColorMode(String),

    #[error("Invalid log level: '{0}'")]
    InvalidLogLevel(String),

    #[error("Three-way mode requires --base option")]
    ThreeWayRequiresBase,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, DiffCopyError>;
