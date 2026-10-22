use crate::ExportOptions;
use reqwest::Client;
use reqwest::StatusCode;
use url::Url;

pub async fn fetch_page(
    initial_url: &Url,
    validate_url: fn(&Url) -> Result<(), String>,
    options: &ExportOptions,
) -> Result<String, String> {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(options.request_timeout)
        .user_agent(format!("chatbot-exporter/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("Could not initialize the HTTP client: {error}"))?;
    let mut url = initial_url.clone();

    for redirect_count in 0..=options.max_redirects {
        let response = client
            .get(url.clone())
            .header("accept", "text/html,application/xhtml+xml,application/json")
            .send()
            .await
            .map_err(|error| format!("Could not fetch the provider page: {error}"))?;

        if is_redirect(response.status()) {
            if redirect_count == options.max_redirects {
                return Err("The provider page redirected too many times.".to_owned());
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| {
                    "The provider redirected without providing a destination.".to_owned()
                })?
                .to_str()
                .map_err(|_| "The redirect destination is invalid.".to_owned())?;
            url = resolve_redirect(
                &url,
                location,
                redirect_count,
                options.max_redirects,
                validate_url,
            )?;
            continue;
        }

        if !response.status().is_success() {
            return Err(format!(
                "Could not fetch the provider page (HTTP {}). Check that the conversation link is publicly accessible and try again.",
                response.status().as_u16()
            ));
        }

        return response
            .text()
            .await
            .map_err(|error| format!("Could not read the provider page: {error}"));
    }

    Err("Could not fetch the provider page.".to_owned())
}

fn is_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

fn resolve_redirect(
    current: &Url,
    location: &str,
    redirect_count: usize,
    max_redirects: usize,
    validate_url: fn(&Url) -> Result<(), String>,
) -> Result<Url, String> {
    if redirect_count >= max_redirects {
        return Err("The provider page redirected too many times.".to_owned());
    }
    let resolved = current
        .join(location)
        .map_err(|_| "The redirect destination is invalid.".to_owned())?;
    validate_url(&resolved)?;
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::chatgpt::ChatGptProvider;
    use std::io;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    struct MockResponse {
        status: &'static str,
        headers: &'static str,
        body: &'static str,
        delay: Duration,
    }

    async fn mock_server(
        responses: Vec<MockResponse>,
    ) -> (Url, JoinHandle<io::Result<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let url = Url::parse(&format!("http://{address}/start")).unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().await?;
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                loop {
                    let bytes_read = stream.read(&mut buffer).await?;
                    if bytes_read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..bytes_read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                requests.push(String::from_utf8_lossy(&request).into_owned());
                tokio::time::sleep(response.delay).await;
                let response_text = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                    response.status,
                    response.body.len(),
                    response.headers,
                    response.body
                );
                stream.write_all(response_text.as_bytes()).await?;
            }
            Ok(requests)
        });
        (url, server)
    }

    fn validate_local_url(url: &Url) -> Result<(), String> {
        if url.scheme() == "http" && url.host_str() == Some("127.0.0.1") {
            Ok(())
        } else {
            Err("Only local test URLs are supported.".to_owned())
        }
    }

    #[test]
    fn resolves_relative_redirects_and_revalidates_destination() {
        let original = Url::parse("https://chatgpt.com/share/abc").unwrap();
        assert_eq!(
            resolve_redirect(
                &original,
                "/share/new-id",
                0,
                5,
                ChatGptProvider::validate_share_url,
            )
            .unwrap()
            .as_str(),
            "https://chatgpt.com/share/new-id"
        );
        assert!(
            resolve_redirect(
                &original,
                "https://example.com/",
                0,
                5,
                ChatGptProvider::validate_share_url
            )
            .is_err()
        );
        assert!(
            resolve_redirect(
                &original,
                "/share/next/extra",
                0,
                5,
                ChatGptProvider::validate_share_url
            )
            .is_err()
        );
    }

    #[test]
    fn enforces_redirect_limit() {
        let original = Url::parse("https://chatgpt.com/share/abc").unwrap();
        assert!(
            resolve_redirect(
                &original,
                "/share/next",
                5,
                5,
                ChatGptProvider::validate_share_url
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn follows_redirect_chains_and_sends_user_agent() {
        let (url, server) = mock_server(vec![
            MockResponse {
                status: "302 Found",
                headers: "Location: /middle\r\n",
                body: "",
                delay: Duration::ZERO,
            },
            MockResponse {
                status: "307 Temporary Redirect",
                headers: "Location: /final\r\n",
                body: "",
                delay: Duration::ZERO,
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: "page contents",
                delay: Duration::ZERO,
            },
        ])
        .await;

        let page = fetch_page(&url, validate_local_url, &ExportOptions::default())
            .await
            .unwrap();
        let requests = server.await.unwrap().unwrap();

        assert_eq!(page, "page contents");
        assert_eq!(requests.len(), 3);
        for (request, path) in requests.iter().zip(["/start", "/middle", "/final"]) {
            assert!(request.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
            assert!(
                request.to_ascii_lowercase().contains(
                    &format!(
                        "User-Agent: chatbot-exporter/{}\r\n",
                        env!("CARGO_PKG_VERSION")
                    )
                    .to_ascii_lowercase()
                ),
                "{request}"
            );
        }
    }

    #[tokio::test]
    async fn reports_non_success_http_status() {
        let (url, server) = mock_server(vec![MockResponse {
            status: "404 Not Found",
            headers: "",
            body: "not found",
            delay: Duration::ZERO,
        }])
        .await;

        let error = fetch_page(&url, validate_local_url, &ExportOptions::default())
            .await
            .unwrap_err();
        server.await.unwrap().unwrap();

        assert!(error.contains("HTTP 404"), "{error}");
    }

    #[tokio::test]
    async fn enforces_request_timeout() {
        let (url, server) = mock_server(vec![MockResponse {
            status: "200 OK",
            headers: "",
            body: "too late",
            delay: Duration::from_secs(1),
        }])
        .await;
        let options = ExportOptions {
            request_timeout: Duration::from_millis(100),
            ..ExportOptions::default()
        };

        let started = tokio::time::Instant::now();
        let error = fetch_page(&url, validate_local_url, &options)
            .await
            .unwrap_err();
        let elapsed = started.elapsed();
        server.abort();
        let _ = server.await;

        assert!(error.contains("Could not fetch"), "{error}");
        assert!(
            elapsed < Duration::from_millis(800),
            "request took {elapsed:?}, exceeding the configured timeout"
        );
    }
}
