use base64::Engine;

/// Unwraps custom download manager URL schemes (Thunder, FlashGet, QQDownload) into standard URLs.
pub fn unwrap_custom_url_scheme(input: &str) -> String {
    let trimmed = input.trim();
    if let Some(payload) = trimmed.strip_prefix("thunder://") {
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(payload.trim()) {
            if let Ok(s) = String::from_utf8(decoded) {
                let inner = s.strip_prefix("AA").unwrap_or(&s);
                let inner = inner.strip_suffix("ZZ").unwrap_or(inner).trim();
                if inner.starts_with("http://") || inner.starts_with("https://") || inner.starts_with("ftp://") {
                    return inner.to_string();
                }
            }
        }
    } else if let Some(payload) = trimmed.strip_prefix("flashget://") {
        let clean = payload.split('&').next().unwrap_or(payload).trim();
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(clean) {
            if let Ok(s) = String::from_utf8(decoded) {
                let inner = s.strip_prefix("[FLASHGET]").unwrap_or(&s);
                let inner = inner.strip_suffix("[FLASHGET]").unwrap_or(inner).trim();
                if inner.starts_with("http://") || inner.starts_with("https://") || inner.starts_with("ftp://") {
                    return inner.to_string();
                }
            }
        }
    } else if let Some(payload) = trimmed.strip_prefix("qqdl://") {
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(payload.trim()) {
            if let Ok(s) = String::from_utf8(decoded) {
                let inner = s.trim();
                if inner.starts_with("http://") || inner.starts_with("https://") || inner.starts_with("ftp://") {
                    return inner.to_string();
                }
            }
        }
    }
    trimmed.to_string()
}

/// Rewrites GitHub or HuggingFace URLs using mirrors if enabled.
pub fn apply_mirrors(url: &str, gh_mirror: Option<&str>, hf_mirror: Option<&str>) -> String {
    let mut current = url.to_string();
    if let Some(ghm) = gh_mirror {
        let trimmed_mirror = ghm.trim_end_matches('/');
        if (current.starts_with("https://github.com/") || current.starts_with("https://raw.githubusercontent.com/"))
            && !current.starts_with(trimmed_mirror)
        {
            current = format!("{}/{}", trimmed_mirror, current);
        }
    }
    if let Some(hfm) = hf_mirror {
        let trimmed_mirror = hfm.trim_end_matches('/');
        if current.starts_with("https://huggingface.co/") {
            current = current.replace("https://huggingface.co", trimmed_mirror);
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_thunder() {
        // "AAhttps://example.com/file.zipZZ" in base64: QUFodHRwczovL2V4YW1wbGUuY29tL2ZpbGUuemlwWlo=
        let thunder = "thunder://QUFodHRwczovL2V4YW1wbGUuY29tL2ZpbGUuemlwWlo=";
        assert_eq!(unwrap_custom_url_scheme(thunder), "https://example.com/file.zip");
    }

    #[test]
    fn test_unwrap_flashget() {
        // "[FLASHGET]https://example.com/file.iso[FLASHGET]" in base64: W0ZMQVNIR0VUXWh0dHBzOi8vZXhhbXBsZS5jb20vZmlsZS5pc29bRkxBU0hHRVRd
        let fg = "flashget://W0ZMQVNIR0VUXWh0dHBzOi8vZXhhbXBsZS5jb20vZmlsZS5pc29bRkxBU0hHRVRd&1234";
        assert_eq!(unwrap_custom_url_scheme(fg), "https://example.com/file.iso");
    }

    #[test]
    fn test_unwrap_qqdl() {
        // "https://example.com/music.mp3" in base64: aHR0cHM6Ly9leGFtcGxlLmNvbS9tdXNpYy5tcDM=
        let qq = "qqdl://aHR0cHM6Ly9leGFtcGxlLmNvbS9tdXNpYy5tcDM=";
        assert_eq!(unwrap_custom_url_scheme(qq), "https://example.com/music.mp3");
    }

    #[test]
    fn test_apply_mirrors() {
        let gh = "https://github.com/owner/repo/releases/download/v1.0/app.exe";
        assert_eq!(
            apply_mirrors(gh, Some("https://ghproxy.net/"), None),
            "https://ghproxy.net/https://github.com/owner/repo/releases/download/v1.0/app.exe"
        );
        let hf = "https://huggingface.co/bert-base-uncased/resolve/main/pytorch_model.bin";
        assert_eq!(
            apply_mirrors(hf, None, Some("https://hf-mirror.com")),
            "https://hf-mirror.com/bert-base-uncased/resolve/main/pytorch_model.bin"
        );
    }
}
