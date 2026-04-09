use serde::{Deserialize, Serialize};

// ─── Event Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum NotifyEvent {
    DaemonStart,
    DaemonStop,
    DaemonError,
    DataUpdateFailed,
    SevereVolTrigger,
    OverlayApplied,
    TradeExecuted,
}

impl NotifyEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotifyEvent::DaemonStart => "daemon_start",
            NotifyEvent::DaemonStop => "daemon_stop",
            NotifyEvent::DaemonError => "daemon_error",
            NotifyEvent::DataUpdateFailed => "data_update_failed",
            NotifyEvent::SevereVolTrigger => "severe_vol_trigger",
            NotifyEvent::OverlayApplied => "overlay_applied",
            NotifyEvent::TradeExecuted => "trade_executed",
        }
    }
}

// ─── Config ─────────────────────────────────────────────────────

fn default_timeout_secs() -> u64 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyConfig {
    /// Shell command to run on events. Receives QUANTBOT_EVENT, QUANTBOT_DETAIL,
    /// QUANTBOT_TIMESTAMP as environment variables.
    #[serde(default)]
    pub cmd: Option<String>,

    /// Webhook URL for POST notifications.
    #[serde(default)]
    pub webhook_url: Option<String>,

    /// Timeout in seconds for shell commands and HTTP requests.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

// ─── Notifier ───────────────────────────────────────────────────

pub struct Notifier {
    config: NotifyConfig,
    client: reqwest::Client,
}

impl Notifier {
    pub fn new(config: NotifyConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();
        Self { config, client }
    }

    /// Fire-and-forget notification. Spawns async tasks, never propagates errors.
    pub fn notify(&self, event: NotifyEvent, detail: &str) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let event_str = event.as_str().to_string();
        let detail = detail.to_string();

        // Shell command
        if let Some(ref cmd) = self.config.cmd {
            let cmd = cmd.clone();
            let event_s = event_str.clone();
            let detail_s = detail.clone();
            let ts = timestamp.clone();
            let timeout = self.config.timeout_secs;
            tokio::spawn(async move {
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout),
                    tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .env("QUANTBOT_EVENT", &event_s)
                        .env("QUANTBOT_DETAIL", &detail_s)
                        .env("QUANTBOT_TIMESTAMP", &ts)
                        .output(),
                )
                .await;

                match result {
                    Ok(Ok(output)) => {
                        if !output.status.success() {
                            eprintln!(
                                "  notify cmd failed (exit {}): {}",
                                output.status,
                                String::from_utf8_lossy(&output.stderr).trim()
                            );
                        }
                    }
                    Ok(Err(e)) => eprintln!("  notify cmd error: {e}"),
                    Err(_) => eprintln!("  notify cmd timed out after {timeout}s"),
                }
            });
        }

        // Webhook
        if let Some(ref url) = self.config.webhook_url {
            let client = self.client.clone();
            let url = url.clone();
            let payload = serde_json::json!({
                "event": event_str,
                "detail": detail,
                "timestamp": timestamp,
            });
            tokio::spawn(async move {
                match client.post(&url).json(&payload).send().await {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            eprintln!("  notify webhook {}: HTTP {}", url, resp.status());
                        }
                    }
                    Err(e) => eprintln!("  notify webhook error: {e}"),
                }
            });
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_as_str_all_variants() {
        let events = [
            (NotifyEvent::DaemonStart, "daemon_start"),
            (NotifyEvent::DaemonStop, "daemon_stop"),
            (NotifyEvent::DaemonError, "daemon_error"),
            (NotifyEvent::DataUpdateFailed, "data_update_failed"),
            (NotifyEvent::SevereVolTrigger, "severe_vol_trigger"),
            (NotifyEvent::OverlayApplied, "overlay_applied"),
            (NotifyEvent::TradeExecuted, "trade_executed"),
        ];
        for (event, expected) in events {
            assert_eq!(event.as_str(), expected);
        }
    }

    #[test]
    fn webhook_payload_structure() {
        let event = NotifyEvent::TradeExecuted;
        let detail = "GBPUSD=X BUY 0.5";
        let payload = serde_json::json!({
            "event": event.as_str(),
            "detail": detail,
            "timestamp": "2026-04-09T12:00:00Z",
        });
        assert_eq!(payload["event"], "trade_executed");
        assert_eq!(payload["detail"], "GBPUSD=X BUY 0.5");
        assert!(payload["timestamp"].is_string());
    }

    #[test]
    fn notify_config_defaults() {
        let toml_str = r#"
cmd = "echo test"
"#;
        let config: NotifyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cmd.as_deref(), Some("echo test"));
        assert!(config.webhook_url.is_none());
        assert_eq!(config.timeout_secs, 5);
    }
}
