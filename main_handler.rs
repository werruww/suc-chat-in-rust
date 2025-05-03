use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_files as fs;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize)]
struct ChatRequest {
    message: String,
}

#[derive(Serialize)]
struct ChatResponse {
    message: String,
    error: Option<String>,
}

async fn chat_handler(data: web::Json<ChatRequest>) -> impl Responder {
    let client = Client::new();
    
    // Prepare the request to Ollama API
    let ollama_payload = json!({
        "model": "llama3.2:1b",
        "prompt": data.message,
        "stream": false
    });

    // Send request to Ollama API with a timeout
    let response = client
        .post("http://localhost:11434/api/generate")
        .timeout(std::time::Duration::from_secs(100)) // 10-second timeout
        .json(&ollama_payload)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(ollama_response) => {
                    let ai_message = ollama_response["response"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "No valid response from AI.".to_string());

                    HttpResponse::Ok().json(ChatResponse {
                        message: ai_message,
                        error: None,
                    })
                }
                Err(e) => HttpResponse::InternalServerError().json(ChatResponse {
                    message: "".to_string(),
                    error: Some(format!("Failed to parse AI response: {}", e)),
                }),
            }
        }
        Ok(resp) => HttpResponse::BadGateway().json(ChatResponse {
            message: "".to_string(),
            error: Some(format!("Ollama API error: Status {}", resp.status())),
        }),
        Err(e) if e.is_timeout() => HttpResponse::GatewayTimeout().json(ChatResponse {
            message: "".to_string(),
            error: Some("Request to AI model timed out. Please try again.".to_string()),
        }),
        Err(e) if e.is_connect() => HttpResponse::ServiceUnavailable().json(ChatResponse {
            message: "".to_string(),
            error: Some("Failed to connect to AI model. Is Ollama running?".to_string()),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ChatResponse {
            message: "".to_string(),
            error: Some(format!("Error communicating with AI model: {}", e)),
        }),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/api/chat", web::post().to(chat_handler))
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}