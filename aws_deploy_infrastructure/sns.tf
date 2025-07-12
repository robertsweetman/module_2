# SNS Topic for ML Prediction Notifications
resource "aws_sns_topic" "ml_predictions" {
  name = "ml-prediction-notifications"

  tags = {
    Name = "ML Prediction Notifications"
  }
}

# SNS Topic subscription for email notifications (example)
# You can add email subscriptions here or manage them through AWS Console
resource "aws_sns_topic_subscription" "email_notification" {
  count     = length(var.notification_emails) > 0 ? length(var.notification_emails) : 0
  topic_arn = aws_sns_topic.ml_predictions.arn
  protocol  = "email"
  endpoint  = var.notification_emails[count.index]
}

# Output the topic ARN for use in other resources
output "sns_topic_arn" {
  value = aws_sns_topic.ml_predictions.arn
  description = "ARN of the ML predictions SNS topic"
}
