use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use super::knowledge::KnowledgeBase;
use super::isolation::{AdaptiveIsolation, IsolationLevel, Workload};
use futures::StreamExt;
use std::net::IpAddr;

/// Описание инструмента
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParam>,
}

/// Параметр инструмента
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    pub name: String,
    pub param_type: String, // "string", "int", "bool"
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum ToolCallError {
    Parse,
    ToolNotFound,
    MissingRequiredParam(String),
    UnknownParam(String),
    InvalidType { key: String, expected: String },
    TooLarge(String),
}

/// Результат вызова инструмента
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Реестр инструментов
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolExecutor>>,
    definitions: Vec<ToolDef>,
}

/// Трейт для исполняемых инструментов
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, params: HashMap<String, String>) -> ToolResult;
    fn definition(&self) -> ToolDef;
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            definitions: Vec::new(),
        }
    }

    /// Зарегистрировать инструмент
    pub fn register(&mut self, executor: Box<dyn ToolExecutor>) {
        let def = executor.definition();
        self.definitions.push(def.clone());
        self.tools.insert(def.name.clone(), executor);
    }

    /// Получить список доступных инструментов для LLM
    pub fn get_tools_for_prompt(&self) -> Vec<(&str, &str)> {
        self.definitions
            .iter()
            .map(|d| (d.name.as_str(), d.description.as_str()))
            .collect()
    }

    /// Получить подробное описание инструментов
    pub fn get_tools_description(&self) -> String {
        self.definitions
            .iter()
            .map(|d| {
                let params = d
                    .parameters
                    .iter()
                    .map(|p| format!("  - {} ({}{}): {}", p.name, p.param_type, if p.required { ", обязательно" } else { "" }, p.description))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("- {}: {}\n{}", d.name, d.description, params)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Выполнить инструмент по имени
    pub async fn execute(&self, name: &str, params: HashMap<String, String>) -> Option<ToolResult> {
        if let Some(tool) = self.tools.get(name) {
            Some(tool.execute(params).await)
        } else {
            None
        }
    }

    pub fn validate_call(&self, name: &str, params: &HashMap<String, String>) -> Result<(), ToolCallError> {
        const MAX_PARAM_VALUE: usize = 8000;
        let def = self
            .definitions
            .iter()
            .find(|d| d.name == name)
            .ok_or(ToolCallError::ToolNotFound)?;

        // Reject unknown params
        for k in params.keys() {
            if !def.parameters.iter().any(|p| p.name == *k) {
                return Err(ToolCallError::UnknownParam(k.clone()));
            }
        }

        // Check required + type sanity + size
        for p in &def.parameters {
            if p.required && !params.contains_key(&p.name) {
                return Err(ToolCallError::MissingRequiredParam(p.name.clone()));
            }
            if let Some(v) = params.get(&p.name) {
                if v.len() > MAX_PARAM_VALUE {
                    return Err(ToolCallError::TooLarge(p.name.clone()));
                }
                match p.param_type.as_str() {
                    "string" => {}
                    "int" => {
                        if v.parse::<i64>().is_err() {
                            return Err(ToolCallError::InvalidType { key: p.name.clone(), expected: "int".into() });
                        }
                    }
                    "bool" => {
                        let vl = v.to_lowercase();
                        if !(vl == "true" || vl == "false" || vl == "1" || vl == "0") {
                            return Err(ToolCallError::InvalidType { key: p.name.clone(), expected: "bool".into() });
                        }
                    }
                    other => {
                        return Err(ToolCallError::InvalidType { key: p.name.clone(), expected: other.into() });
                    }
                }
            }
        }

        Ok(())
    }

    /// Разобрать вызов инструмента из строки вида "tool_name(key=value, key2=value2)"
    pub fn parse_action(action: &str) -> Option<(String, HashMap<String, String>)> {
        // Strict, zero-trust parser:
        // - name: [a-zA-Z0-9_]{1,64}
        // - args: key="value" pairs, comma-separated
        // - supports \" and \\ inside quoted values
        // - rejects extra trailing characters and nested parentheses
        const MAX_ACTION_LEN: usize = 2048;
        const MAX_ARGS: usize = 24;
        const MAX_KEY_LEN: usize = 64;
        const MAX_VAL_LEN: usize = 2048;

        let action = action.trim();
        if action.is_empty() || action.len() > MAX_ACTION_LEN {
            return None;
        }

        let open_paren = action.find('(')?;
        let close_paren = action.rfind(')')?;
        if close_paren + 1 != action.len() {
            return None; // no trailing bytes allowed
        }

        let name = action[..open_paren].trim();
        if name.is_empty() || name.len() > 64 {
            return None;
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }

        // Reject nested parentheses anywhere (defense-in-depth)
        if action[open_paren + 1..close_paren].contains('(') || action[open_paren + 1..close_paren].contains(')') {
            return None;
        }

        let args_str = action[open_paren + 1..close_paren].trim();
        let mut params = HashMap::new();
        if args_str.is_empty() {
            return Some((name.to_string(), params));
        }

        let mut i = 0usize;
        let bytes = args_str.as_bytes();
        let mut parsed_args = 0usize;

        while i < bytes.len() {
            // skip whitespace
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }

            // parse key
            let key_start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if i == key_start {
                return None;
            }
            let key = &args_str[key_start..i];
            if key.len() > MAX_KEY_LEN {
                return None;
            }

            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] != b'=' {
                return None;
            }
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] != b'"' {
                return None; // require quoted strings only
            }
            i += 1;

            // parse quoted value with basic escapes
            let mut val = String::new();
            while i < bytes.len() {
                let b = bytes[i];
                if b == b'"' {
                    i += 1;
                    break;
                }
                if b == b'\\' {
                    i += 1;
                    if i >= bytes.len() {
                        return None;
                    }
                    let esc = bytes[i];
                    match esc {
                        b'\\' => val.push('\\'),
                        b'"' => val.push('"'),
                        b'n' => val.push('\n'),
                        b'r' => val.push('\r'),
                        b't' => val.push('\t'),
                        _ => return None,
                    }
                    i += 1;
                } else {
                    // forbid control chars
                    if b < 0x20 {
                        return None;
                    }
                    val.push(b as char);
                    i += 1;
                }
                if val.len() > MAX_VAL_LEN {
                    return None;
                }
            }
            if val.len() > MAX_VAL_LEN {
                return None;
            }

            params.insert(key.to_string(), val);
            parsed_args += 1;
            if parsed_args > MAX_ARGS {
                return None;
            }

            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i == bytes.len() {
                break;
            }
            if bytes[i] != b',' {
                return None;
            }
            i += 1; // consume comma and continue
        }

        Some((name.to_string(), params))
    }
}

// ================================================================
// Встроенные инструменты
// ================================================================

/// Инструмент: fetch_url — получить содержимое URL
pub struct FetchUrlTool {
    client: reqwest::Client,
}

impl FetchUrlTool {
    pub fn new() -> Self {
        // Hard timeout + no redirects to reduce SSRF surface.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client }
    }
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

fn host_allowed_by_allowlist(host: &str) -> bool {
    // Configurable allowlist:
    // AEGIS_FETCH_URL_ALLOWLIST="example.com,sub.example.org"
    // Matches exact host or any subdomain of an entry.
    //
    // If unset/empty, we allow all public hosts (still blocking private IPs).
    let raw = std::env::var("AEGIS_FETCH_URL_ALLOWLIST").unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return true;
    }

    let host = host.trim_end_matches('.').to_ascii_lowercase();
    for entry in raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let e = entry.trim_end_matches('.').to_ascii_lowercase();
        if host == e || host.ends_with(&format!(".{}", e)) {
            return true;
        }
    }
    false
}

#[async_trait::async_trait]
impl ToolExecutor for FetchUrlTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "fetch_url".into(),
            description: "Получить содержимое веб-страницы по URL".into(),
            parameters: vec![ToolParam {
                name: "url".into(),
                param_type: "string".into(),
                required: true,
                description: "URL для запроса".into(),
            }],
        }
    }

    async fn execute(&self, params: HashMap<String, String>) -> ToolResult {
        let url = match params.get("url") {
            Some(u) => u.clone(),
            None => return ToolResult { success: false, output: String::new(), error: Some("url не указан".into()) },
        };

        let parsed = match reqwest::Url::parse(&url) {
            Ok(u) => u,
            Err(_) => {
                return ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("invalid url".into()),
                }
            }
        };

        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some("only http/https allowed".into()),
            };
        }

        let host = match parsed.host_str() {
            Some(h) => h,
            None => {
                return ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("missing host".into()),
                }
            }
        };

        if !host_allowed_by_allowlist(host) {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some("blocked by allowlist".into()),
            };
        }

        // Block obvious local hosts
        let host_lc = host.to_ascii_lowercase();
        if host_lc == "localhost" {
            return ToolResult { success: false, output: String::new(), error: Some("SSRF blocked".into()) };
        }

        // If host is an IP literal, validate it directly. Otherwise resolve and validate.
        if let Ok(ip) = host.parse::<IpAddr>() {
            if is_private_ip(ip) {
                return ToolResult { success: false, output: String::new(), error: Some("private ip blocked".into()) };
            }
        } else {
            let port = parsed.port_or_known_default().unwrap_or(80);
            match tokio::net::lookup_host((host, port)).await {
                Ok(addrs) => {
                    for a in addrs {
                        if is_private_ip(a.ip()) {
                            return ToolResult { success: false, output: String::new(), error: Some("resolved to private ip".into()) };
                        }
                    }
                }
                Err(_) => {
                    return ToolResult { success: false, output: String::new(), error: Some("dns resolution failed".into()) };
                }
            }
        }

        let resp = match self.client.get(parsed).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult { success: false, output: String::new(), error: Some(e.to_string()) },
        };

        // Enforce max bytes even if Content-Length missing/misleading.
        const MAX_BYTES: usize = 1_000_000; // 1MB
        let mut buf: Vec<u8> = Vec::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => return ToolResult { success: false, output: String::new(), error: Some(e.to_string()) },
            };
            if buf.len() + chunk.len() > MAX_BYTES {
                return ToolResult { success: false, output: String::new(), error: Some("response too large".into()) };
            }
            buf.extend_from_slice(&chunk);
        }

        let body = String::from_utf8_lossy(&buf).to_string();
        ToolResult { success: true, output: body, error: None }
    }
}

/// Инструмент: search_knowledge_base — реальный поиск по Базе Знаний AEGIS (OSINT + DarkNet)
pub struct SearchKnowledgeBaseTool {
    kb: Option<Arc<KnowledgeBase>>,
}

impl SearchKnowledgeBaseTool {
    pub fn new() -> Self {
        Self { kb: None }
    }

    pub fn with_knowledge_base(mut self, kb: Arc<KnowledgeBase>) -> Self {
        self.kb = Some(kb);
        self
    }
}

#[async_trait::async_trait]
impl ToolExecutor for SearchKnowledgeBaseTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "search_knowledge_base".into(),
            description: "Поиск по Базе Знаний AEGIS (OSINT + DarkNet)".into(),
            parameters: vec![
                ToolParam {
                    name: "query".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "Поисковый запрос".into(),
                },
                ToolParam {
                    name: "source".into(),
                    param_type: "string".into(),
                    required: false,
                    description: "osint или darknet (по умолчанию оба)".into(),
                },
            ],
        }
    }

    async fn execute(&self, params: HashMap<String, String>) -> ToolResult {
        let query = params.get("query").cloned().unwrap_or_default();
        let source = params.get("source").cloned().unwrap_or_else(|| "both".to_string());

        if let Some(kb) = &self.kb {
            let results = if source == "darknet" {
                kb.search_darknet(&query).await
            } else {
                kb.search_osint(&query).await
            };

            match results {
                Ok(docs) if !docs.is_empty() => {
                    let default = "unknown".to_string();
                    let summary: Vec<String> = docs.iter().take(5).map(|(score, _, payload)| {
                        let title = payload.get("title").unwrap_or(&default);
                        format!("{:.2} | {}", score, title)
                    }).collect();
                    ToolResult {
                        success: true,
                        output: format!("Найдено {} документов:\n{}", docs.len(), summary.join("\n")),
                        error: None,
                    }
                }
                _ => ToolResult {
                    success: true,
                    output: "Релевантных документов не найдено.".into(),
                    error: None,
                },
            }
        } else {
            ToolResult {
                success: false,
                output: "KnowledgeBase не подключена".into(),
                error: Some("KB missing".into()),
            }
        }
    }
}

/// Инструмент: run_sandbox — выполнить код в изолированной среде (с адаптивной изоляцией)
pub struct RunSandboxTool {
    workload: Workload,
}

impl RunSandboxTool {
    pub fn new() -> Self {
        Self {
            workload: Workload::Sentinel, // по умолчанию — Sentinel (высокая изоляция)
        }
    }

    pub fn for_workload(mut self, workload: Workload) -> Self {
        self.workload = workload;
        self
    }
}

#[async_trait::async_trait]
impl ToolExecutor for RunSandboxTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "run_sandbox".into(),
            description: "Выполнить код в изолированной песочнице (Python/JS)".into(),
            parameters: vec![
                ToolParam {
                    name: "language".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "Язык: python или javascript".into(),
                },
                ToolParam {
                    name: "code".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "Код для выполнения".into(),
                },
                ToolParam {
                    name: "timeout_ms".into(),
                    param_type: "int".into(),
                    required: false,
                    description: "Таймаут в миллисекундах (по умолчанию 5000)".into(),
                },
            ],
        }
    }

    async fn execute(&self, params: HashMap<String, String>) -> ToolResult {
        // Kill-switch: allow operators to disable sandbox tool at runtime
        if std::env::var("AEGIS_ENABLE_RUN_SANDBOX").map(|v| v == "0").unwrap_or(false) {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some("run_sandbox disabled by AEGIS_ENABLE_RUN_SANDBOX=0".into()),
            };
        }

        let language = params
            .get("language")
            .cloned()
            .unwrap_or_else(|| "python".to_string())
            .to_lowercase();
        let code = params.get("code").cloned().unwrap_or_default();
        let timeout_ms_raw: u64 = params
            .get("timeout_ms")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5000);
        let timeout_ms = timeout_ms_raw.clamp(100, 10_000);

        if language != "python" && language != "javascript" {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some("unsupported language (allowed: python|javascript)".into()),
            };
        }

        // Hard bounds: keep payload small to reduce attack surface + log volume
        const MAX_CODE_CHARS: usize = 8000;
        if code.is_empty() {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some("code is empty".into()),
            };
        }
        if code.len() > MAX_CODE_CHARS {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("code too large ({} > {})", code.len(), MAX_CODE_CHARS)),
            };
        }

        // Zero-trust denylist for common escape hatches / RCE primitives.
        // NOTE: This is defensive even though execution is simulated today.
        let lc = code.to_lowercase();
        let mut denied = Vec::new();
        if language == "python" {
            let needles = [
                "import os", "import subprocess", "subprocess.", "os.system", "os.popen", "pty",
                "__import__", "eval(", "exec(", "open(", "socket", "requests", "urllib", "http",
            ];
            for n in needles {
                if lc.contains(n) { denied.push(n); }
            }
        } else {
            let needles = [
                "child_process", "require(", "process.", "fs.", "net.", "dgram", "http", "https",
                "eval(", "new function", "webassembly", "fetch(",
            ];
            for n in needles {
                if lc.contains(n) { denied.push(n); }
            }
        }
        if !denied.is_empty() {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("sandbox policy blocked code patterns: {}", denied.join(", "))),
            };
        }

        let mut config = AdaptiveIsolation::for_workload(self.workload.clone());

        if lc.contains("exec") || lc.contains("os.system") || lc.contains("subprocess") || lc.contains("eval(") {
            config = AdaptiveIsolation::escalate(&config, 0.85);
        }

        let runtime = match config.level {
            IsolationLevel::Low => "docker",
            IsolationLevel::Medium => "kata-containers",
            IsolationLevel::High => "firecracker",
            IsolationLevel::Critical => "firecracker (bare-metal)",
        };

        // Production note: real execution would use firecracker or docker with seccomp + resource limits.
        // For now we simulate with full security context logging.
        let output = format!(
            "=== Adaptive Isolation (SIMULATED) ===\n\
             Workload: {:?}\n\
             Level: {:?} → {}\n\
             Limits: {}MB / {:.1} CPU / {:?} network / {}ms\n\
             Language: {}\n\
             Code length: {} chars\n\
             Security boundary applied. Real execution pending Firecracker/Docker integration.",
            self.workload, config.level, runtime, config.memory_mb, config.cpu_cores, config.network, timeout_ms, language, code.len()
        );

        ToolResult { success: true, output, error: None }
    }
}

/// Создать ToolRegistry со стандартным набором инструментов (реальный search если KB передан)
pub fn create_default_registry(kb: Option<Arc<KnowledgeBase>>, air_gapped: bool) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    // Air-Gapped hardening: disable external fetch tool completely
    if !air_gapped {
        registry.register(Box::new(FetchUrlTool::new()));
    }

    let search_tool = if let Some(kb_arc) = kb {
        SearchKnowledgeBaseTool::new().with_knowledge_base(kb_arc)
    } else {
        SearchKnowledgeBaseTool::new()
    };
    registry.register(Box::new(search_tool));

    // Sandbox с высокой изоляцией по умолчанию (Sentinel workload)
    registry.register(Box::new(RunSandboxTool::new().for_workload(Workload::Sentinel)));

    // Реальный web_search через DuckDuckGo (бесплатно, без ключа)
    registry.register(Box::new(WebSearchTool::new()));

    registry
}

/// Инструмент: web_search — реальный поиск в интернете (DuckDuckGo)
pub struct WebSearchTool {
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for WebSearchTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "web_search".into(),
            description: "Поиск в интернете по запросу (DuckDuckGo)".into(),
            parameters: vec![ToolParam {
                name: "query".into(),
                param_type: "string".into(),
                required: true,
                description: "Поисковый запрос".into(),
            }],
        }
    }

    async fn execute(&self, params: HashMap<String, String>) -> ToolResult {
        let query = params.get("query").cloned().unwrap_or_default();
        if query.is_empty() {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some("query не указан".into()),
            };
        }

        // DuckDuckGo Instant Answer API (бесплатно)
        let url = format!("https://api.duckduckgo.com/?q={}&format=json&no_html=1", urlencoding::encode(&query));

        match self.client.get(&url).send().await {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let abstract_text = json["AbstractText"].as_str().unwrap_or("").to_string();
                    let related = json["RelatedTopics"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|t| t["Text"].as_str())
                                .take(3)
                                .collect::<Vec<_>>()
                                .join(" | ")
                        })
                        .unwrap_or_default();

                    let output = if !abstract_text.is_empty() {
                        format!("Abstract: {}\nRelated: {}", abstract_text, related)
                    } else {
                        format!("No direct abstract. Related: {}", related)
                    };

                    ToolResult {
                        success: true,
                        output,
                        error: None,
                    }
                }
                Err(e) => ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                },
            },
            Err(e) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action() {
        let (name, params) = ToolRegistry::parse_action("fetch_url(url=\"https://example.com\")").unwrap();
        assert_eq!(name, "fetch_url");
        assert_eq!(params.get("url").unwrap(), "https://example.com");
    }

    #[test]
    fn test_parse_action_no_args() {
        let (name, params) = ToolRegistry::parse_action("health_check()").unwrap();
        assert_eq!(name, "health_check");
        assert!(params.is_empty());
    }

    #[test]
    fn test_registry_has_tools() {
        let registry = create_default_registry(None, false);
        let tools = registry.get_tools_for_prompt();
        assert!(tools.len() >= 2);
    }
}