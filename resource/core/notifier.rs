use chorus::prelude::*;
use chorus::providers::email::resend::ResendEmailSender;
use metrics::counter;
use std::sync::Arc;

pub struct Notifier {
    chorus: Option<Chorus>,
}

impl Notifier {
    pub fn new(resend_api_key: Option<&str>, from_email: &str) -> Self {
        let chorus = resend_api_key.map(|key| {
            let resend = ResendEmailSender::new(key.to_string(), from_email.to_string());
            Chorus::builder()
                .add_email_provider(Arc::new(resend))
                .default_from_email(from_email.to_string())
                .build()
        });
        Self { chorus }
    }

    pub async fn send_alert_email(
        &self,
        to: &str,
        rule_name: &str,
        message: &str,
    ) -> Result<(), String> {
        let Some(ref chorus) = self.chorus else {
            tracing::warn!("Email not configured — skipping notification to {}", to);
            counter!("strata_email_sent_total", "status" => "skipped").increment(1);
            return Ok(());
        };

        match chorus
            .send_email(&EmailMessage {
                to: to.to_string(),
                subject: format!("[Strata Alert] {}", rule_name),
                html_body: format!(
                    "<h2>Alert: {}</h2><p>{}</p><p><small>Sent by Strata</small></p>",
                    rule_name, message
                ),
                text_body: format!("Alert: {}\n\n{}", rule_name, message),
                from: None,
            })
            .await
        {
            Ok(_) => {
                counter!("strata_email_sent_total", "status" => "success").increment(1);
                Ok(())
            }
            Err(e) => {
                counter!("strata_email_sent_total", "status" => "error").increment(1);
                Err(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_without_api_key_creates_disabled_notifier() {
        let notifier = Notifier::new(None, "test@test.com");
        assert!(notifier.chorus.is_none());
    }

    #[test]
    fn new_with_api_key_creates_enabled_notifier() {
        let notifier = Notifier::new(Some("re_test_key"), "alerts@test.com");
        assert!(notifier.chorus.is_some());
    }

    #[tokio::test]
    async fn send_alert_email_skips_when_not_configured() {
        let notifier = Notifier::new(None, "test@test.com");
        let result = notifier
            .send_alert_email("user@test.com", "CPU Alert", "CPU is at 95%")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn send_alert_email_records_skipped_metric_when_disabled() {
        crate::metrics::install_for_tests();
        let notifier = Notifier::new(None, "test@test.com");
        notifier
            .send_alert_email("user@test.com", "Test", "Body")
            .await
            .unwrap();
        let rendered = crate::metrics::render();
        assert!(
            rendered.contains("strata_email_sent_total")
                && rendered.contains(r#"status="skipped""#),
            "expected skipped counter; got:\n{rendered}"
        );
    }

    #[tokio::test]
    async fn send_alert_email_with_configured_notifier_attempts_send() {
        // Resend API will reject the fake key, but we verify the error path works
        let notifier = Notifier::new(Some("re_fake_key"), "alerts@test.com");
        let result = notifier
            .send_alert_email("user@test.com", "CPU Alert", "CPU is at 95%")
            .await;
        // Should fail because the API key is fake, but should not panic
        assert!(result.is_err());
    }
}
