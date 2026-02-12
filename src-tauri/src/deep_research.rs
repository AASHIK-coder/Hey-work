//! Deep Research Mode — Chrome-Powered Search + LLM Synthesis
//!
//! Two-phase pipeline for high-quality research:
//!   Phase 1: Chrome opens Google, searches, visits pages, extracts FULL content
//!   Phase 2: All extracted content is sent to Claude LLM for intelligent synthesis
//!
//! This gives the best of both worlds:
//! - User sees Chrome performing real searches (magic experience)
//! - Full page content extraction (not just snippets)
//! - Claude LLM produces a polished, properly formatted report
//!   tailored to the user's specific request
//! - Falls back to Claude's built-in web_search if Chrome unavailable

use crate::api::{AnthropicClient, ContentBlock, Message};
use crate::browser::{BrowserClient, SharedBrowserClient};
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};

// ============================================================
// Data Structures
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchQuery {
    pub query: String,
    pub intent: String,
    pub priority: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    pub query: String,
    pub sources: Vec<ResearchSource>,
    pub summary: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSource {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub content: String,
    pub credibility_score: f32,
    pub published_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchReport {
    pub original_query: String,
    pub research_queries: Vec<ResearchQuery>,
    pub results: Vec<ResearchResult>,
    pub synthesized_answer: String,
    pub key_findings: Vec<String>,
    pub sources: Vec<ResearchSource>,
    pub follow_up_questions: Vec<String>,
    pub confidence_score: f32,
    pub research_depth: String,
}

// ============================================================
// Phase 1: Chrome Search & Content Extraction
// ============================================================

/// JavaScript to extract Google search results
const GOOGLE_EXTRACT_JS: &str = r#"
(function() {
    var results = [];
    var items = document.querySelectorAll('div.g');
    if (items.length === 0) items = document.querySelectorAll('[data-sokoban]');
    if (items.length === 0) {
        var h3s = document.querySelectorAll('#search h3');
        var wrapped = [];
        h3s.forEach(function(h3) { var p = h3.closest('div'); if (p) wrapped.push(p); });
        if (wrapped.length > 0) items = wrapped;
    }
    items.forEach(function(el) {
        var h3 = el.querySelector('h3');
        if (!h3) return;
        var link = h3.closest('a') || el.querySelector('a[href^="http"]');
        if (!link) return;
        var url = link.href;
        if (!url || !url.startsWith('http')) return;
        if (url.indexOf('google.com/search') !== -1) return;
        if (url.indexOf('accounts.google') !== -1) return;
        var title = (h3.innerText || '').trim();
        if (!title) return;
        var snippet = '';
        var snippetEl = el.querySelector('.VwiC3b, [data-sncf], .lEBKkf, .IsZvec');
        if (snippetEl) snippet = (snippetEl.innerText || '').trim();
        if (!snippet) {
            var divs = el.querySelectorAll('div');
            for (var i = 0; i < divs.length; i++) {
                var t = (divs[i].innerText || '').trim();
                if (t.length > 50 && t !== title) { snippet = t; break; }
            }
        }
        results.push({ title: title.substring(0, 200), url: url, snippet: snippet.substring(0, 500) });
    });
    var seen = {}; var unique = [];
    results.forEach(function(r) { if (!seen[r.url]) { seen[r.url] = true; unique.push(r); } });
    return JSON.stringify(unique.slice(0, 10));
})()
"#;

/// JavaScript to extract full readable text from any webpage — deep extraction
const PAGE_CONTENT_EXTRACT_JS: &str = r#"
(function() {
    var el = document.body.cloneNode(true);
    // Remove noise elements
    var noise = el.querySelectorAll('script, style, nav, header, footer, aside, iframe, noscript, svg, .sidebar, .nav, .footer, .header, .menu, .ad, .ads, .cookie, .popup, .modal, .banner, .social-share, .comments, .related-posts, [role="navigation"], [role="banner"], [role="complementary"], [aria-hidden="true"]');
    noise.forEach(function(n) { if(n.parentNode) n.parentNode.removeChild(n); });

    // Try to find main content area first
    var main = el.querySelector('main, article, [role="main"], .post-content, .article-body, .entry-content, .article-content, #article-body, .story-body, .post-body, .blog-post, .page-content, .content-body, .td-post-content');
    var text = (main || el).innerText || '';

    // Clean up whitespace
    text = text.replace(/\n{3,}/g, '\n\n').replace(/[ \t]{2,}/g, ' ').trim();

    // Return up to 8000 chars of content (enough for LLM to work with)
    return text.substring(0, 8000);
})()
"#;

/// Generate research queries using LLM
async fn generate_search_queries(
    original_query: &str,
    depth: &str,
    api_key: &str,
    model: &str,
) -> Vec<ResearchQuery> {
    let num_queries = match depth {
        "quick" => 3,
        "standard" => 5,
        "deep" => 8,
        _ => 5,
    };

    let client = AnthropicClient::new(api_key.to_string(), model.to_string());

    let prompt = format!(
        r#"Generate {} diverse Google search queries to thoroughly research: "{}"

For each, return a JSON array: [{{"query": "search query", "intent": "what this covers"}}]
Cover different angles: overview, statistics, expert opinions, recent news, comparisons.
Return ONLY the JSON array."#,
        num_queries, original_query
    );

    let messages = vec![Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text { text: prompt }],
    }];

    match client.complete(None, messages, None).await {
        Ok(result) => {
            let text: String = result.content.iter()
                .filter_map(|b| if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
                .collect();

            if let Some(start) = text.find('[') {
                if let Some(end) = text.rfind(']') {
                    if let Ok(queries) = serde_json::from_str::<Vec<serde_json::Value>>(&text[start..=end]) {
                        return queries.iter().enumerate().map(|(idx, q)| {
                            ResearchQuery {
                                query: q.get("query").and_then(|v| v.as_str()).unwrap_or(original_query).to_string(),
                                intent: q.get("intent").and_then(|v| v.as_str()).unwrap_or("related").to_string(),
                                priority: (100 - (idx as u32 * 10)).max(10),
                            }
                        }).collect();
                    }
                }
            }
            fallback_queries(original_query, depth)
        }
        Err(e) => {
            println!("[deep_research] LLM query gen failed: {}, using fallback", e);
            fallback_queries(original_query, depth)
        }
    }
}

fn fallback_queries(query: &str, depth: &str) -> Vec<ResearchQuery> {
    let mut queries = vec![
        ResearchQuery { query: query.to_string(), intent: "primary".to_string(), priority: 100 },
    ];
    let extras: Vec<String> = match depth {
        "quick" => vec![format!("{} 2025 2026", query)],
        "deep" => vec![
            format!("{} comprehensive guide 2025 2026", query),
            format!("{} latest research findings", query),
            format!("{} expert analysis opinions", query),
            format!("{} statistics data trends", query),
            format!("{} comparison alternatives", query),
            format!("{} future predictions", query),
        ],
        _ => vec![
            format!("{} overview 2025 2026", query),
            format!("{} latest developments", query),
            format!("{} expert analysis", query),
        ],
    };
    for (i, q) in extras.into_iter().enumerate() {
        queries.push(ResearchQuery {
            query: q, intent: "related".to_string(), priority: 80 - (i as u32 * 10),
        });
    }
    queries
}

/// Search Google using Chrome with stealth and extract results
async fn chrome_search(
    query: &str,
    browser: &mut BrowserClient,
    search_index: usize,
) -> Vec<ResearchSource> {
    let encoded = query.replace(' ', "+").replace('"', "%22").replace('&', "%26");
    let url = format!("https://www.google.com/search?q={}&hl=en&gl=us&pws=0", encoded);

    println!("[deep_research] 🌐 Chrome → Google: \"{}\"", query);

    // Use stealth page (about:blank → inject stealth → set cookies → navigate)
    if let Err(e) = browser.new_page_stealth(&url).await {
        println!("[deep_research] Failed to open search: {}", e);
        return vec![];
    }

    // Wait for results — human-like timing
    let wait = if search_index == 0 { 2800 } else { 2000 } + (search_index as u64 * 137) % 500;
    tokio::time::sleep(Duration::from_millis(wait)).await;

    // Dismiss cookie consent if present
    let _ = browser.dismiss_cookie_consent().await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Extract search results
    let json = match browser.evaluate_js(GOOGLE_EXTRACT_JS).await {
        Ok(j) => j,
        Err(e) => {
            println!("[deep_research] Extract failed: {}", e);
            close_last_tab(browser).await;
            return vec![];
        }
    };

    let sources = parse_search_results(&json, 8);
    println!("[deep_research] ✅ Found {} results for: \"{}\"", sources.len(), query);

    close_last_tab(browser).await;

    // Delay between searches
    let delay = 800 + (search_index as u64 * 200) % 600;
    tokio::time::sleep(Duration::from_millis(delay)).await;

    sources
}

/// Visit a page in Chrome and extract its full content
async fn chrome_read_page(url: &str, browser: &mut BrowserClient) -> Option<String> {
    println!("[deep_research] 📖 Chrome → Reading: {}", url);

    if browser.new_page_stealth(url).await.is_err() {
        return None;
    }

    // Wait for page to load
    tokio::time::sleep(Duration::from_millis(2500)).await;

    let content = match browser.evaluate_js(PAGE_CONTENT_EXTRACT_JS).await {
        Ok(text) if text.len() > 150 => {
            println!("[deep_research] ✅ Extracted {} chars from page", text.len());
            Some(text)
        }
        Ok(text) => {
            println!("[deep_research] ⚠️ Page too short ({} chars)", text.len());
            None
        }
        Err(e) => {
            println!("[deep_research] ❌ Extract failed: {}", e);
            None
        }
    };

    close_last_tab(browser).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    content
}

fn parse_search_results(json_str: &str, max: usize) -> Vec<ResearchSource> {
    serde_json::from_str::<Vec<serde_json::Value>>(json_str)
        .unwrap_or_default()
        .into_iter()
        .take(max)
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.to_string();
            let url = item.get("url")?.as_str()?.to_string();
            let snippet = item.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() || url.is_empty() || !url.starts_with("http") { return None; }
            Some(ResearchSource {
                title, url, snippet,
                content: String::new(),
                credibility_score: 0.75,
                published_date: None,
            })
        })
        .collect()
}

async fn close_last_tab(browser: &mut BrowserClient) {
    let count = browser.page_count();
    if count > 1 { let _ = browser.close_page(count - 1).await; }
}

// ============================================================
// Phase 2: LLM Synthesis — The Quality Layer
// ============================================================

/// Send all extracted content to Claude for intelligent synthesis.
/// Claude formats the output based on the user's original request.
async fn llm_synthesize(
    original_query: &str,
    sources: &[ResearchSource],
    depth: &str,
    api_key: &str,
    model: &str,
) -> (String, Vec<String>, Vec<String>) {
    let client = AnthropicClient::new(api_key.to_string(), model.to_string());

    // Build rich context from all extracted content
    let mut source_context = String::new();
    for (idx, source) in sources.iter().enumerate().take(12) {
        source_context.push_str(&format!(
            "\n━━━ Source {} ━━━\nTitle: {}\nURL: {}\nSnippet: {}\n",
            idx + 1, source.title, source.url, source.snippet
        ));
        if !source.content.is_empty() {
            // Include full extracted content (up to 2500 chars per source)
            let preview = if source.content.len() > 2500 {
                &source.content[..2500]
            } else {
                &source.content
            };
            source_context.push_str(&format!("Full Content:\n{}\n", preview));
        }
    }

    let depth_instruction = match depth {
        "quick" => "Provide a focused, concise answer.",
        "deep" => "Provide an exhaustive, highly detailed analysis covering every angle.",
        _ => "Provide a thorough, well-structured answer.",
    };

    let prompt = format!(
        r#"You are an expert research analyst. I searched the web and extracted content from multiple sources. Synthesize these into a high-quality research report.

ORIGINAL QUESTION: "{query}"

EXTRACTED SOURCES:
{sources}

INSTRUCTIONS:
1. {depth}
2. Write a comprehensive, well-structured answer using markdown formatting.
3. Cite sources inline using [Source N] notation (e.g., [Source 1], [Source 2]).
4. Extract 5-7 KEY FINDINGS as bullet points.
5. Suggest 3-4 FOLLOW-UP QUESTIONS.
6. Be specific: include dates, numbers, statistics where available.
7. Note conflicting information if found.
8. Format the output professionally — this goes directly to the end user.

FORMAT YOUR RESPONSE AS JSON:
{{
  "synthesis": "The full markdown answer with [Source N] citations...",
  "key_findings": ["Finding 1", "Finding 2", ...],
  "follow_up_questions": ["Question 1", "Question 2", ...]
}}"#,
        query = original_query,
        sources = source_context,
        depth = depth_instruction,
    );

    let messages = vec![Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text { text: prompt }],
    }];

    match client.complete(None, messages, None).await {
        Ok(result) => {
            let text: String = result.content.iter()
                .filter_map(|b| if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
                .collect();

            // Try to parse structured JSON response
            if let Some(start) = text.find('{') {
                if let Some(end) = text.rfind('}') {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                        let synthesis = parsed.get("synthesis")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&text)
                            .to_string();

                        let findings: Vec<String> = parsed.get("key_findings")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                            .unwrap_or_default();

                        let follow_up: Vec<String> = parsed.get("follow_up_questions")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                            .unwrap_or_default();

                        return (synthesis, findings, follow_up);
                    }
                }
            }

            // Fallback: return raw text
            (text, vec![], default_follow_ups(original_query))
        }
        Err(e) => {
            let fallback = format!(
                "Research found {} sources for \"{}\". Synthesis failed: {}. Raw findings from sources are available below.",
                sources.len(), original_query, e
            );
            (fallback, vec![], default_follow_ups(original_query))
        }
    }
}

fn default_follow_ups(query: &str) -> Vec<String> {
    vec![
        format!("What are the latest developments in {}?", query),
        format!("How does {} compare to alternatives?", query),
        format!("What are expert opinions on {}?", query),
    ]
}

// ============================================================
// Fallback: Claude Built-in Web Search (no Chrome needed)
// ============================================================

async fn research_with_claude_web_search(
    query: &str,
    depth: &str,
    api_key: &str,
    model: &str,
) -> (Vec<ResearchSource>, String, Vec<String>, Vec<String>) {
    let max_searches = match depth {
        "quick" => 5,
        "standard" => 10,
        "deep" => 20,
        _ => 10,
    };

    let client = AnthropicClient::new(api_key.to_string(), model.to_string());

    let system = format!(
        r#"You are an expert research analyst. Search the web thoroughly to answer the user's question.
Search from multiple angles. Provide a comprehensive answer with citations.
Format with markdown. Include Key Findings and Follow-up Questions sections."#
    );

    let messages = vec![Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text {
            text: format!("Research thoroughly: {}\n\nDepth: {}", query, depth),
        }],
    }];

    match client.complete_with_web_search(Some(system), messages, max_searches).await {
        Ok(result) => {
            let mut text = String::new();
            let mut sources = Vec::new();
            let mut seen_urls = std::collections::HashSet::new();

            for block in &result.content {
                match block {
                    ContentBlock::Text { text: t, .. } => text.push_str(t),
                    ContentBlock::WebSearchToolResult { content, .. } => {
                        if let Some(arr) = content.as_array() {
                            for r in arr {
                                if r.get("type").and_then(|v| v.as_str()) == Some("web_search_result") {
                                    let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let page_age = r.get("page_age").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    if !url.is_empty() && seen_urls.insert(url.clone()) {
                                        sources.push(ResearchSource {
                                            title, url, snippet: String::new(), content: String::new(),
                                            credibility_score: 0.9, published_date: page_age,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            let findings = extract_bullet_points(&text, "key findings");
            let follow_ups = extract_bullet_points(&text, "follow-up");

            (sources, text, findings, follow_ups)
        }
        Err(e) => {
            println!("[deep_research] Claude web search failed: {}", e);
            (vec![], format!("Research failed: {}", e), vec![], default_follow_ups(query))
        }
    }
}

fn extract_bullet_points(text: &str, section_keyword: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_section = false;
    for line in text.lines() {
        let t = line.trim().to_lowercase();
        if t.contains(section_keyword) { in_section = true; continue; }
        if in_section && line.trim().starts_with("##") { break; }
        if in_section {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("• ") {
                items.push(trimmed[2..].to_string());
            }
        }
    }
    items
}

// ============================================================
// Main Orchestrator
// ============================================================

/// Perform deep research:
///   1. Try Chrome (Google search + full page reading)
///   2. Pass ALL content to Claude LLM for polished synthesis
///   3. Falls back to Claude's built-in web_search if Chrome unavailable
pub async fn perform_deep_research(
    query: &str,
    depth: &str,
    api_key: &str,
    model: &str,
    browser_client: &SharedBrowserClient,
) -> Result<DeepResearchReport, String> {
    let start_time = std::time::Instant::now();
    println!("[deep_research] ========================================");
    println!("[deep_research] Starting research: \"{}\" (depth: {})", query, depth);
    println!("[deep_research] ========================================");

    // Step 1: Generate smart search queries using LLM
    let research_queries = generate_search_queries(query, depth, api_key, model).await;
    println!("[deep_research] Generated {} search queries", research_queries.len());

    // Step 2: Try Chrome-based research first
    let (all_sources, synthesized_answer, key_findings, follow_up_questions) = {
        let mut guard = browser_client.lock().await;
        let browser_was_open = guard.is_some();

        // Try to connect Chrome
        if guard.is_none() {
            println!("[deep_research] 🚀 Launching Chrome...");
            match crate::browser::BrowserClient::connect().await {
                Ok(client) => { *guard = Some(client); }
                Err(e) => {
                    println!("[deep_research] Chrome connect failed: {}, trying restart...", e);
                    match crate::browser::restart_chrome_with_debugging().await {
                        Ok(client) => { *guard = Some(client); }
                        Err(e2) => { println!("[deep_research] ⚠️ Chrome unavailable: {}", e2); }
                    }
                }
            }
        }

        if guard.is_some() {
            // ====== CHROME PATH: Search + Extract + LLM Synthesize ======
            println!("[deep_research] 🌐 Using Chrome for Google searches — watch the magic!");
            let browser = guard.as_mut().unwrap();
            let original_page_count = browser.page_count();
            let original_selected = browser.selected_page_index();

            // Phase 1a: Search Google for each query
            let mut all_sources: Vec<ResearchSource> = Vec::new();
            let mut results: Vec<ResearchResult> = Vec::new();

            for (idx, rq) in research_queries.iter().enumerate() {
                println!("[deep_research] 🔍 [{}/{}] ({}): \"{}\"", idx + 1, research_queries.len(), rq.intent, rq.query);

                match timeout(Duration::from_secs(20), chrome_search(&rq.query, browser, idx)).await {
                    Ok(sources) => {
                        results.push(ResearchResult {
                            query: rq.query.clone(),
                            sources: sources.clone(),
                            summary: String::new(),
                            confidence: if sources.is_empty() { 0.2 } else { 0.8 },
                        });
                        all_sources.extend(sources);
                    }
                    Err(_) => {
                        println!("[deep_research] ⏰ Search timed out: \"{}\"", rq.query);
                    }
                }
            }

            // Deduplicate by URL
            let mut seen = std::collections::HashSet::new();
            all_sources.retain(|s| seen.insert(s.url.clone()));
            println!("[deep_research] 📊 {} unique sources found", all_sources.len());

            // Phase 1b: Visit top pages and extract FULL content
            let max_pages = match depth {
                "quick" => 3,
                "standard" => 5,
                "deep" => 8,
                _ => 5,
            };

            for source in all_sources.iter_mut().take(max_pages) {
                match timeout(Duration::from_secs(12), chrome_read_page(&source.url, browser)).await {
                    Ok(Some(content)) => {
                        source.content = content;
                        source.credibility_score = 0.95; // higher for pages we actually read
                    }
                    _ => {}
                }
            }

            let read_count = all_sources.iter().filter(|s| !s.content.is_empty()).count();
            println!("[deep_research] 📚 Read full content from {} pages", read_count);

            // Cleanup Chrome
            if browser_was_open {
                let _ = browser.select_page(original_selected, false).await;
            } else {
                let _ = browser.close_all_pages().await;
                *guard = None;
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "tell application \"Google Chrome\" to quit"])
                    .output();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("taskkill")
                    .args(["/IM", "chrome.exe", "/T"])
                    .output();
                println!("[deep_research] ✅ Chrome closed");
            }

            // Phase 2: LLM Synthesis — the quality layer
            println!("[deep_research] 🧠 Sending {} sources to Claude for synthesis...", all_sources.len());
            let (synthesis, findings, follow_ups) =
                llm_synthesize(query, &all_sources, depth, api_key, model).await;

            (all_sources, synthesis, findings, follow_ups)
        } else {
            // ====== FALLBACK: Claude's built-in web_search ======
            drop(guard);
            println!("[deep_research] 📡 Chrome unavailable — using Claude's built-in web search");
            research_with_claude_web_search(query, depth, api_key, model).await
        }
    };

    let elapsed = start_time.elapsed();
    println!("[deep_research] ✅ Research complete in {:.1}s ({} sources)", elapsed.as_secs_f64(), all_sources.len());

    let confidence = if all_sources.iter().any(|s| !s.content.is_empty()) && all_sources.len() >= 5 {
        0.95
    } else if all_sources.len() >= 3 {
        0.85
    } else if !all_sources.is_empty() {
        0.7
    } else {
        0.4
    };

    Ok(DeepResearchReport {
        original_query: query.to_string(),
        research_queries,
        results: vec![ResearchResult {
            query: query.to_string(),
            sources: all_sources.clone(),
            summary: synthesized_answer.clone(),
            confidence,
        }],
        synthesized_answer,
        key_findings,
        sources: all_sources,
        follow_up_questions,
        confidence_score: confidence,
        research_depth: depth.to_string(),
    })
}

// ============================================================
// Report Formatting & Detection
// ============================================================

pub fn format_research_report(report: &DeepResearchReport) -> String {
    let method = if report.sources.iter().any(|s| !s.content.is_empty()) {
        "Chrome + Google + LLM Synthesis"
    } else if !report.sources.is_empty() {
        "Claude Web Search"
    } else {
        "Background Search"
    };

    format!(
        r#"# Deep Research: {}

**Depth:** {} | **Confidence:** {:.0}% | **Sources:** {} | **Method:** {}

---

{}

## Key Findings

{}

## Sources

{}

## Follow-up Questions

{}
"#,
        report.original_query,
        report.research_depth,
        report.confidence_score * 100.0,
        report.sources.len(),
        method,
        report.synthesized_answer,
        if report.key_findings.is_empty() {
            "_See report above._".to_string()
        } else {
            report.key_findings.iter().enumerate()
                .map(|(i, f)| format!("{}. {}", i + 1, f))
                .collect::<Vec<_>>().join("\n")
        },
        if report.sources.is_empty() {
            "_No sources found_".to_string()
        } else {
            report.sources.iter().enumerate()
                .map(|(i, s)| {
                    let read = if !s.content.is_empty() { " ✅ [read]" } else { "" };
                    let date = s.published_date.as_deref().map(|d| format!(" ({})", d)).unwrap_or_default();
                    format!("{}. [{}]({}) — {}{}{}", i + 1, s.title, s.url,
                        if s.snippet.is_empty() { "No description" } else { &s.snippet },
                        read, date)
                })
                .collect::<Vec<_>>().join("\n")
        },
        report.follow_up_questions.iter().map(|q| format!("- {}", q)).collect::<Vec<_>>().join("\n"),
    )
}

pub fn should_use_deep_research(query: &str) -> bool {
    let keywords = [
        "research", "deep dive", "comprehensive", "in-depth", "in depth",
        "thorough", "detailed analysis", "investigate", "explore in detail",
        "compare", "pros and cons", "latest developments",
        "what is the current state", "how does it work",
        "explain in detail", "find out about", "look into", "deep research",
    ];
    let q = query.to_lowercase();
    keywords.iter().any(|kw| q.contains(kw))
}

pub fn get_research_depth(query: &str) -> &'static str {
    let q = query.to_lowercase();
    if q.contains("quick") || q.contains("brief") { "quick" }
    else if q.contains("deep") || q.contains("comprehensive") || q.contains("thorough") { "deep" }
    else { "standard" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_deep_research() {
        assert!(should_use_deep_research("Research the history of AI"));
        assert!(should_use_deep_research("Give me a comprehensive analysis"));
        assert!(!should_use_deep_research("What time is it?"));
    }

    #[test]
    fn test_research_depth() {
        assert_eq!(get_research_depth("quick overview"), "quick");
        assert_eq!(get_research_depth("deep analysis"), "deep");
        assert_eq!(get_research_depth("tell me about AI"), "standard");
    }

    #[test]
    fn test_parse_search_results() {
        let json = r#"[{"title":"Test","url":"https://example.com","snippet":"A snippet"}]"#;
        let results = parse_search_results(json, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test");
    }

    #[test]
    fn test_extract_bullet_points() {
        let text = "Some text\n## Key Findings\n- One\n- Two\n## Sources";
        let items = extract_bullet_points(text, "key findings");
        assert_eq!(items, vec!["One", "Two"]);
    }
}
