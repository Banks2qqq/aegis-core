use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() {
    let client = Client::new();
    let key = "sk-6c532b10b9494cc681949eaf5368a822";
    let body = json!({"model": "google/gemini-2.0-flash-001", "messages": [{"role": "user", "content": "hi"}]});
    let res = client.post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", key))
        .json(&body)
        .send().await.unwrap();
    println!("Status: {}", res.status());
    println!("Body: {}", res.text().await.unwrap());
}
