use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_ses::{Client as SesClient, types::Content, types::Body, types::Message, types::Destination};
use handlebars::Handlebars;
use tracing::{info, error, warn};

use crate::types::{Config, SNSMessage, EmailData, NotificationPriority};

pub struct EmailService {
    ses_client: SesClient,
    handlebars: Handlebars<'static>,
    config: Config,
}

impl EmailService {
    pub async fn new(config: &Config) -> Result<Self> {
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .load()
            .await;
       
        let ses_client = SesClient::new(&aws_config);
        let mut handlebars = Handlebars::new();
        
        // Register email templates
        handlebars.register_template_string("email_html", include_str!("../templates/email.hbs"))?;
        handlebars.register_template_string("email_text", include_str!("../templates/email.txt"))?;
        
        Ok(EmailService {
            ses_client,
            handlebars,
            config: config.clone(),
        })
    }

    pub async fn send_notification(&self, sns_message: &SNSMessage) -> Result<()> {
        if self.config.notification_emails.is_empty() {
            warn!("No notification emails configured, skipping email send");
            return Ok(());
        }

        let email_data = EmailData::from_sns_message(sns_message).map_err(|e| anyhow::anyhow!(e))?;
        let priority = NotificationPriority::from(sns_message.priority.as_str());

        info!("Sending {} priority notification for tender: {}", 
              sns_message.priority, email_data.resource_id);

        // Generate email content
        let html_body = self.handlebars.render("email_html", &email_data)?;
        let text_body = self.handlebars.render("email_text", &email_data)?;

        // Determine recipients based on priority
        let recipients = self.get_recipients_for_priority(&priority);

        // Send email using AWS SES
        self.send_ses_email(
            &email_data.subject,
            &html_body,
            &text_body,
            &recipients,
        ).await?;

        info!("Email notification sent successfully to {} recipients", recipients.len());
        Ok(())
    }

    fn get_recipients_for_priority(&self, priority: &NotificationPriority) -> Vec<String> {
        match priority {
            NotificationPriority::Urgent => {
                // Send to all recipients for urgent notifications
                self.config.notification_emails.clone()
            },
            NotificationPriority::High => {
                // Send to all recipients for high priority
                self.config.notification_emails.clone()
            },
            NotificationPriority::Normal => {
                // Send to all recipients for normal priority
                self.config.notification_emails.clone()
            },
        }
    }

    async fn send_ses_email(
        &self,
        subject: &str,
        html_body: &str,
        text_body: &str,
        recipients: &[String],
    ) -> Result<()> {
        if recipients.is_empty() {
            warn!("No recipients specified for email");
            return Ok(());
        }

        info!("Preparing to send email:");
        info!("  From: {}", self.config.from_email);
        info!("  To: {:?}", recipients);
        info!("  Subject: {}", subject);

        let destination = Destination::builder()
            .set_to_addresses(Some(recipients.to_vec()))
            .build();

        let subject_content = Content::builder()
            .data(subject)
            .charset("UTF-8")
            .build()?;

        let html_content = Content::builder()
            .data(html_body)
            .charset("UTF-8")
            .build()?;

        let text_content = Content::builder()
            .data(text_body)
            .charset("UTF-8")
            .build()?;

        let body = Body::builder()
            .html(html_content)
            .text(text_content)
            .build();

        let message = Message::builder()
            .subject(subject_content)
            .body(body)
            .build();

        let send_email_result = self.ses_client
            .send_email()
            .source(&self.config.from_email)
            .destination(destination)
            .message(message)
            .send()
            .await;

        match send_email_result {
            Ok(output) => {
                info!("Email sent successfully. Message ID: {:?}", output.message_id());
                Ok(())
            },
            Err(e) => {
                error!("Failed to send email via SES: {}", e);
                error!("SES Error details: {:?}", e);
                
                // Try to extract more specific error information
                let error_message = format!("{}", e);
                if error_message.contains("MessageRejected") {
                    error!("Email was rejected - check if sender/recipient emails are verified in SES");
                } else if error_message.contains("Throttling") {
                    error!("SES rate limit exceeded");
                } else if error_message.contains("AccessDenied") {
                    error!("Lambda doesn't have permission to use SES");
                }
                
                Err(anyhow::anyhow!("SES send error: {}", e))
            }
        }
    }
}
