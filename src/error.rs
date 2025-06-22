use std::fmt;

/// Specific error types for the launch.json generator
#[derive(Debug)]
pub enum GeneratorError {
    /// Configuration root directory does not exist
    ConfigDirectoryNotFound(String),
    /// Templates subdirectory does not exist
    TemplatesDirectoryNotFound(String),
    /// Referenced template file not found
    TemplateNotFound(String),
    /// Configuration file not found
    ConfigNotFound(String),
    /// Invalid extends field value (contains path separators)
    InvalidExtendsValue(String, String),
    /// Multiple configs have the same name
    DuplicateConfigName(String, Vec<String>),
    /// No JSON config files found in configs directory
    NoConfigFiles(String),
    /// JSON parsing failed
    JsonParseError(String, String),
    /// File read operation failed
    FileReadError(String, String),
    /// File write operation failed
    FileWriteError(String, String),
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneratorError::ConfigDirectoryNotFound(path) => {
                write!(f, "Config directory does not exist: {}", path)
            }
            GeneratorError::TemplatesDirectoryNotFound(path) => {
                write!(f, "Templates directory does not exist: {}", path)
            }
            GeneratorError::TemplateNotFound(name) => {
                write!(f, "Base template '{}' not found", name)
            }
            GeneratorError::ConfigNotFound(path) => {
                write!(f, "Config file not found: {}", path)
            }
            GeneratorError::InvalidExtendsValue(value, file) => {
                write!(
                    f,
                    "Invalid extends value '{}' in {}\nOnly template names are allowed (e.g., 'cpp', 'lldb')",
                    value, file
                )
            }
            GeneratorError::DuplicateConfigName(name, files) => {
                write!(
                    f,
                    "Duplicate configuration name '{}' found in:\n{}\nEach configuration must have a unique name.",
                    name,
                    files.join("\n")
                )
            }
            GeneratorError::NoConfigFiles(dir) => {
                write!(f, "No configuration files found in: {}", dir)
            }
            GeneratorError::JsonParseError(file, error) => {
                write!(f, "Failed to parse JSON in {}: {}", file, error)
            }
            GeneratorError::FileReadError(file, error) => {
                write!(f, "Failed to read file {}: {}", file, error)
            }
            GeneratorError::FileWriteError(file, error) => {
                write!(f, "Failed to write file {}: {}", file, error)
            }
        }
    }
}

impl std::error::Error for GeneratorError {}
