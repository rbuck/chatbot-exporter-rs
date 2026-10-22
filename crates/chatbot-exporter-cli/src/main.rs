use chatbot_exporter::{ExportOptions, ProviderRegistry, export_one, export_stdin};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use tokio::io::BufReader;

enum RunFailure {
    Item(String),
    Fatal(String),
}

enum InputMode {
    One {
        source: String,
        output: Option<PathBuf>,
    },
    Stdin,
}

fn usage() -> &'static str {
    "Usage: chatbot-exporter <share-url> [output.md] [options]\n\
     \x20      chatbot-exporter --stdin [options]\n\
     \x20      cat urls.txt | chatbot-exporter [options]\n\
     \x20      xargs -n1 chatbot-exporter [options]\n\
     Options:\n\
     \x20 --output-dir DIR          Directory for title-derived output files\n\
     \x20 --timeout-secs SECONDS    Per-request timeout (default: 60)\n\
     \x20 --max-redirects COUNT     Maximum redirects per provider (default: 5)\n\
     \x20 -h, --help                Show this help"
}

fn parse_args() -> Result<Option<(InputMode, ExportOptions)>, String> {
    let mut args = std::env::args().skip(1).peekable();
    let mut options = ExportOptions::default();
    let mut positional = Vec::new();
    let mut stdin_requested = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--stdin" => stdin_requested = true,
            "--output-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--output-dir requires a directory path.".to_owned())?;
                options.output_directory = PathBuf::from(value);
            }
            "--timeout-secs" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--timeout-secs requires a positive integer.".to_owned())?;
                let seconds = value
                    .parse::<u64>()
                    .ok()
                    .filter(|seconds| *seconds > 0)
                    .ok_or_else(|| "--timeout-secs requires a positive integer.".to_owned())?;
                options.request_timeout = std::time::Duration::from_secs(seconds);
            }
            "--max-redirects" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--max-redirects requires a non-negative integer.".to_owned())?;
                options.max_redirects = value
                    .parse::<usize>()
                    .map_err(|_| "--max-redirects requires a non-negative integer.".to_owned())?;
            }
            "-h" | "--help" => {
                println!("{}", usage());
                return Ok(None);
            }
            value if value.starts_with('-') && value != "-" => {
                return Err(format!("Unknown option: {value}"));
            }
            value => positional.push(value.to_owned()),
        }
    }

    if stdin_requested || positional.first().is_some_and(|value| value == "-") {
        if positional.len() > 1 || (positional.len() == 1 && positional[0] != "-") {
            return Err("Stdin mode does not accept a URL or output path.".to_owned());
        }
        return Ok(Some((InputMode::Stdin, options)));
    }

    if positional.is_empty() {
        if std::io::stdin().is_terminal() {
            return Err("No URL supplied.".to_owned());
        }
        return Ok(Some((InputMode::Stdin, options)));
    }
    if positional.len() > 2 {
        return Err("Too many positional arguments.".to_owned());
    }

    let source = positional.remove(0);
    let output = positional.pop().map(PathBuf::from);
    Ok(Some((InputMode::One { source, output }, options)))
}

async fn run() -> Result<(), RunFailure> {
    let Some((input, options)) = parse_args().map_err(RunFailure::Fatal)? else {
        return Ok(());
    };
    options.validate().map_err(RunFailure::Fatal)?;
    tokio::fs::create_dir_all(&options.output_directory)
        .await
        .map_err(|error| {
            RunFailure::Fatal(format!(
                "Could not prepare output directory {}: {error}",
                options.output_directory.display()
            ))
        })?;
    let registry = ProviderRegistry::default();

    match input {
        InputMode::One { source, output } => {
            let path = export_one(&registry, &source, &options, output.as_deref())
                .await
                .map_err(RunFailure::Item)?;
            print_saved(&path);
        }
        InputMode::Stdin => {
            let stdin = tokio::io::stdin();
            let report = export_stdin(BufReader::new(stdin), &registry, &options)
                .await
                .map_err(RunFailure::Fatal)?;
            for path in &report.exported_files {
                print_saved(path);
            }
            for failure in &report.failures {
                eprintln!("Line {}: {}", failure.line_number, failure.message);
            }
            if !report.failures.is_empty() {
                return Err(RunFailure::Item(format!(
                    "Failed to export {} URL(s); see errors above.",
                    report.failures.len()
                )));
            }
        }
    }
    Ok(())
}

fn print_saved(path: &Path) {
    println!(
        "Saved conversation to {}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    );
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(()) => {}
        Err(RunFailure::Item(message)) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
        Err(RunFailure::Fatal(message)) => {
            eprintln!("{message}\n\n{}", usage());
            std::process::exit(255);
        }
    }
}
