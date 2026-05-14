use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() {
    let client = Client::new();
    let key = "sk-6c532b10b9494cc681949eaf5368a822";
    let body = json!({"model": "deepseek-chat", "messages": [{"role": "user", "content": "hi"}]});
    let res = client.post("https://api.deepseek.com/chat/completions")
        .header("Authorization", format!("Bearer {}", key))
        .json(&body)
        .send().await.unwrap();
    println!("DeepSeek Status: {}", res.status());
    println!("DeepSeek Body: {}", res.text().await.unwrap());
}
