//! Shared HTTP helpers for scout_intel sources.

use serde_json::Value;

pub async fn post_json(url: &str, body: Value, headers: &[(&str, &str)]) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&body);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("HTTP {} — {}", status, snippet));
    }
    res.json().await.map_err(|e| e.to_string())
}

/// abuse.ch MalwareBazaar / some legacy endpoints expect `application/x-www-form-urlencoded`.
pub async fn post_form(
    url: &str,
    form: &[(&str, &str)],
    headers: &[(&str, &str)],
) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.post(url).form(
        &form
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
    );
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("HTTP {} — {}", status, snippet));
    }
    res.json().await.map_err(|e| e.to_string())
}

pub async fn get_text(url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .user_agent("AEGIS-Scout/2.0 (+https://aegis-security.ru)")
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("HTTP {} — {}", status, snippet));
    }
    res.text().await.map_err(|e| e.to_string())
}

pub async fn get_json(url: &str, headers: &[(&str, &str)]) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("HTTP {} — {}", status, snippet));
    }
    res.json().await.map_err(|e| e.to_string())
}

pub fn clip_chars(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}
