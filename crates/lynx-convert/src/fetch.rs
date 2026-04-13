use anyhow::Result;
use lynx_core::error::LynxError;
use std::path::Path;

/// Resolved source for a theme file.
pub enum Source {
    /// Local file path.
    Local(String),
    /// Remote URL (HTTPS only).
    Remote(String),
}

/// Detect the source type from a user-provided string.
/// Converts GitHub blob URLs to raw URLs automatically.
pub fn resolve_source(input: &str) -> Result<Source> {
    if Path::new(input).exists() {
        return Ok(Source::Local(input.to_string()));
    }

    if input.starts_with("https://") || input.starts_with("http://") {
        let url = normalize_github_url(input);
        if !url.starts_with("https://") {
            return Err(
                LynxError::Theme("only HTTPS URLs are supported for security".into()).into(),
            );
        }
        return Ok(Source::Remote(url));
    }

    Err(LynxError::Theme(format!("not a valid file path or URL: {input}")).into())
}

/// Convert GitHub blob URLs to raw.githubusercontent.com URLs.
/// Example: https://github.com/user/repo/blob/master/file.zsh-theme
///       -> https://raw.githubusercontent.com/user/repo/master/file.zsh-theme
fn normalize_github_url(url: &str) -> String {
    if url.contains("github.com") && url.contains("/blob/") {
        url.replace("github.com", "raw.githubusercontent.com")
            .replace("/blob/", "/")
    } else {
        url.to_string()
    }
}

/// Fetch content from a resolved source.
pub fn fetch_content(source: &Source) -> Result<String> {
    match source {
        Source::Local(path) => Ok(std::fs::read_to_string(path)?),
        Source::Remote(url) => {
            let resp = ureq::get(url).call()?;
            Ok(resp.into_string()?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_blob_url_normalized() {
        let url = "https://github.com/ohmyzsh/ohmyzsh/blob/master/themes/robbyrussell.zsh-theme";
        let raw = normalize_github_url(url);
        assert!(raw.contains("raw.githubusercontent.com"));
        assert!(!raw.contains("/blob/"));
    }

    #[test]
    fn raw_url_unchanged() {
        let url = "https://raw.githubusercontent.com/user/repo/main/file.zsh-theme";
        let raw = normalize_github_url(url);
        assert_eq!(raw, url);
    }

    #[test]
    fn local_file_detected() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let result = resolve_source(tmp.path().to_str().unwrap());
        assert!(matches!(result.unwrap(), Source::Local(_)));
    }

    #[test]
    fn https_url_accepted() {
        let result = resolve_source("https://example.com/theme.zsh-theme");
        assert!(matches!(result.unwrap(), Source::Remote(_)));
    }

    #[test]
    fn http_rejected() {
        let result = resolve_source("http://example.com/theme.zsh-theme");
        assert!(result.is_err());
    }

    #[test]
    fn nonexistent_path_errors() {
        let result = resolve_source("/nonexistent/path/to/theme");
        assert!(result.is_err());
    }
}
