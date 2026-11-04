//! Shared HTTP client for backend-only network calls (the frontend never
//! calls third-party APIs).

use std::sync::OnceLock;
use std::time::Duration;

use crate::error::AppError;

pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(concat!("Ludotheque/", env!("CARGO_PKG_VERSION")))
            .build()
            .into()
    })
}

/// Converts a ureq error into an app error, removing `secret` (an API key that
/// may be part of the URL) from the message.
pub fn network_error(e: ureq::Error, secret: Option<&str>) -> AppError {
    let message = match &e {
        ureq::Error::StatusCode(code) => format!("HTTP {code}"),
        other => other.to_string(),
    };
    let message = match secret.filter(|s| !s.is_empty()) {
        Some(s) => message.replace(s, "***"),
        None => message,
    };
    AppError::Network {
        status: status_of(&e),
        message,
    }
}

fn status_of(e: &ureq::Error) -> Option<u16> {
    match e {
        ureq::Error::StatusCode(code) => Some(*code),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_errors_hide_the_secret() {
        let e = ureq::Error::BadUri("https://x/?key=SECRETKEY".into());
        let err = network_error(e, Some("SECRETKEY"));
        assert!(!err.to_string().contains("SECRETKEY"));
        assert!(matches!(
            network_error(ureq::Error::StatusCode(403), None),
            AppError::Network {
                status: Some(403),
                ..
            }
        ));
    }
}
