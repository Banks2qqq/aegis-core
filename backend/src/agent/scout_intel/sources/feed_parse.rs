//! Lightweight parsers for blocklists and RSS (no extra XML deps).

use regex::Regex;
use std::sync::LazyLock;

static RE_IPV4: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d?\d)\b")
        .unwrap()
});

static RE_CVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bCVE-\d{4}-\d{4,}\b").unwrap());

/// Parse Talos-style blocklist (.blf or plain): `ip` or `ip,score` per line.
pub fn parse_ip_blocklist(text: &str, max_ips: usize) -> Vec<String> {
    let mut ips = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let ip_part = line.split(',').next().unwrap_or(line).trim();
        if RE_IPV4.is_match(ip_part) && !is_private_ip(ip_part) {
            if !ips.iter().any(|x| x == ip_part) {
                ips.push(ip_part.to_string());
                if ips.len() >= max_ips {
                    break;
                }
            }
        }
    }
    ips
}

fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("127.")
        || ip.starts_with("0.")
        || ip.starts_with("255.")
}

#[derive(Debug, Clone)]
pub struct RssItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: String,
}

/// Minimal RSS 2.0 item extraction (FortiGuard feeds).
pub fn parse_rss_items(xml: &str, max_items: usize) -> Vec<RssItem> {
    let mut items = Vec::new();
    let re_item = Regex::new(r"(?is)<item\b[^>]*>(.*?)</item>").unwrap();
    let re_title = Regex::new(r"(?is)<title[^>]*>(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?</title>").unwrap();
    let re_link = Regex::new(r"(?is)<link[^>]*>(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?</link>").unwrap();
    let re_desc = Regex::new(r"(?is)<description[^>]*>(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?</description>")
        .unwrap();
    let re_pub = Regex::new(r"(?is)<pubDate[^>]*>(.*?)</pubDate>").unwrap();

    for cap in re_item.captures_iter(xml) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let title = first_capture(&re_title, block).unwrap_or_else(|| "Untitled".into());
        let link = first_capture(&re_link, block).unwrap_or_default();
        let description = strip_html(&first_capture(&re_desc, block).unwrap_or_default());
        let pub_date = first_capture(&re_pub, block).unwrap_or_default();
        items.push(RssItem {
            title: decode_xml_entities(&title),
            link: decode_xml_entities(&link),
            description: decode_xml_entities(&description),
            pub_date,
        });
        if items.len() >= max_items {
            break;
        }
    }
    items
}

pub fn extract_cves(blob: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in RE_CVE.find_iter(blob) {
        let c = m.as_str().to_uppercase();
        if !out.iter().any(|x| x == &c) {
            out.push(c);
        }
    }
    out
}

fn first_capture(re: &Regex, hay: &str) -> Option<String> {
    re.captures(hay)
        .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|s| !s.is_empty())
}

fn strip_html(s: &str) -> String {
    let re_tag = Regex::new(r"(?is)<[^>]+>").unwrap();
    let t = re_tag.replace_all(s, " ");
    decode_xml_entities(t.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocklist() {
        let text = "# comment\n1.2.3.4,-1.0\n5.6.7.8\n";
        let ips = parse_ip_blocklist(text, 10);
        assert_eq!(ips, vec!["1.2.3.4", "5.6.7.8"]);
    }

    #[test]
    fn parses_rss_item() {
        let xml = r#"<rss><channel><item><title>Test Alert</title><link>https://example.com/a</link><description>CVE-2024-1234 RCE</description></item></channel></rss>"#;
        let items = parse_rss_items(xml, 5);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Test Alert");
        assert!(extract_cves(&items[0].description).contains(&"CVE-2024-1234".to_string()));
    }
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;nbsp;", " ")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#13;", "")
        .replace("&#40;", "(")
        .replace("&#41;", ")")
        .replace("&apos;", "'")
}
