# SNS Topic for ML Prediction Notifications (kept for potential future use)
resource "aws_sns_topic" "ml_predictions" {
  name = "ml-prediction-notifications"

  tags = {
    Name = "ML Prediction Notifications"
  }
}

# NOTE: SNS subscription to sns_notification Lambda removed
# We now use SQS queue approach:
# ai_summary → sns-notification-queue (SQS) → sns_notification Lambda → emails


