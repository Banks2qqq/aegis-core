fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = reqwest::Client::new();
        let resp = client.get("http://localhost:8080/federation/merkle").send().await.unwrap();
        println!("Status: {}", resp.status());
        println!("Body: {:?}", resp.text().await.unwrap());
    });
}
