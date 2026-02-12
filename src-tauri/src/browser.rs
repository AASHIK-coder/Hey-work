use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::accessibility::{
    AxNode, AxPropertyName, GetFullAxTreeParams,
};
use chromiumoxide::cdp::browser_protocol::dom::{
    BackendNodeId, GetBoxModelParams, ResolveNodeParams,
};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams,
    DispatchMouseEventType, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::network::SetCookieParams;
use chromiumoxide::cdp::browser_protocol::page::{
    AddScriptToEvaluateOnNewDocumentParams,
    CaptureScreenshotFormat, CloseParams, HandleJavaScriptDialogParams, NavigateParams,
    ReloadParams,
};
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::handler::Handler;
use chromiumoxide::Page;
use futures::StreamExt;
use tokio::sync::Mutex;

// paths to check for DevToolsActivePort (for connecting to existing chrome)
#[cfg(target_os = "macos")]
const CHROME_PROFILES: &[&str] = &[
    "Library/Application Support/Google/Chrome",
    "Library/Application Support/Google/Chrome Canary",
    "Library/Application Support/Arc/User Data",
    "Library/Application Support/Chromium",
];

#[cfg(target_os = "windows")]
const CHROME_PROFILES: &[&str] = &[
    "AppData/Local/Google/Chrome/User Data",
    "AppData/Local/Google/Chrome SxS/User Data",
];

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const CHROME_PROFILES: &[&str] = &[];

pub struct BrowserClient {
    browser: Browser,
    _handler_task: tokio::task::JoinHandle<()>,
    pages: Vec<Page>,
    selected_page_idx: usize,
    // snapshot state
    snapshot_id: u64,
    uid_to_backend_node: HashMap<String, BackendNodeId>,
}

impl BrowserClient {
    pub async fn connect() -> Result<Self> {
        // try to connect to existing chrome first
        if let Some(ws_url) = try_find_existing_chrome().await {
            println!("[browser] Connecting to existing Chrome at {}", ws_url);
            match Browser::connect(&ws_url).await {
                Ok((mut browser, handler)) => {
                    let handler_task = tokio::spawn(async move {
                        handler_loop(handler).await;
                    });

                    // fetch existing targets so we can see tabs that were already open
                    let _ = browser.fetch_targets().await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let pages = browser.pages().await.unwrap_or_default();
                    println!("[browser] Found {} existing pages", pages.len());

                    return Ok(Self {
                        browser,
                        _handler_task: handler_task,
                        pages,
                        selected_page_idx: 0,
                        snapshot_id: 0,
                        uid_to_backend_node: HashMap::new(),
                    });
                }
                Err(e) => {
                    println!("[browser] Failed to connect to existing Chrome: {}", e);
                }
            }
        }

        // no existing chrome with debugging, try to launch a new one
        // on macOS, this only works if Chrome isn't already running
        println!("[browser] Launching Chrome with user profile...");
        let (browser, handler) = match launch_chrome_with_profile().await {
            Ok(b) => b,
            Err(e) => {
                // check if chrome is already running without debugging
                if is_chrome_running() {
                    return Err(anyhow!("CHROME_NEEDS_RESTART"));
                }
                return Err(e);
            }
        };

        let handler_task = tokio::spawn(async move {
            handler_loop(handler).await;
        });

        let pages = browser.pages().await.unwrap_or_default();
        Ok(Self {
            browser,
            _handler_task: handler_task,
            pages,
            selected_page_idx: 0,
            snapshot_id: 0,
            uid_to_backend_node: HashMap::new(),
        })
    }

    fn selected_page(&self) -> Result<&Page> {
        self.pages
            .get(self.selected_page_idx)
            .ok_or_else(|| anyhow!("no page selected"))
    }

    // refresh page list from browser
    async fn refresh_pages(&mut self) -> Result<()> {
        self.pages = self.browser.pages().await?;
        if self.selected_page_idx >= self.pages.len() && !self.pages.is_empty() {
            self.selected_page_idx = 0;
        }
        Ok(())
    }

    // tool: take_snapshot
    pub async fn take_snapshot(&mut self, verbose: bool) -> Result<String> {
        println!("[browser] take_snapshot: starting");
        let start = std::time::Instant::now();

        let page = self.selected_page()?;
        println!("[browser] take_snapshot: got page, calling GetFullAxTree...");

        let resp = page
            .execute(GetFullAxTreeParams::builder().build())
            .await
            .context("failed to get a11y tree")?;
        println!("[browser] take_snapshot: GetFullAxTree returned in {:?}", start.elapsed());

        self.snapshot_id += 1;
        self.uid_to_backend_node.clear();

        let nodes = resp.result.nodes;
        println!("[browser] take_snapshot: formatting {} nodes", nodes.len());
        let snapshot_text = format_ax_tree(&nodes, self.snapshot_id, verbose, &mut self.uid_to_backend_node);
        println!("[browser] take_snapshot: done in {:?}, {} chars", start.elapsed(), snapshot_text.len());

        Ok(snapshot_text)
    }

    // tool: click
    pub async fn click(&mut self, uid: &str, dbl_click: bool) -> Result<String> {
        println!("[browser] click: resolving uid {}", uid);
        let start = std::time::Instant::now();
        let (x, y) = self.resolve_uid_to_point(uid).await?;
        println!("[browser] click: resolved to ({}, {}) in {:?}", x, y, start.elapsed());
        let page = self.selected_page()?;

        // move mouse
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseMoved)
                .x(x)
                .y(y)
                .build()
                .unwrap(),
        )
        .await?;

        let click_count = if dbl_click { 2 } else { 1 };

        // mouse down
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MousePressed)
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .click_count(click_count)
                .build()
                .unwrap(),
        )
        .await?;

        // mouse up
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseReleased)
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .click_count(click_count)
                .build()
                .unwrap(),
        )
        .await?;

        let action = if dbl_click { "double clicked" } else { "clicked" };
        Ok(format!("Successfully {action} on element"))
    }

    // tool: hover
    pub async fn hover(&mut self, uid: &str) -> Result<String> {
        let (x, y) = self.resolve_uid_to_point(uid).await?;
        let page = self.selected_page()?;

        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseMoved)
                .x(x)
                .y(y)
                .build()
                .unwrap(),
        )
        .await?;

        Ok("Successfully hovered over element".to_string())
    }

    // tool: fill
    pub async fn fill(&mut self, uid: &str, value: &str) -> Result<String> {
        // click first to focus
        self.click(uid, false).await?;

        let page = self.selected_page()?;

        // clear existing content with ctrl+a then delete
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyDown)
                .key("a")
                .modifiers(2) // ctrl/cmd
                .build()
                .unwrap(),
        )
        .await?;
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyUp)
                .key("a")
                .build()
                .unwrap(),
        )
        .await?;

        // type each character
        for c in value.chars() {
            page.execute(
                DispatchKeyEventParams::builder()
                    .r#type(DispatchKeyEventType::Char)
                    .text(c.to_string())
                    .build()
                    .unwrap(),
            )
            .await?;
        }

        Ok("Successfully filled element".to_string())
    }

    // tool: press_key
    pub async fn press_key(&mut self, key: &str) -> Result<String> {
        let page = self.selected_page()?;

        // parse modifiers from key string like "Control+A" or "Enter"
        let parts: Vec<&str> = key.split('+').collect();
        let (modifiers, key_name) = if parts.len() > 1 {
            let mods = &parts[..parts.len() - 1];
            let key = parts[parts.len() - 1];
            let mut mod_flags = 0;
            for m in mods {
                match m.to_lowercase().as_str() {
                    "control" | "ctrl" => mod_flags |= 2,
                    "alt" | "option" => mod_flags |= 1,
                    "shift" => mod_flags |= 8,
                    "meta" | "cmd" | "command" => mod_flags |= 4,
                    _ => {}
                }
            }
            (mod_flags, key)
        } else {
            (0, key)
        };

        // key down
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyDown)
                .key(key_name)
                .modifiers(modifiers)
                .build()
                .unwrap(),
        )
        .await?;

        // key up
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyUp)
                .key(key_name)
                .modifiers(modifiers)
                .build()
                .unwrap(),
        )
        .await?;

        Ok(format!("Successfully pressed key: {key}"))
    }

    // tool: scroll - uses JS for reliability (CDP Input.dispatchMouseEvent can timeout)
    pub async fn scroll(&mut self, direction: &str, amount: Option<i64>) -> Result<String> {
        let page = self.selected_page()?;
        let pixels = amount.unwrap_or(500);

        let (delta_x, delta_y) = match direction.to_lowercase().as_str() {
            "up" => (0, -pixels),
            "down" => (0, pixels),
            "left" => (-pixels, 0),
            "right" => (pixels, 0),
            _ => return Err(anyhow!("Invalid scroll direction: {}", direction)),
        };

        // use JS scrollBy - faster and more reliable than CDP mouse wheel events
        let js = format!("window.scrollBy({}, {})", delta_x, delta_y);

        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            page.evaluate(js)
        ).await {
            Ok(Ok(_)) => Ok(format!("Scrolled {} by {} pixels", direction, pixels)),
            Ok(Err(e)) => Err(anyhow!("Scroll failed: {e}")),
            Err(_) => Err(anyhow!("Scroll timed out")),
        }
    }

    // tool: navigate_page
    pub async fn navigate_page(
        &mut self,
        nav_type: &str,
        url: Option<&str>,
        ignore_cache: bool,
    ) -> Result<String> {
        let page = self.selected_page()?;

        match nav_type {
            "url" => {
                let url = url.ok_or_else(|| anyhow!("url required for type=url"))?;
                // don't wait for full page load - heavy sites timeout
                // agent can take_snapshot to verify when ready
                let nav_future = page.execute(NavigateParams::builder().url(url).build().unwrap());
                match tokio::time::timeout(std::time::Duration::from_secs(5), nav_future).await {
                    Ok(Ok(_)) => Ok(format!("Navigated to {url}")),
                    Ok(Err(e)) => Err(anyhow!("Navigation failed: {e}")),
                    Err(_) => Ok(format!("Navigating to {url} (page still loading, use take_snapshot to check)")),
                }
            }
            "back" => {
                // use js history.back()
                page.evaluate("history.back()").await?;
                Ok("Successfully navigated back".to_string())
            }
            "forward" => {
                page.evaluate("history.forward()").await?;
                Ok("Successfully navigated forward".to_string())
            }
            "reload" => {
                page.execute(
                    ReloadParams::builder()
                        .ignore_cache(ignore_cache)
                        .build(),
                )
                .await?;
                Ok("Successfully reloaded page".to_string())
            }
            _ => Err(anyhow!("unknown navigation type: {nav_type}")),
        }
    }

    // tool: wait_for
    // uses fast JS evaluation instead of heavy a11y tree polling
    pub async fn wait_for(&mut self, text: &str, timeout_ms: u64) -> Result<String> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let page = self.selected_page()?;

        // js to check if text exists in page
        let js = format!(
            r#"document.body && document.body.innerText.includes("{}")"#,
            text.replace('\\', "\\\\").replace('"', "\\\"")
        );

        loop {
            // check timeout FIRST
            if start.elapsed() > timeout {
                return Err(anyhow!("timeout waiting for text: {text}"));
            }

            // use JS evaluation - much faster than GetFullAXTree
            let eval_result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                page.evaluate(js.clone())
            ).await;

            match eval_result {
                Ok(Ok(result)) => {
                    if let Ok(found) = result.into_value::<bool>() {
                        if found {
                            return Ok(format!("Element with text \"{text}\" found"));
                        }
                    }
                }
                Ok(Err(e)) => {
                    println!("[browser] wait_for eval error: {e}");
                }
                Err(_) => {
                    println!("[browser] wait_for eval timed out");
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    // tool: upload_file
    pub async fn upload_file(&mut self, uid: &str, file_path: &str) -> Result<String> {
        let backend_node_id = self.get_backend_node_id(uid)?;
        let page = self.selected_page()?;

        // resolve node to get remote object
        let resolve_resp = page
            .execute(
                ResolveNodeParams::builder()
                    .backend_node_id(backend_node_id)
                    .build(),
            )
            .await?;

        let object_id = resolve_resp
            .result
            .object
            .object_id
            .ok_or_else(|| anyhow!("could not resolve element"))?;

        // set file via js
        let js = format!(
            r#"
            (function(files) {{
                const input = this;
                const dt = new DataTransfer();
                for (const f of files) {{
                    dt.items.add(new File([''], f));
                }}
                input.files = dt.files;
                input.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }})(["{file_path}"])
            "#
        );

        page.evaluate(format!(
            "((obj) => {{ const el = obj; {js} }})(document.querySelector('[data-object-id=\"{}\"]'))",
            object_id.inner()
        ))
        .await?;

        Ok(format!("File uploaded: {file_path}"))
    }

    // tool: new_page
    pub async fn new_page(&mut self, url: &str) -> Result<String> {
        let page = self.browser.new_page(url).await?;
        
        // Inject stealth scripts into the new page/tab so that
        // navigator.webdriver and other automation signals are hidden
        let stealth_js = Self::stealth_script();
        let _ = page.execute(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(stealth_js)
                .build()
                .unwrap()
        ).await;
        // Also run immediately on current context
        let _ = page.evaluate(Self::stealth_script().to_string()).await;
        
        self.pages.push(page);
        self.selected_page_idx = self.pages.len() - 1;
        Ok(format!("Created new page and navigated to {url}"))
    }

    /// Open a new page with FULL stealth protection.
    /// This is critical for Google: opens about:blank FIRST, injects stealth
    /// scripts and cookies, THEN navigates to the target URL.
    /// This ensures navigator.webdriver is hidden BEFORE Google's scripts run.
    pub async fn new_page_stealth(&mut self, url: &str) -> Result<String> {
        println!("[browser] new_page_stealth: opening about:blank first");
        
        // Step 1: Create a blank page — no target site scripts run yet
        let page = self.browser.new_page("about:blank").await?;
        
        // Step 2: Inject stealth via addScriptToEvaluateOnNewDocument
        // This registers the script to run BEFORE any page JS on future navigations
        let stealth_js = Self::stealth_script();
        let _ = page.execute(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(stealth_js)
                .build()
                .unwrap()
        ).await;
        
        // Step 3: Pre-set Google consent cookies via CDP Network.setCookie
        // This prevents the cookie consent overlay from appearing
        Self::set_google_cookies_on_page(&page).await;
        
        // Step 4: NOW navigate to the actual URL — stealth runs before page JS
        println!("[browser] new_page_stealth: navigating to {}", url);
        let nav_result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            page.goto(url)
        ).await;
        
        match nav_result {
            Ok(Ok(_)) => println!("[browser] new_page_stealth: navigation complete"),
            Ok(Err(e)) => println!("[browser] new_page_stealth: nav error (continuing): {}", e),
            Err(_) => println!("[browser] new_page_stealth: nav timeout (page still loading)"),
        }
        
        self.pages.push(page);
        self.selected_page_idx = self.pages.len() - 1;
        Ok(format!("Created stealth page and navigated to {url}"))
    }

    /// Set Google consent and preference cookies on a page via CDP
    async fn set_google_cookies_on_page(page: &Page) {
        // SOCS cookie: Google's consent acceptance cookie (GDPR/CCPA)
        let _ = page.execute(
            SetCookieParams::builder()
                .name("SOCS")
                .value("CAISNQgDEitib3FfaWRlbnRpdHlmcm9udGVuZHVpc2VydmVyXzIwMjMwODI5LjA3X3AxGgJlbiADGgYIgOLQqgY")
                .domain(".google.com")
                .path("/")
                .build()
                .unwrap()
        ).await;
        
        // CONSENT cookie: Legacy consent cookie (backup)
        let _ = page.execute(
            SetCookieParams::builder()
                .name("CONSENT")
                .value("YES+cb.20210720-07-p0.en+FX+688")
                .domain(".google.com")
                .path("/")
                .build()
                .unwrap()
        ).await;
        
        // AEC cookie: Helps avoid automated traffic detection
        let _ = page.execute(
            SetCookieParams::builder()
                .name("AEC")
                .value("AVYB7cpOSairVfJuni4yDHKnOGdCNy3USxmAllmGbIK9sJlvxAolWJJoLQ")
                .domain(".google.com")
                .path("/")
                .build()
                .unwrap()
        ).await;

        // NID cookie: Google preferences (language=en, region=US)
        let _ = page.execute(
            SetCookieParams::builder()
                .name("NID")
                .value("511=some-pref-value")
                .domain(".google.com")
                .path("/")
                .build()
                .unwrap()
        ).await;
        
        println!("[browser] Google consent cookies set via CDP");
    }

    /// Try to dismiss any cookie consent overlay on the current page
    pub async fn dismiss_cookie_consent(&mut self) -> Result<String> {
        let page = self.selected_page()?;
        let dismiss_js = r#"
        (function() {
            // Strategy 1: Google's consent buttons (multiple known selectors)
            var selectors = [
                'button#L2AGLb',
                'button[aria-label="Accept all"]',
                'button[aria-label="Reject all"]',
                'div.QS5gu.sy4vM',
                'form[action*="consent"] button',
                'form[action*="consent"] input[type="submit"]'
            ];
            for (var sel of selectors) {
                var btn = document.querySelector(sel);
                if (btn) {
                    btn.click();
                    return 'clicked:' + sel;
                }
            }
            
            // Strategy 2: Look for buttons with "Accept" text
            var buttons = document.querySelectorAll('button, [role="button"]');
            for (var btn of buttons) {
                var text = (btn.innerText || btn.textContent || '').toLowerCase().trim();
                if (text === 'accept all' || text === 'i agree' || text === 'accept' || text === 'agree') {
                    btn.click();
                    return 'clicked:text:' + text;
                }
            }
            
            // Strategy 3: Check if we're on consent.google.com and try to submit
            if (window.location.hostname.includes('consent.google')) {
                var form = document.querySelector('form');
                if (form) { form.submit(); return 'submitted:consent-form'; }
            }
            
            return 'no-consent-found';
        })()
        "#;
        
        let result = page.evaluate(dismiss_js.to_string()).await
            .map(|v| v.into_value::<String>().unwrap_or_default())
            .unwrap_or_default();
        
        println!("[browser] dismiss_cookie_consent: {}", result);
        Ok(result)
    }

    // tool: list_pages
    pub async fn list_pages(&mut self) -> Result<String> {
        self.refresh_pages().await?;

        let mut result = String::new();
        for (idx, page) in self.pages.iter().enumerate() {
            let url = page.url().await?.unwrap_or_default();
            let selected = if idx == self.selected_page_idx {
                " [selected]"
            } else {
                ""
            };
            result.push_str(&format!("{idx}: {url}{selected}\n"));
        }

        if result.is_empty() {
            result = "No pages open".to_string();
        }

        Ok(result)
    }

    // tool: select_page
    pub async fn select_page(&mut self, page_idx: usize, bring_to_front: bool) -> Result<String> {
        self.refresh_pages().await?;

        if page_idx >= self.pages.len() {
            return Err(anyhow!(
                "page index {page_idx} out of range (0..{})",
                self.pages.len()
            ));
        }

        self.selected_page_idx = page_idx;

        if bring_to_front {
            let page = &self.pages[page_idx];
            page.bring_to_front().await?;
        }

        Ok(format!("Selected page {page_idx}"))
    }

    // tool: close_page
    pub async fn close_page(&mut self, page_idx: usize) -> Result<String> {
        self.refresh_pages().await?;

        if self.pages.len() <= 1 {
            return Err(anyhow!("cannot close the last open page"));
        }

        if page_idx >= self.pages.len() {
            return Err(anyhow!(
                "page index {page_idx} out of range (0..{})",
                self.pages.len()
            ));
        }

        let page = &self.pages[page_idx];
        page.execute(CloseParams::default()).await?;

        // remove from our list
        self.pages.remove(page_idx);

        // adjust selected index if needed
        if self.selected_page_idx >= self.pages.len() {
            self.selected_page_idx = self.pages.len().saturating_sub(1);
        }

        Ok(format!("Closed page {page_idx}"))
    }

    // tool: drag (drag element from one uid to another)
    pub async fn drag(&mut self, from_uid: &str, to_uid: &str) -> Result<String> {
        let (from_x, from_y) = self.resolve_uid_to_point(from_uid).await?;
        let (to_x, to_y) = self.resolve_uid_to_point(to_uid).await?;
        let page = self.selected_page()?;

        // mouse down at source
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MousePressed)
                .x(from_x)
                .y(from_y)
                .button(MouseButton::Left)
                .click_count(1)
                .build()
                .unwrap(),
        )
        .await?;

        // move to target
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseMoved)
                .x(to_x)
                .y(to_y)
                .button(MouseButton::Left)
                .build()
                .unwrap(),
        )
        .await?;

        // mouse up at target
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseReleased)
                .x(to_x)
                .y(to_y)
                .button(MouseButton::Left)
                .click_count(1)
                .build()
                .unwrap(),
        )
        .await?;

        Ok("Successfully dragged element".to_string())
    }

    // tool: fill_form (fill multiple form elements at once)
    pub async fn fill_form(&mut self, elements: &[(String, String)]) -> Result<String> {
        let mut filled = 0;
        for (uid, value) in elements {
            self.fill(uid, value).await?;
            filled += 1;
        }
        Ok(format!("Filled {filled} form elements"))
    }

    // tool: handle_dialog (accept/dismiss browser dialogs)
    pub async fn handle_dialog(&mut self, accept: bool, prompt_text: Option<&str>) -> Result<String> {
        let page = self.selected_page()?;

        let params = if let Some(text) = prompt_text {
            HandleJavaScriptDialogParams::builder()
                .accept(accept)
                .prompt_text(text)
                .build()
                .unwrap()
        } else {
            HandleJavaScriptDialogParams::builder()
                .accept(accept)
                .build()
                .unwrap()
        };

        page.execute(params).await?;

        let action = if accept { "accepted" } else { "dismissed" };
        Ok(format!("Successfully {action} dialog"))
    }

    // tool: screenshot - capture page as base64 jpeg
    pub async fn screenshot(&self) -> Result<String> {
        let page = self.selected_page()?;

        let params = ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Jpeg)
            .quality(60)
            .build();

        let bytes = page.screenshot(params).await?;
        Ok(BASE64.encode(&bytes))
    }

    // helper: get backend node id from uid
    fn get_backend_node_id(&self, uid: &str) -> Result<BackendNodeId> {
        // validate snapshot id
        let parts: Vec<&str> = uid.split('_').collect();
        if parts.len() != 2 {
            return Err(anyhow!("invalid uid format: {uid}"));
        }

        let snapshot_id: u64 = parts[0]
            .parse()
            .map_err(|_| anyhow!("invalid snapshot id in uid"))?;

        if snapshot_id != self.snapshot_id {
            return Err(anyhow!(
                "stale uid from snapshot {snapshot_id}, current is {}. take a new snapshot first.",
                self.snapshot_id
            ));
        }

        self.uid_to_backend_node
            .get(uid)
            .copied()
            .ok_or_else(|| anyhow!("uid not found: {uid}"))
    }

    // helper: resolve uid to center point
    async fn resolve_uid_to_point(&self, uid: &str) -> Result<(f64, f64)> {
        let backend_node_id = self.get_backend_node_id(uid)?;
        let page = self.selected_page()?;

        let box_resp = page
            .execute(
                GetBoxModelParams::builder()
                    .backend_node_id(backend_node_id)
                    .build(),
            )
            .await
            .context("failed to get box model for element")?;

        let model = box_resp.result.model;
        // content quad: 4 points (x1,y1,x2,y2,x3,y3,x4,y4)
        let quad = model.content.inner();
        let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
        let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;

        Ok((x, y))
    }

    // === Stealth & Anti-Detection ===

    /// Returns the stealth JavaScript that overrides automation detection signals.
    /// This script hides navigator.webdriver, spoofs plugins, languages, etc.
    fn stealth_script() -> &'static str {
        r#"
            // 1. Override navigator.webdriver — the #1 detection signal
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });

            // 2. Override navigator.plugins — empty in headless/automation
            Object.defineProperty(navigator, 'plugins', {
                get: () => {
                    const p = [
                        { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
                        { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai', description: '' },
                        { name: 'Native Client', filename: 'internal-nacl-plugin', description: '' }
                    ];
                    p.length = 3;
                    return p;
                }
            });

            // 3. Override navigator.languages
            Object.defineProperty(navigator, 'languages', { get: () => ['en-US', 'en'] });

            // 4. Override permissions API (Notification permission query is a common check)
            const originalQuery = window.Notification && Notification.permission
                ? Notification.permission : 'default';
            if (navigator.permissions) {
                const origQuery = navigator.permissions.query;
                navigator.permissions.query = (parameters) => (
                    parameters.name === 'notifications'
                        ? Promise.resolve({ state: originalQuery })
                        : origQuery(parameters)
                );
            }

            // 5. Override chrome.runtime to look like a real browser extension environment
            if (!window.chrome) window.chrome = {};
            if (!window.chrome.runtime) {
                window.chrome.runtime = {
                    connect: function() {},
                    sendMessage: function() {},
                    PlatformOs: { MAC: 'mac', WIN: 'win', ANDROID: 'android', CROS: 'cros', LINUX: 'linux', OPENBSD: 'openbsd' },
                    PlatformArch: { ARM: 'arm', X86_32: 'x86-32', X86_64: 'x86-64', MIPS: 'mips', MIPS64: 'mips64' },
                    PlatformNaclArch: { ARM: 'arm', X86_32: 'x86-32', X86_64: 'x86-64', MIPS: 'mips', MIPS64: 'mips64' },
                    RequestUpdateCheckStatus: { THROTTLED: 'throttled', NO_UPDATE: 'no_update', UPDATE_AVAILABLE: 'update_available' },
                    OnInstalledReason: { INSTALL: 'install', UPDATE: 'update', CHROME_UPDATE: 'chrome_update', SHARED_MODULE_UPDATE: 'shared_module_update' },
                    OnRestartRequiredReason: { APP_UPDATE: 'app_update', OS_UPDATE: 'os_update', PERIODIC: 'periodic' },
                };
            }

            // 6. Spoof WebGL vendor/renderer (headless has different values)
            const getParameter = WebGLRenderingContext.prototype.getParameter;
            WebGLRenderingContext.prototype.getParameter = function(parameter) {
                if (parameter === 37445) return 'Intel Inc.';
                if (parameter === 37446) return 'Intel Iris OpenGL Engine';
                return getParameter.call(this, parameter);
            };

            // 7. Suppress `cdc_` property on HTMLElement (ChromeDriver detection)
            // Some sites check for the presence of `$cdc_` prefixed properties
            try {
                const ownKeys = Object.keys;
                Object.keys = function(obj) {
                    return ownKeys(obj).filter(k => !k.startsWith('$cdc_'));
                };
            } catch(e) {}

            // 8. Override connection info (headless uses different value)
            Object.defineProperty(navigator, 'connection', {
                get: () => ({
                    rtt: 50,
                    downlink: 10,
                    effectiveType: '4g',
                    saveData: false
                })
            });

            // 9. Override navigator.hardwareConcurrency (headless defaults to 2)
            Object.defineProperty(navigator, 'hardwareConcurrency', { get: () => 8 });

            // 10. Override navigator.deviceMemory (not present in headless)
            Object.defineProperty(navigator, 'deviceMemory', { get: () => 8 });
        "#
    }

    /// Inject stealth scripts into the currently selected page so Google doesn't detect automation.
    /// Uses Page.addScriptToEvaluateOnNewDocument which runs before any page JS.
    pub async fn inject_stealth(&self) -> Result<()> {
        let page = self.selected_page()?;
        let stealth_js = Self::stealth_script();

        page.execute(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(stealth_js)
                .build()
                .unwrap()
        ).await?;

        // Also immediately run the script on the current page context
        let _ = page.evaluate(stealth_js.to_string()).await;

        println!("[browser] Stealth scripts injected");
        Ok(())
    }

    // === Deep Research helpers ===

    /// Evaluate JavaScript on the current page and return result as string
    pub async fn evaluate_js(&mut self, js: &str) -> Result<String> {
        let page = self.selected_page()?;
        let eval_result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            page.evaluate(js.to_string()),
        )
        .await;

        match eval_result {
            Ok(Ok(result)) => Ok(result.into_value::<String>().unwrap_or_default()),
            Ok(Err(e)) => Err(anyhow!("JS evaluation failed: {}", e)),
            Err(_) => Err(anyhow!("JS evaluation timed out")),
        }
    }

    /// Get the current page URL
    pub async fn current_url(&mut self) -> Result<String> {
        let page = self.selected_page()?;
        Ok(page.url().await?.unwrap_or_default())
    }

    /// Get the number of open pages/tabs
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Get the currently selected page index
    pub fn selected_page_index(&self) -> usize {
        self.selected_page_idx
    }

    /// Close all pages/tabs (used for cleanup after research)
    pub async fn close_all_pages(&mut self) -> Result<()> {
        self.refresh_pages().await?;
        // Close pages from last to first, keeping at least one (Chrome needs it)
        while self.pages.len() > 1 {
            let idx = self.pages.len() - 1;
            let page = &self.pages[idx];
            let _ = page.execute(CloseParams::default()).await;
            self.pages.remove(idx);
        }
        // Navigate the last remaining page to blank so nothing visible
        if let Some(page) = self.pages.first() {
            let _ = page.execute(NavigateParams::builder().url("about:blank").build().unwrap()).await;
        }
        self.selected_page_idx = 0;
        Ok(())
    }
}

// handler event loop
async fn handler_loop(mut handler: Handler) {
    while let Some(event) = handler.next().await {
        if event.is_err() {
            break;
        }
    }
}

fn profile_base_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return PathBuf::from(home);
        }
    }

    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn chrome_debug_profile_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = dirs::data_local_dir() {
            return base.join("hey-work").join("heywork-chrome");
        }
    }
    profile_base_dir().join(".heywork-chrome")
}

fn find_chrome_binary() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let p = PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        return p.exists().then_some(p);
    }

    #[cfg(target_os = "windows")]
    {
        let local = std::env::var("LOCALAPPDATA").ok();
        let pf = std::env::var("ProgramFiles").ok();
        let pf86 = std::env::var("ProgramFiles(x86)").ok();
        let candidates = [
            local.map(|p| PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
            pf.map(|p| PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
            pf86.map(|p| PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
        ];
        return candidates.into_iter().flatten().find(|p| p.exists());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

// check if chrome is already running
fn is_chrome_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        return std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq chrome.exe"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("chrome.exe"))
            .unwrap_or(false);
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("pgrep")
            .args(["-x", "Google Chrome"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

// restart chrome with debugging enabled (macOS)
// returns a connected BrowserClient if successful
pub async fn restart_chrome_with_debugging() -> Result<BrowserClient> {
    // try graceful quit first
    println!("[browser] Quitting Chrome...");
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("osascript")
            .args(["-e", "tell application \"Google Chrome\" to quit"])
            .output()
            .context("failed to quit Chrome")?;
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", "chrome.exe", "/T"])
            .output();
    }

    // wait for chrome to quit gracefully
    for _ in 0..6 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        if !is_chrome_running() {
            break;
        }
    }

    // if still running, force kill
    if is_chrome_running() {
        println!("[browser] Chrome didn't quit gracefully, force killing...");
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "chrome.exe", "/T"])
            .output();
        #[cfg(not(target_os = "windows"))]
        let _ = std::process::Command::new("pkill")
            .args(["-9", "Google Chrome"])
            .output();

        // wait for force kill to take effect
        for _ in 0..10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            if !is_chrome_running() {
                break;
            }
        }
    }

    if is_chrome_running() {
        return Err(anyhow!("Chrome didn't quit in time"));
    }

    // launch with dedicated debug profile (not user's main profile)
    // using the main profile causes issues with "confirm before quit" dialogs
    // and bot detection on login pages
    println!("[browser] Launching Chrome with debug profile...");
    let user_data_dir = chrome_debug_profile_dir();
    // Launch Chrome binary DIRECTLY instead of via `open -a`
    // `open -a` ignores --args if Chrome was recently running, causing
    // anti-detection flags to not be applied
    let chrome_binary = find_chrome_binary()
        .ok_or_else(|| anyhow!("failed to locate Google Chrome binary"))?;
    std::process::Command::new(chrome_binary)
        .args([
            "--remote-debugging-port=9222",
            &format!("--user-data-dir={}", user_data_dir.to_string_lossy()),
            "--profile-directory=Default",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-blink-features=AutomationControlled",
            "--disable-features=AutomationControlled",
            "--disable-infobars",
            "--disable-background-timer-throttling",
            "--disable-backgrounding-occluded-windows",
            "--disable-renderer-backgrounding",
            "--disable-ipc-flooding-protection",
            "--password-store=basic",
            "--use-mock-keychain",
            "--lang=en-US,en",
        ])
        .spawn()
        .context("failed to launch Chrome")?;

    // wait for debug port to be ready
    for _ in 0..20 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        if let Ok(resp) = reqwest::get("http://127.0.0.1:9222/json/version").await {
            if resp.status().is_success() {
                break;
            }
        }
    }

    // try to connect
    let (mut browser, handler) = Browser::connect("http://127.0.0.1:9222")
        .await
        .context("failed to connect after restart")?;

    println!("[browser] Connected to Chrome with debugging");

    let handler_task = tokio::spawn(async move {
        handler_loop(handler).await;
    });

    // fetch existing targets
    let _ = browser.fetch_targets().await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let pages = browser.pages().await.unwrap_or_default();
    println!("[browser] Found {} pages after restart", pages.len());

    Ok(BrowserClient {
        browser,
        _handler_task: handler_task,
        pages,
        selected_page_idx: 0,
        snapshot_id: 0,
        uid_to_backend_node: HashMap::new(),
    })
}

// try to find existing chrome with debugging enabled
async fn try_find_existing_chrome() -> Option<String> {
    let home = profile_base_dir();

    // check DevToolsActivePort files in known profile locations
    for profile in CHROME_PROFILES {
        let port_file = home.join(profile).join("Default/DevToolsActivePort");

        if let Ok(content) = tokio::fs::read_to_string(&port_file).await {
            let lines: Vec<&str> = content.lines().collect();
            if lines.len() >= 2 {
                let port = lines[0].trim();
                let path = lines[1].trim();
                let ws_url = format!("ws://127.0.0.1:{port}{path}");
                return Some(ws_url);
            }
        }
    }

    // fallback: try localhost:9222
    if reqwest::get("http://127.0.0.1:9222/json/version")
        .await
        .is_ok()
    {
        return Some("http://127.0.0.1:9222".to_string());
    }

    None
}

// launch chrome using chromiumoxide with dedicated debug profile
async fn launch_chrome_with_profile() -> Result<(Browser, Handler)> {
    // chrome requires a NON-DEFAULT user data dir for remote debugging
    // using the default chrome profile path doesn't work - chrome treats it specially
    // so we create a dedicated debug profile that's separate from the user's main profile
    let user_data_dir = chrome_debug_profile_dir();

    println!("[browser] Using debug profile: {:?}", user_data_dir);

    // disable_default_args() skips puppeteer automation flags that break normal browser usage
    // (like --disable-extensions, --disable-sync, --enable-automation, etc.)
    // Anti-detection flags prevent Google from identifying automated Chrome
    let config = BrowserConfig::builder()
        .disable_default_args()
        .with_head()
        .user_data_dir(&user_data_dir)
        .viewport(None)
        // === Anti-Detection Chrome flags ===
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--disable-features=AutomationControlled")
        .arg("--disable-infobars")
        .arg("--disable-background-timer-throttling")
        .arg("--disable-backgrounding-occluded-windows")
        .arg("--disable-renderer-backgrounding")
        .arg("--disable-ipc-flooding-protection")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--password-store=basic")
        .arg("--use-mock-keychain")
        .arg("--lang=en-US,en")
        .build()
        .map_err(|e| anyhow!("failed to build browser config: {}", e))?;

    Browser::launch(config)
        .await
        .context("failed to launch chrome")
}

// format a11y tree to text snapshot
fn format_ax_tree(
    nodes: &[AxNode],
    snapshot_id: u64,
    verbose: bool,
    uid_map: &mut HashMap<String, BackendNodeId>,
) -> String {
    // build parent->children map
    let mut children_map: HashMap<String, Vec<&AxNode>> = HashMap::new();
    let mut node_map: HashMap<String, &AxNode> = HashMap::new();
    let mut root_id: Option<String> = None;

    for node in nodes {
        let id = node.node_id.inner().to_string();
        node_map.insert(id.clone(), node);

        if let Some(ref parent_id) = node.parent_id {
            children_map
                .entry(parent_id.inner().to_string())
                .or_default()
                .push(node);
        } else {
            root_id = Some(id);
        }
    }

    let mut output = String::new();
    let mut node_index = 0u64;

    if let Some(root_id) = root_id {
        if let Some(root) = node_map.get(&root_id) {
            format_node(
                root,
                &children_map,
                &node_map,
                0,
                snapshot_id,
                &mut node_index,
                uid_map,
                verbose,
                None, // no parent name at root
                &mut output,
            );
        }
    }

    output
}

// roles that are pure noise - never useful for interaction or reading
const SKIP_ROLES: &[&str] = &[
    "InlineTextBox",  // individual text fragments - text already in parent
    "LineBreak",      // <br> tags
    "none",           // explicitly hidden
    "presentation",   // decorative only
];

// roles that should be collapsed if they have no name (pass children through)
const COLLAPSE_IF_EMPTY: &[&str] = &[
    "generic",     // divs - only useful if they have a name
    "paragraph",   // usually just wraps text
    "group",       // grouping container
];

// text-like roles where the name IS the content (don't output if it duplicates parent)
const TEXT_ROLES: &[&str] = &["StaticText"];

fn get_node_role(node: &AxNode) -> Option<&str> {
    node.role.as_ref()?.value.as_ref()?.as_str()
}

fn get_node_name(node: &AxNode) -> Option<&str> {
    node.name.as_ref()?.value.as_ref()?.as_str()
}

fn is_focusable(node: &AxNode) -> bool {
    if let Some(ref props) = node.properties {
        for prop in props {
            if matches!(prop.name, AxPropertyName::Focusable) {
                if let Some(ref val) = prop.value.value {
                    return val.as_bool() == Some(true);
                }
            }
        }
    }
    false
}

fn format_node(
    node: &AxNode,
    children_map: &HashMap<String, Vec<&AxNode>>,
    node_map: &HashMap<String, &AxNode>,
    depth: usize,
    snapshot_id: u64,
    node_index: &mut u64,
    uid_map: &mut HashMap<String, BackendNodeId>,
    verbose: bool,
    parent_name: Option<&str>,
    output: &mut String,
) {
    let role = get_node_role(node);
    let name = get_node_name(node);

    // skip ignored nodes unless verbose
    if node.ignored && !verbose {
        process_children(node, children_map, node_map, depth, snapshot_id, node_index, uid_map, verbose, parent_name, output);
        return;
    }

    // skip noise roles entirely (pass children through at same depth)
    if let Some(r) = role {
        if SKIP_ROLES.contains(&r) && !verbose {
            process_children(node, children_map, node_map, depth, snapshot_id, node_index, uid_map, verbose, parent_name, output);
            return;
        }
    }

    // skip StaticText if it just duplicates parent name
    if let Some(r) = role {
        if TEXT_ROLES.contains(&r) && !verbose {
            if let (Some(n), Some(pn)) = (name, parent_name) {
                if n == pn || pn.contains(n) {
                    // text duplicates parent - skip entirely
                    return;
                }
            }
        }
    }

    // collapse empty containers (no name, not focusable) - pass children through
    if let Some(r) = role {
        if COLLAPSE_IF_EMPTY.contains(&r) && !verbose {
            let has_name = name.map(|n| !n.is_empty()).unwrap_or(false);
            if !has_name && !is_focusable(node) {
                process_children(node, children_map, node_map, depth, snapshot_id, node_index, uid_map, verbose, parent_name, output);
                return;
            }
        }
    }

    // node is visible - assign uid and output it
    let uid = format!("{}_{}", snapshot_id, *node_index);
    *node_index += 1;

    // store backend node id mapping
    if let Some(backend_id) = node.backend_dom_node_id {
        uid_map.insert(uid.clone(), backend_id);
    }

    // build attributes
    let indent = "  ".repeat(depth);
    let mut attrs = vec![format!("uid={uid}")];

    // role
    if let Some(r) = role {
        attrs.push(r.to_string());
    }

    // name (truncate if very long, utf-8 safe)
    if let Some(n) = name {
        if !n.is_empty() {
            let display_name = if n.chars().count() > 200 {
                format!("{}...", n.chars().take(200).collect::<String>())
            } else {
                n.to_string()
            };
            attrs.push(format!("\"{}\"", display_name.replace('"', "\\\"")));
        }
    }

    // properties
    if let Some(ref props) = node.properties {
        for prop in props {
            let prop_name = &prop.name;
            if let Some(ref val) = prop.value.value {
                match prop_name {
                    AxPropertyName::Focusable => {
                        if val.as_bool() == Some(true) {
                            attrs.push("focusable".to_string());
                        }
                    }
                    AxPropertyName::Focused => {
                        if val.as_bool() == Some(true) {
                            attrs.push("focused".to_string());
                        }
                    }
                    AxPropertyName::Disabled => {
                        if val.as_bool() == Some(true) {
                            attrs.push("disabled".to_string());
                        }
                    }
                    AxPropertyName::Expanded => {
                        if val.as_bool() == Some(true) {
                            attrs.push("expanded".to_string());
                        }
                    }
                    AxPropertyName::Selected => {
                        if val.as_bool() == Some(true) {
                            attrs.push("selected".to_string());
                        }
                    }
                    AxPropertyName::Checked => {
                        if let Some(s) = val.as_str() {
                            attrs.push(format!("checked={s}"));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    output.push_str(&format!("{}{}\n", indent, attrs.join(" ")));

    // recurse to children, passing current name for deduplication
    process_children(node, children_map, node_map, depth + 1, snapshot_id, node_index, uid_map, verbose, name, output);
}

fn process_children(
    node: &AxNode,
    children_map: &HashMap<String, Vec<&AxNode>>,
    node_map: &HashMap<String, &AxNode>,
    depth: usize,
    snapshot_id: u64,
    node_index: &mut u64,
    uid_map: &mut HashMap<String, BackendNodeId>,
    verbose: bool,
    parent_name: Option<&str>,
    output: &mut String,
) {
    if let Some(child_ids) = &node.child_ids {
        for child_id in child_ids {
            if let Some(child) = node_map.get(child_id.inner()) {
                format_node(
                    child,
                    children_map,
                    node_map,
                    depth,
                    snapshot_id,
                    node_index,
                    uid_map,
                    verbose,
                    parent_name,
                    output,
                );
            }
        }
    }
}

// thread-safe wrapper
pub type SharedBrowserClient = Arc<Mutex<Option<BrowserClient>>>;

pub fn create_shared_browser_client() -> SharedBrowserClient {
    Arc::new(Mutex::new(None))
}
