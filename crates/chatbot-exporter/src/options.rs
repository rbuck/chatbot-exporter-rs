use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub request_timeout: Duration,
    pub max_redirects: usize,
    pub output_directory: PathBuf,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(60),
            max_redirects: 5,
            output_directory: PathBuf::from("."),
        }
    }
}

impl ExportOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.request_timeout.is_zero() {
            return Err("Request timeout must be greater than zero.".to_owned());
        }
        if self.output_directory.as_os_str().is_empty() {
            return Err("Output directory must not be empty.".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conservative_and_documented() {
        let options = ExportOptions::default();
        assert_eq!(options.request_timeout, Duration::from_secs(60));
        assert_eq!(options.max_redirects, 5);
        assert_eq!(options.output_directory, PathBuf::from("."));
        assert!(options.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_timeout_and_output_directory() {
        let options = ExportOptions {
            request_timeout: Duration::ZERO,
            ..ExportOptions::default()
        };
        assert!(options.validate().is_err());

        let options = ExportOptions {
            output_directory: PathBuf::new(),
            ..ExportOptions::default()
        };
        assert!(options.validate().is_err());
    }
}
