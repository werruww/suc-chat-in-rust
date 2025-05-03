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

// Helper function to format the AI response into a tidy, list-based structure
fn format_ai_response(raw_response: &str) -> String {
    // Split the response into lines or sentences
    let lines: Vec<&str> = raw_response
        .split(|c| c == '\n' || c == '.' || c == '!')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // Check if the response already resembles a list (e.g., starts with numbers)
    let mut is_list = lines.iter().any(|line| {
        line.chars()
            .next()
            .map(|c| c.is_digit(10) || c == '-' || c == '*')
            .unwrap_or(false)
    });

    // If it’s not a list, try to break it into logical points
    if !is_list && lines.len() > 1 {
        is_list = true; // Treat multiple sentences as a list
    }

    if is_list {
        // Extract points and remove any existing numbering
        let mut points: Vec<String> = lines
            .into_iter()
            .map(|line| {
                let trimmed = line.trim_start_matches(|c: char| c.is_digit(10) || c == '.' || c == '-' || c == '*' || c == ' ').to_string();
                trimmed
            })
            .filter(|s| !s.is_empty())
            .collect();

        // Sort points alphabetically (or by some logical order if needed)
        points.sort();

        // Format as a numbered list
        let mut formatted = String::new();
        for (i, point) in points.iter().enumerate() {
            formatted.push_str(&format!("{}. {}\n", i + 1, point));
        }
        formatted.trim().to_string()
    } else {
        // If it’s not a list, format as a single paragraph with proper spacing
        raw_response
            .split(|c| c == '\n' || c == '.' || c == '!')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<&str>>()
            .join(". ") + "."
    }
}

async fn chat_handler(data: web::Json<ChatRequest>) -> impl Responder {
    let client = Client::new();
    
    // Prepare the request to Ollama API
    let ollama_payload = json!({
        "model": "llama3.2:1b", // model of ollama
        "prompt": data.message,
        "stream": false
    });

    // Send request to Ollama API with a timeout
    let response = client
        .post("http://localhost:11434/api/generate")
        .timeout(std::time::Duration::from_secs(100))
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

                    // Format the AI response
                    let formatted_message = format_ai_response(&ai_message);

                    HttpResponse::Ok().json(ChatResponse {
                        message: formatted_message,
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