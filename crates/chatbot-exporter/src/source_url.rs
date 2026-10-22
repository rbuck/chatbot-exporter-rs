use url::Url;

pub fn parse_source_url(value: &str) -> Result<Url, String> {
    let url =
        Url::parse(value).map_err(|_| "Provide a valid conversation share URL.".to_owned())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(
            "Only HTTPS conversation URLs without embedded credentials are supported.".to_owned(),
        );
    }
    Ok(url)
}

pub fn canonical_source_url(source_url: &Url) -> String {
    let mut url = source_url.clone();
    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_urls_and_rejects_insecure_or_credentialed_urls() {
        assert!(parse_source_url("https://chatgpt.com/share/id").is_ok());
        assert!(parse_source_url("https://example.com/share/id").is_ok());
        assert!(parse_source_url("http://chatgpt.com/share/id").is_err());
        assert!(parse_source_url("https://user@example.com/share/id").is_err());
        assert!(parse_source_url("not a URL").is_err());
    }

    #[test]
    fn canonical_url_drops_query_fragment_and_trailing_slashes() {
        let url = parse_source_url("https://example.com/share/id/?x=1#part").unwrap();
        assert_eq!(canonical_source_url(&url), "https://example.com/share/id");
    }
}
