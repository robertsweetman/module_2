# SNS Topic for ML Prediction Notifications
resource "aws_sns_topic" "ml_predictions" {
  name = "ml-prediction-notifications"

  tags = {
    Name = "ML Prediction Notifications"
  }
}

# SNS Topic subscription for lambda notifications
resource "aws_sns_topic_subscription" "lambda_notification" {
  topic_arn = aws_sns_topic.ml_predictions.arn
  protocol  = "lambda"
  endpoint  = aws_lambda_function.sns_notification.arn
}

# Grant SNS permission to invoke the lambda
resource "aws_lambda_permission" "allow_sns" {
  statement_id  = "AllowExecutionFromSNS"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.sns_notification.function_name
  principal     = "sns.amazonaws.com"
  source_arn    = aws_sns_topic.ml_predictions.arn
}

# Output the topic ARN for use in other resources
output "sns_topic_arn" {
  value = aws_sns_topic.ml_predictions.arn
  description = "ARN of the ML predictions SNS topic"
}
