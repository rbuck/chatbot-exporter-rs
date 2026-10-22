# chatbot-exporter-rs

An asynchronous Rust workspace for exporting publicly shared ChatGPT
conversations as Markdown. Gemini is not currently supported.

## Workspace

- `crates/chatbot-exporter`: reusable library with provider dispatch,
  asynchronous download, decoding, Markdown formatting, output naming, file
  writing, and single-URL/stdin export APIs.
- `crates/chatbot-exporter-cli`: small command-line application. Its executable
  is named `chatbot-exporter`.

## Build and test

From this directory:

```sh
make help
make test
make build
make release
```

- `make build` creates an unoptimized workspace build.
- `make release` creates the optimized CLI binary for the current Rust host
  target.
- `make all` cleans build and package artifacts, runs workspace tests, then
  creates both debug and optimized release builds.
- `make precommit` runs formatting, Clippy with warnings denied, and tests.

## Package a GitHub release asset

`make package` builds the optimized CLI and creates a tar.gz for the current
Rust host target under `dist/`:

```sh
make package
```

The archive is named
`chatbot-exporter-<version>-<rust-target>.tar.gz` and contains a
`chatbot-exporter/` directory with the CLI binary, this README, and the MIT
`LICENSE`. For example, extract it and run the bundled command:

```sh
tar -xzf dist/chatbot-exporter-0.1.0-aarch64-apple-darwin.tar.gz
./chatbot-exporter/chatbot-exporter --help
```

The package is host-target-only by default. Cross-compilation and a GitHub
release upload workflow are not included.

## Build a Debian Bookworm Linux package with Docker

Docker builds the release inside the official Rust Bookworm image, with GNU
Make installed. The workspace is mounted into the container, so the package
appears in the host's `dist/` directory:

```sh
make docker-package
```

The archive filename includes the Linux Rust target for the Docker
architecture, for example
`dist/chatbot-exporter-0.1.0-x86_64-unknown-linux-gnu.tar.gz`. The command
builds for Docker's default architecture; it does not run the test suite. To
cross-build for a specific architecture, pass Docker's platform name to both
the image build and container run:

```sh
make docker-package DOCKER_PLATFORM=linux/amd64
make docker-package DOCKER_PLATFORM=linux/arm64
```

Docker may need emulation support enabled to build and run a non-native
architecture.

Build packages for both supported Linux architectures in sequence with:

```sh
make docker-packages
```

This creates `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`
archives in `dist/`.

## Use the CLI

Export one conversation:

```sh
cargo run -p chatbot-exporter-cli -- \
  'https://chatgpt.com/share/your-share-id'
```

The output filename is derived from the conversation title. An explicit
filename is also accepted:

```sh
cargo run -p chatbot-exporter-cli -- \
  'https://chatgpt.com/share/your-share-id' ./conversation.md
```

By default, output goes to the current directory. Configure the output
directory, request timeout, and maximum redirects with:

```sh
cargo run -p chatbot-exporter-cli -- \
  --output-dir ./exports \
  --timeout-secs 90 \
  --max-redirects 8 \
  'https://chatgpt.com/share/your-share-id'
```

Existing output files are never overwritten; a collision is reported as an
error.

## Sequential URL input

Provide one URL per line. Blank lines and lines beginning with `#` are ignored.
Each line is processed before the next; a URL failure is reported with its
line number and processing continues. The command exits with status 1 if any
line failed:

```sh
cat share-urls.txt | cargo run -p chatbot-exporter-cli --
```

An explicit stdin marker can be used as well:

```sh
cat share-urls.txt | cargo run -p chatbot-exporter-cli -- --stdin
```

## xargs

Single-URL mode is compatible with xargs. An individual export failure exits
with status 1 so xargs continues to later URLs. Status 255 is reserved for
fatal CLI/configuration or stdin-read errors:

```sh
cat share-urls.txt | xargs -n1 ./chatbot-exporter/chatbot-exporter
```

This assumes the release archive has been extracted into the current
directory. For xargs to continue after an individual export fails, do not
enable options that abort on the first nonzero child status.

## Library configuration

`chatbot_exporter::ExportOptions` has these defaults:

- Request timeout: 60 seconds.
- Maximum redirects: 5, with every redirect revalidated by the selected
  provider.
- Output directory: current directory.
- Existing files: refused.
- HTTP User-Agent: `chatbot-exporter/<library version>`.

The public async `export_one`, `export_stdin`, and `write_markdown` functions
are available from the library crate. `ProviderRegistry` selects an
implementation by URL host. The `ConversationProvider` trait isolates
provider-specific URL validation, downloading, decoding, and Markdown
formatting.

The default registry currently contains only the ChatGPT provider. Gemini is
not supported.

## Known Issues and Follow-ups

This table records recommendations from the project review. Items remaining
items are cleanup or release improvements.

| Status | Item | Notes |
|---|---|---|
| **Open** | Provider-specific output metadata | Output filename generation currently reads the title from the Markdown heading. Consider returning title/metadata separately from formatted Markdown. |
| **Open** | Automated GitHub releases | Add a CI target matrix, GitHub Release upload, and checksums if multi-platform automated releases are desired. |
| **Open** | Response-size limit | Consider a configurable maximum response size when accepting arbitrary URLs in batch input. |
