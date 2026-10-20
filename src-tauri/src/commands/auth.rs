use crate::error::Result;
use tauri::Url;

/// Allowlist of URL schemes that may be handed to the OS shell opener.
/// `javascript:`, `data:`, `vbscript:`, `file:` etc. are refused — a remote
/// page must never be able to make the desktop shell execute arbitrary
/// content through this command. (v2.1.0: the command previously forwarded
/// any string straight to `open::that`.)
const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// Shared by the command and new-window router before invoking the OS opener.
pub(crate) fn is_allowed_external_url(url: &Url) -> bool {
    ALLOWED_SCHEMES.contains(&url.scheme())
}

/// Open a URL in the OS default browser / handler.
///
/// Called from the injected page engine (`window.__tiktoknow_open`) for every
/// external link the user clicks inside TikTok, so it is deliberately strict:
/// only real web/e-mail URLs pass; everything else is rejected with an error.
#[tauri::command]
pub async fn jump_to_external(url: String) -> Result<()> {
    let parsed =
        Url::parse(&url).map_err(|e| crate::error::AppError::Custom(format!("invalid URL: {e}")))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if !is_allowed_external_url(&parsed) {
        return Err(crate::error::AppError::Custom(format!(
            "scheme '{scheme}' is not allowed for external opening"
        )));
    }
    open::that(parsed.as_str()).map_err(|e| crate::error::AppError::Custom(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_allowed_external_url;
    use tauri::Url;

    #[test]
    fn external_url_allows_web_and_email_schemes() {
        for url in [
            "http://example.com/path",
            "https://example.com/path",
            "HTTPS://example.com/path",
            "mailto:user@example.com",
        ] {
            assert!(is_allowed_external_url(&Url::parse(url).unwrap()), "{url}");
        }
    }

    #[test]
    fn external_url_rejects_executable_local_and_custom_schemes() {
        for url in [
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "file:///C:/Windows/System32/cmd.exe",
            "vbscript:msgbox(1)",
            "ms-settings:privacy",
            "tiktok://user/profile",
        ] {
            assert!(!is_allowed_external_url(&Url::parse(url).unwrap()), "{url}");
        }
    }
}
