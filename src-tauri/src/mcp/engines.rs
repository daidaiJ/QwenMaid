use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

static UA_INDEX: AtomicUsize = AtomicUsize::new(0);

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
];

fn random_ua() -> &'static str {
    let idx = UA_INDEX.fetch_add(1, Ordering::Relaxed) % USER_AGENTS.len();
    USER_AGENTS[idx]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcademicResult {
    pub title: String,
    pub url: String,
    pub authors: String,
    pub published: String,
    pub abstract_text: String,
    pub source: String,
}

// ── Bing Search ──────────────────────────────────────────

pub async fn search_bing(client: &reqwest::Client, query: &str) -> Result<Vec<SearchResult>, String> {
    let url = format!(
        "https://www.bing.com/search?q={}&count=10",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .header("User-Agent", random_ua())
        .header("Accept-Language", "en-US,en;q=0.9,zh-CN;q=0.8")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Bing request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Bing returned status {}", resp.status()));
    }

    let html = resp.text().await.map_err(|e| e.to_string())?;
    parse_bing_html(&html)
}

fn parse_bing_html(html: &str) -> Result<Vec<SearchResult>, String> {
    let re_block = regex::Regex::new(r#"(?s)<li class="b_algo"[^>]*>.*?</li>"#).unwrap();
    let re_link =
        regex::Regex::new(r#"(?s)<h2>\s*<a[^>]+href="(https?://[^"]+)"[^>]*>(.*?)</a>"#).unwrap();
    let re_snippet =
        regex::Regex::new(r#"(?s)<p class="b_lineclamp[^"]*"[^>]*>(.*?)</p>"#).unwrap();

    let mut results = Vec::new();
    for block in re_block.find_iter(html) {
        let block_str = block.as_str();
        if let Some(caps) = re_link.captures(block_str) {
            let url = caps[1].to_string();
            let title = strip_html_tags(&caps[2]);
            let snippet = re_snippet
                .captures(block_str)
                .map(|c| strip_html_tags(&c[1]))
                .unwrap_or_default();
            if !title.trim().is_empty() {
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
            }
        }
    }
    Ok(results)
}

// ── Baidu Search ─────────────────────────────────────────

pub async fn search_baidu(
    client: &reqwest::Client,
    query: &str,
    api_key: Option<&str>,
) -> Result<Vec<SearchResult>, String> {
    // 有 API key 时使用百度搜索 API，否则走抓取
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        return search_baidu_api(client, query, key).await;
    }

    let url = format!(
        "https://www.baidu.com/s?tn=json&wd={}&rn=10",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .header("User-Agent", random_ua())
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Baidu request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Baidu parse error: {}", e))?;

    let entries = body
        .get("feed")
        .and_then(|f| f.get("entry"))
        .and_then(|e| e.as_array())
        .ok_or_else(|| "Invalid Baidu response format".to_string())?;

    let results: Vec<SearchResult> = entries
        .iter()
        .filter_map(|entry| {
            let title = strip_html_tags(entry.get("title")?.as_str()?);
            let url = entry.get("url")?.as_str()?.to_string();
            let snippet = entry
                .get("abs")
                .and_then(|a| a.as_str())
                .map(strip_html_tags)
                .unwrap_or_default();
            Some(SearchResult {
                title,
                url,
                snippet,
            })
        })
        .collect();

    Ok(results)
}

/// 百度搜索 API（千帆 / 自定义搜索）
async fn search_baidu_api(
    client: &reqwest::Client,
    query: &str,
    api_key: &str,
) -> Result<Vec<SearchResult>, String> {
    let resp = client
        .get("https://qianfan.baidubce.com/v2/app/tools/web_search")
        .header("Authorization", format!("Bearer {}", api_key))
        .query(&[("query", query), ("count", "10")])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Baidu API request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Baidu API parse error: {}", e))?;

    let items = body
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "Invalid Baidu API response".to_string())?;

    let results: Vec<SearchResult> = items
        .iter()
        .filter_map(|r| {
            let title = r.get("title")?.as_str()?.to_string();
            let url = r.get("url")?.as_str()?.to_string();
            let snippet = r
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            Some(SearchResult { title, url, snippet })
        })
        .collect();

    Ok(results)
}

// ── Tavily Search ────────────────────────────────────────

pub async fn search_tavily(
    client: &reqwest::Client,
    query: &str,
    api_key: &str,
) -> Result<Vec<SearchResult>, String> {
    let resp = client
        .post("https://api.tavily.com/search")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "query": query,
            "search_depth": "basic",
            "max_results": 10,
        }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Tavily request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Tavily parse error: {}", e))?;

    let items = body
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "Invalid Tavily response".to_string())?;

    let results: Vec<SearchResult> = items
        .iter()
        .filter_map(|r| {
            let title = r.get("title")?.as_str()?.to_string();
            let url = r.get("url")?.as_str()?.to_string();
            let snippet = r
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            Some(SearchResult {
                title,
                url,
                snippet,
            })
        })
        .collect();

    Ok(results)
}

// ── arXiv ────────────────────────────────────────────────

pub async fn search_arxiv(client: &reqwest::Client, query: &str) -> Result<Vec<AcademicResult>, String> {
    let url = format!(
        "http://export.arxiv.org/api/query?search_query=all:{}&max_results=5",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("arXiv request failed: {}", e))?;

    let xml = resp.text().await.map_err(|e| e.to_string())?;
    parse_arxiv_xml(&xml)
}

fn parse_arxiv_xml(xml: &str) -> Result<Vec<AcademicResult>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut results = Vec::new();
    let mut in_entry = false;
    let mut current_title = String::new();
    let mut current_id = String::new();
    let mut current_summary = String::new();
    let mut current_published = String::new();
    let mut current_authors: Vec<String> = Vec::new();
    let mut current_tag = String::new();
    let mut in_author = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "entry" => {
                        in_entry = true;
                        current_title.clear();
                        current_id.clear();
                        current_summary.clear();
                        current_published.clear();
                        current_authors.clear();
                    }
                    "author" if in_entry => {
                        in_author = true;
                    }
                    _ if in_entry => {
                        current_tag = tag;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_entry => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_author && current_tag == "name" {
                    current_authors.push(text);
                } else {
                    match current_tag.as_str() {
                        "title" => current_title = text,
                        "id" => current_id = text,
                        "summary" => current_summary = text,
                        "published" => current_published = text,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "entry" => {
                        in_entry = false;
                        results.push(AcademicResult {
                            title: current_title.trim().to_string(),
                            url: current_id.trim().to_string(),
                            authors: current_authors.join(", "),
                            published: current_published.trim().to_string(),
                            abstract_text: current_summary.trim().to_string(),
                            source: "arXiv".to_string(),
                        });
                    }
                    "author" => in_author = false,
                    _ => {}
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("arXiv XML error: {}", e)),
            _ => {}
        }
    }

    Ok(results)
}

// ── Crossref ─────────────────────────────────────────────

pub async fn search_crossref(client: &reqwest::Client, query: &str) -> Result<Vec<AcademicResult>, String> {
    let url = format!(
        "https://api.crossref.org/works?query={}&rows=5",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .header("User-Agent", "AgentBox/0.1 (mailto:agent@example.com)")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Crossref request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Crossref parse error: {}", e))?;

    let items = body
        .get("message")
        .and_then(|m| m.get("items"))
        .and_then(|i| i.as_array())
        .ok_or_else(|| "Invalid Crossref response".to_string())?;

    let results: Vec<AcademicResult> = items
        .iter()
        .filter_map(|item| {
            let title = item
                .get("title")?
                .as_array()?
                .first()?
                .as_str()?
                .to_string();
            let url = item.get("URL")?.as_str()?.to_string();
            let authors = item
                .get("author")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| {
                            let family = a.get("family")?.as_str()?;
                            let given = a.get("given")?.as_str()?;
                            Some(format!("{} {}", given, family))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let published = extract_crossref_date(item);
            Some(AcademicResult {
                title,
                url,
                authors,
                published,
                abstract_text: String::new(),
                source: "Crossref".to_string(),
            })
        })
        .collect();

    Ok(results)
}

fn extract_crossref_date(item: &serde_json::Value) -> String {
    item.get("published-print")
        .or_else(|| item.get("published-online"))
        .and_then(|p| p.get("date-parts"))
        .and_then(|d| d.as_array()?.first()?.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.as_i64().map(|n| n.to_string()))
                .collect::<Vec<_>>()
                .join("-")
        })
        .unwrap_or_default()
}

// ── OpenAlex ─────────────────────────────────────────────

pub async fn search_openalex(client: &reqwest::Client, query: &str) -> Result<Vec<AcademicResult>, String> {
    let url = format!(
        "https://api.openalex.org/works?search={}&per_page=5",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("OpenAlex request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("OpenAlex parse error: {}", e))?;

    let items = body
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "Invalid OpenAlex response".to_string())?;

    let results: Vec<AcademicResult> = items
        .iter()
        .filter_map(|r| {
            let title = r.get("title")?.as_str()?.to_string();
            let url = r
                .get("doi")
                .and_then(|d| d.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| r.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()))
                .unwrap_or_default();
            let authors = r
                .get("authorships")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("author")?.get("display_name")?.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let published = r
                .get("publication_date")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            let abstract_text = r
                .get("abstract_inverted_index")
                .and_then(reconstruct_abstract)
                .unwrap_or_default();
            Some(AcademicResult {
                title,
                url,
                authors,
                published,
                abstract_text,
                source: "OpenAlex".to_string(),
            })
        })
        .collect();

    Ok(results)
}

fn reconstruct_abstract(idx: &serde_json::Value) -> Option<String> {
    let map = idx.as_object()?;
    let mut positions: Vec<(usize, &str)> = Vec::new();
    for (word, pos_array) in map {
        if let Some(arr) = pos_array.as_array() {
            for pos in arr {
                if let Some(p) = pos.as_u64() {
                    positions.push((p as usize, word.as_str()));
                }
            }
        }
    }
    positions.sort_by_key(|(p, _)| *p);
    Some(
        positions
            .iter()
            .map(|(_, w)| *w)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

// ── Jina Reader ──────────────────────────────────────────

pub async fn fetch_jina(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
) -> Result<String, String> {
    let jina_url = format!("https://r.jina.ai/{}", url);
    let resp = client
        .get(&jina_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "application/json")
        .header("X-Return-Format", "markdown")
        .header("X-Retain-Images", "none")
        .header("X-Timeout", "15")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("Jina request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Jina parse error: {}", e))?;

    body.get("data")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid Jina response".to_string())
}

// ── Direct Web Fetch ─────────────────────────────────────

pub async fn fetch_direct(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;
    if let Some(host) = parsed.host_str() {
        if is_private_ip(host) {
            return Err("Private IP addresses are not allowed".to_string());
        }
    }

    let resp = client
        .get(url)
        .header("User-Agent", random_ua())
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body = resp.text().await.map_err(|e| e.to_string())?;

    if content_type.contains("text/html") {
        Ok(html_to_text(&body))
    } else {
        Ok(body)
    }
}

// ── Helpers ──────────────────────────────────────────────

fn strip_html_tags(html: &str) -> String {
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re.replace_all(html, "");
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

fn html_to_text(html: &str) -> String {
    let re_script =
        regex::Regex::new(r"(?si)<(script|style|nav|header|footer)[^>]*>.*?</\1>").unwrap();
    let text = re_script.replace_all(html, "");
    let re_block = regex::Regex::new(r"(?si)<(br|/p|/div|/h[1-6]|/li|/tr)[^>]*>").unwrap();
    let text = re_block.replace_all(&text, "\n");
    let re_tag = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tag.replace_all(&text, "");
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'");
    let re_ws = regex::Regex::new(r"\n{3,}").unwrap();
    let text = re_ws.replace_all(&text, "\n\n");
    text.trim().to_string()
}

fn is_private_ip(host: &str) -> bool {
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(ip) => {
                ip.is_private()
                    || ip.is_loopback()
                    || ip.is_link_local()
                    || ip.is_broadcast()
                    || ip.is_unspecified()
            }
            std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unspecified(),
        }
    } else {
        host == "localhost"
            || host.ends_with(".local")
            || host.ends_with(".internal")
    }
}
