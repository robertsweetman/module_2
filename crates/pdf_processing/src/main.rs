use lambda_runtime::{service_fn, LambdaEvent, Error, run};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct TestRequest {
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestResponse {
    message: String,
}

async fn function_handler(event: LambdaEvent<TestRequest>) -> Result<TestResponse, Error> {
    println!("Received message: {}", event.payload.message);
    
    Ok(TestResponse {
        message: format!("Processed: {}", event.payload.message),
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Minimal lambda starting");
    run(service_fn(function_handler)).await
}