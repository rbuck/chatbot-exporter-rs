mod fetch;
mod filename;
mod options;
mod providers;
mod registry;
mod source_url;

pub use filename::default_output_name;
pub use options::ExportOptions;
pub use providers::{ConversationProvider, chatgpt::ChatGptProvider, claude::ClaudeProvider};
pub use registry::ProviderRegistry;
pub use source_url::{canonical_source_url, parse_source_url};

use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineFailure {
    pub line_number: usize,
    pub message: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct StdinExportReport {
    pub exported_files: Vec<PathBuf>,
    pub failures: Vec<LineFailure>,
}

pub async fn export_one(
    registry: &ProviderRegistry,
    source: &str,
    options: &ExportOptions,
    output_path: Option<&Path>,
) -> Result<PathBuf, String> {
    options.validate()?;
    let source_url = parse_source_url(source)?;
    let markdown = registry.export(&source_url, options).await?;
    let output_path = output_path.map_or_else(
        || {
            options.output_directory.join(default_output_name(
                markdown.lines().find_map(|line| line.strip_prefix("# ")),
            ))
        },
        Path::to_path_buf,
    );

    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            format!(
                "Could not create output directory {}: {error}",
                parent.display()
            )
        })?;
    }
    write_markdown(&output_path, &markdown).await?;
    Ok(output_path)
}

pub async fn export_stdin<R>(
    reader: R,
    registry: &ProviderRegistry,
    options: &ExportOptions,
) -> Result<StdinExportReport, String>
where
    R: AsyncBufRead + Unpin,
{
    options.validate()?;
    let mut lines = reader.lines();
    let mut report = StdinExportReport::default();
    let mut line_number = 0usize;

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|error| format!("Could not read URL input: {error}"))?
    {
        line_number += 1;
        let source = line.trim();
        if source.is_empty() || source.starts_with('#') {
            continue;
        }
        match export_one(registry, source, options, None).await {
            Ok(path) => report.exported_files.push(path),
            Err(message) => report.failures.push(LineFailure {
                line_number,
                message,
            }),
        }
    }
    Ok(report)
}

pub async fn write_markdown(path: &Path, markdown: &str) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temporary_file = create_temporary_file(parent).map_err(|error| {
        format!(
            "Could not create temporary file in {}: {error}",
            parent.display()
        )
    })?;
    let (file, temporary_path) = temporary_file.into_parts();
    let mut file = tokio::fs::File::from_std(file);

    if let Err(error) = file.write_all(markdown.as_bytes()).await {
        drop(file);
        return Err(format!("Could not write temporary file: {error}"));
    }
    if let Err(error) = file.sync_all().await {
        drop(file);
        return Err(format!("Could not flush temporary file: {error}"));
    }
    drop(file);

    temporary_path.persist_noclobber(path).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            format!("Refusing to overwrite existing file: {}", path.display())
        } else {
            format!("Could not publish {}: {}", path.display(), error.error)
        }
    })
}

fn create_temporary_file(parent: &Path) -> std::io::Result<tempfile::NamedTempFile> {
    tempfile::Builder::new()
        .prefix(".chatbot-exporter-")
        .rand_bytes(10)
        .suffix(".tmp")
        .tempfile_in(parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChatGptProvider, providers::ConversationProvider};
    use async_trait::async_trait;
    use std::io::Cursor;
    use url::Url;

    struct FixtureProvider;

    #[async_trait]
    impl ConversationProvider for FixtureProvider {
        fn base_urls(&self) -> &'static [&'static str] {
            &["fixture.example"]
        }

        fn validate_url(&self, url: &Url) -> Result<(), String> {
            (url.path() == "/share/example")
                .then_some(())
                .ok_or_else(|| "Unexpected fixture URL path.".to_owned())
        }

        async fn download(&self, _url: &Url, _options: &ExportOptions) -> Result<String, String> {
            Ok("fixture page".to_owned())
        }

        fn decode(&self, page: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"page": page}))
        }

        fn format(
            &self,
            _conversation: &serde_json::Value,
            _source_url: &Url,
        ) -> Result<String, String> {
            Ok("# Fixture Conversation\n\nExported fixture.".to_owned())
        }
    }

    fn test_directory() -> PathBuf {
        tempfile::Builder::new()
            .prefix("chatbot-exporter-test-")
            .tempdir()
            .unwrap()
            .keep()
    }

    #[test]
    fn temporary_file_uses_a_ten_character_alphanumeric_suffix() {
        let directory = tempfile::tempdir().unwrap();
        let temporary_file = create_temporary_file(directory.path()).unwrap();
        let name = temporary_file.path().file_name().unwrap().to_str().unwrap();
        let random_part = name
            .strip_prefix(".chatbot-exporter-")
            .unwrap()
            .strip_suffix(".tmp")
            .unwrap();

        assert_eq!(random_part.len(), 10);
        assert!(
            random_part
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        );
    }

    #[tokio::test]
    async fn stdin_reports_bad_lines_and_continues_processing() {
        let options = ExportOptions::default();
        let registry = ProviderRegistry::new(vec![Box::new(ChatGptProvider)]);
        let report = export_stdin(
            Cursor::new(b"\n# comment\nnot-a-url\nhttps://example.com/share/id\n".to_vec()),
            &registry,
            &options,
        )
        .await
        .unwrap();

        assert_eq!(report.exported_files.len(), 0);
        assert_eq!(report.failures.len(), 2);
        assert_eq!(report.failures[0].line_number, 3);
        assert_eq!(report.failures[1].line_number, 4);
    }

    #[tokio::test]
    async fn write_markdown_refuses_overwriting_existing_files() {
        let root = test_directory();
        tokio::fs::create_dir_all(&root).await.unwrap();
        let file = root.join("conversation.md");
        tokio::fs::write(&file, "existing").await.unwrap();
        let error = write_markdown(&file, "replacement").await.unwrap_err();

        assert!(error.contains("Refusing to overwrite"));
        assert_eq!(tokio::fs::read_to_string(&file).await.unwrap(), "existing");
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn write_markdown_publishes_complete_content_without_leaving_temporary_files() {
        let root = test_directory();
        tokio::fs::create_dir_all(&root).await.unwrap();
        let file = root.join("conversation.md");

        write_markdown(&file, "complete markdown").await.unwrap();

        assert_eq!(
            tokio::fs::read_to_string(&file).await.unwrap(),
            "complete markdown"
        );
        let mut entries = tokio::fs::read_dir(&root).await.unwrap();
        let entry = entries.next_entry().await.unwrap().unwrap();
        assert_eq!(entry.file_name(), "conversation.md");
        assert!(entries.next_entry().await.unwrap().is_none());
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn export_one_uses_the_configured_directory_and_refuses_collisions() {
        let output_directory = test_directory();
        let options = ExportOptions {
            output_directory: output_directory.clone(),
            ..ExportOptions::default()
        };
        let registry = ProviderRegistry::new(vec![Box::new(FixtureProvider)]);
        let source = "https://fixture.example/share/example";

        let output = export_one(&registry, source, &options, None).await.unwrap();
        assert_eq!(output, output_directory.join("fixture-conversation.md"));
        assert_eq!(
            tokio::fs::read_to_string(&output).await.unwrap(),
            "# Fixture Conversation\n\nExported fixture."
        );
        assert!(
            export_one(&registry, source, &options, None)
                .await
                .unwrap_err()
                .contains("Refusing to overwrite")
        );

        let _ = tokio::fs::remove_dir_all(output_directory).await;
    }
}
