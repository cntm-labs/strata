use chorus::prelude::*;
use chorus::providers::email::resend::ResendEmailSender;
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
            return Ok(());
        };

        chorus
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
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
