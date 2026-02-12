#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod agent;
mod api;
mod bash;
mod browser;
mod cognitive;
mod computer;
mod deep_research;
mod panels;
mod permissions;
mod python_tool;
mod rate_limiter;
mod storage;
mod voice;

use agent::{Agent, AgentMode, HistoryMessage};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, State,
};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(HeyWorkPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

struct AppState {
    agent: Arc<Mutex<Agent>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

// cached screen info for fast window positioning
#[cfg(target_os = "macos")]
struct ScreenInfo {
    width: f64,
    height: f64,
    menubar_height: f64,
    scale: f64,
}

#[cfg(target_os = "macos")]
static SCREEN_INFO: std::sync::OnceLock<ScreenInfo> = std::sync::OnceLock::new();

// re-export panel handles from shared module
#[cfg(target_os = "macos")]
use panels::{MAIN_PANEL, VOICE_PANEL, BORDER_PANEL};

/// Nuke every native background layer so the panel is truly invisible.
/// Called AFTER to_panel() on the PanelHandle.
#[cfg(target_os = "macos")]
fn make_panel_transparent(panel: &tauri_nspanel::PanelHandle<tauri::Wry>, label: &str) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, AnyClass, Sel};
    use objc2_app_kit::NSColor;

    let ns_panel = panel.as_panel();
    unsafe {
        // --- NSWindow / NSPanel level ---
        let clear = NSColor::clearColor();
        let _: () = msg_send![ns_panel, setBackgroundColor: &*clear];
        let _: () = msg_send![ns_panel, setOpaque: false];
        let _: () = msg_send![ns_panel, setHasShadow: false];
        let _: () = msg_send![ns_panel, setTitlebarAppearsTransparent: true];
        let _: () = msg_send![ns_panel, setMovable: true];
        let _: () = msg_send![ns_panel, setMovableByWindowBackground: true];
        let _: () = msg_send![ns_panel, setAlphaValue: 1.0f64];

        // Verify our changes stuck
        let is_opaque: bool = msg_send![ns_panel, isOpaque];
        let has_shadow: bool = msg_send![ns_panel, hasShadow];
        println!("[heywork][{}] Panel: opaque={}, shadow={}", label, is_opaque, has_shadow);

        // --- Content view: make layer-backed and transparent ---
        let content_view: *mut AnyObject = msg_send![ns_panel, contentView];
        if !content_view.is_null() {
            let _: () = msg_send![content_view, setWantsLayer: true];
            let layer: *mut AnyObject = msg_send![content_view, layer];
            if !layer.is_null() {
                let _: () = msg_send![layer, setBackgroundColor: std::ptr::null::<AnyObject>()];
                let _: () = msg_send![layer, setOpaque: false];
            }
            // Walk and nuke all subviews
            nuke_view_backgrounds(content_view, label);
        }

        // --- CRITICAL: Swizzle WryWebViewParent.isOpaque -> false ---
        // NSView.isOpaque defaults to true. When the compositor sees isOpaque=true,
        // it treats the view as opaque and may fill its frame with a default background.
        // This swizzle makes WryWebViewParent report non-opaque, allowing transparency.
        // Also swizzle WryWebView for good measure.
        for class_to_fix in &[c"WryWebViewParent", c"WryWebView"] {
            let cls = objc2::ffi::objc_getClass(class_to_fix.as_ptr());
            if !cls.is_null() {
                if let Some(sel) = objc2::ffi::sel_registerName(c"isOpaque".as_ptr()) {
                    let method = objc2::ffi::class_getInstanceMethod(cls.cast(), sel);
                    if !method.is_null() {
                        // Replace isOpaque implementation with one that always returns false
                        unsafe extern "C-unwind" fn is_opaque_false(
                            _self: *mut std::ffi::c_void,
                            _cmd: *mut std::ffi::c_void,
                        ) -> bool {
                            false
                        }
                        let new_imp: unsafe extern "C-unwind" fn() = std::mem::transmute(
                            is_opaque_false as unsafe extern "C-unwind" fn(
                                *mut std::ffi::c_void, *mut std::ffi::c_void
                            ) -> bool
                        );
                        objc2::ffi::method_setImplementation(method, new_imp);
                        println!("[heywork][{}] Swizzled {:?}.isOpaque -> false", label,
                            std::ffi::CStr::from_ptr(class_to_fix.as_ptr()));
                    }
                }
            }
        }

        // Also set the panel's minimum size to 1x1 so macOS doesn't enforce a larger size
        let _: () = msg_send![ns_panel, setContentMinSize: objc2_foundation::NSSize { width: 1.0, height: 1.0 }];

        // Force content view and all children to redraw
        let _: () = msg_send![ns_panel, setViewsNeedDisplay: true];
        let _: () = msg_send![ns_panel, invalidateShadow];
        let _: () = msg_send![ns_panel, display];
    }
    println!("[heywork] Panel '{}' transparency applied", label);
}

/// Recursively walk every view and disable background drawing using MULTIPLE strategies.
/// Strategy 1: KVC setValue:forKey:"drawsBackground" — same approach Wry uses internally.
/// Strategy 2: Direct _setDrawsBackground: method call (WKWebView private API).
/// Strategy 3: setPageBackgroundColor: clearColor (modern WKWebView API).
/// Strategy 4: setUnderPageBackgroundColor: clearColor (WKWebView).
/// Strategy 5: Clear CALayer backgrounds on all views.
#[cfg(target_os = "macos")]
unsafe fn nuke_view_backgrounds(view: *mut objc2::runtime::AnyObject, label: &str) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, AnyClass, Sel};

    if view.is_null() { return; }

    // Get class name for targeted handling
    let class_name_ns: *mut AnyObject = msg_send![view, className];
    let utf8: *const std::ffi::c_char = msg_send![class_name_ns, UTF8String];
    let class_name = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
    // WryWebView IS a WKWebView subclass. WryWebViewParent is NOT — it's just an NSView.
    // Only apply WKWebView-specific strategies to actual WKWebView subclasses.
    let is_wkwebview = class_name == "WryWebView"
        || class_name.contains("WKWebView")
        || (class_name.contains("WebView") && !class_name.contains("Parent"));

    // --- Strategy 1: KVC setValue:forKey:"drawsBackground" with NSNumber(false) ---
    // This is the EXACT approach Wry uses internally for WKWebView transparency.
    // Only safe on WKWebView, NOT on plain NSView (would throw NSUnknownKeyException).
    if is_wkwebview {
        let ns_number_cls = AnyClass::get(c"NSNumber");
        if let Some(cls) = ns_number_cls {
            let cls_ptr = cls as *const AnyClass;
            let ns_false: *mut AnyObject = msg_send![cls_ptr, numberWithBool: false];
            let key = objc2_foundation::NSString::from_str("drawsBackground");
            let _: () = msg_send![view, setValue: ns_false forKey: &*key];
            println!("[heywork][{}] KVC drawsBackground=false on {}", label, class_name);
        }
    }

    // --- Strategy 2: Direct _setDrawsBackground: (WKWebView private API) ---
    let sel = Sel::register(c"_setDrawsBackground:");
    let r: bool = msg_send![view, respondsToSelector: sel];
    if r {
        let _: () = msg_send![view, _setDrawsBackground: false];
        if is_wkwebview {
            println!("[heywork][{}] _setDrawsBackground:false on {}", label, class_name);
        }
    }

    // --- Strategy 3: setPageBackgroundColor: (public WKWebView API, macOS 12+) ---
    if is_wkwebview {
        let sel_page = Sel::register(c"setPageBackgroundColor:");
        let r_page: bool = msg_send![view, respondsToSelector: sel_page];
        if r_page {
            let clear = objc2_app_kit::NSColor::clearColor();
            let _: () = msg_send![view, setPageBackgroundColor: &*clear];
            println!("[heywork][{}] setPageBackgroundColor:clear on {}", label, class_name);
        }
    }

    // --- Strategy 4: setUnderPageBackgroundColor: (WKWebView) ---
    let sel3 = Sel::register(c"setUnderPageBackgroundColor:");
    let r3: bool = msg_send![view, respondsToSelector: sel3];
    if r3 {
        let clear = objc2_app_kit::NSColor::clearColor();
        let _: () = msg_send![view, setUnderPageBackgroundColor: &*clear];
    }

    // setDrawsBackground: NO (for NSScrollView, NSTextView, etc. — safe because we check respondsToSelector)
    let sel2 = Sel::register(c"setDrawsBackground:");
    let r2: bool = msg_send![view, respondsToSelector: sel2];
    if r2 { let _: () = msg_send![view, setDrawsBackground: false]; }

    // setOpaque: NO (for views that expose the property)
    let sel_opaque = Sel::register(c"setOpaque:");
    let r_opaque: bool = msg_send![view, respondsToSelector: sel_opaque];
    if r_opaque { let _: () = msg_send![view, setOpaque: false]; }

    // setBackgroundColor: clearColor (on non-WKWebView views)
    if !is_wkwebview {
        let sel_bg = Sel::register(c"setBackgroundColor:");
        let r_bg: bool = msg_send![view, respondsToSelector: sel_bg];
        if r_bg {
            let clear = objc2_app_kit::NSColor::clearColor();
            let _: () = msg_send![view, setBackgroundColor: &*clear];
        }
    }

    // NSVisualEffectView — hide it completely
    if class_name.contains("VisualEffect") {
        println!("[heywork][{}] Found NSVisualEffectView — hiding!", label);
        let _: () = msg_send![view, setHidden: true];
    }

    // --- Strategy 5: Clear CALayer backgrounds on every view ---
    let _: () = msg_send![view, setWantsLayer: true];
    let layer: *mut AnyObject = msg_send![view, layer];
    if !layer.is_null() {
        let _: () = msg_send![layer, setBackgroundColor: std::ptr::null::<AnyObject>()];
        let _: () = msg_send![layer, setOpaque: false];
    }

    // Recurse into subviews
    let subviews: *mut AnyObject = msg_send![view, subviews];
    if !subviews.is_null() {
        let count: usize = msg_send![subviews, count];
        for i in 0..count {
            let subview: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
            nuke_view_backgrounds(subview, label);
        }
    }
}

#[cfg(target_os = "macos")]
fn get_screen_info() -> &'static ScreenInfo {
    SCREEN_INFO.get_or_init(|| {
        use objc2_app_kit::NSScreen;
        use objc2_foundation::MainThreadMarker;

        if let Some(mtm) = MainThreadMarker::new() {
            if let Some(screen) = NSScreen::mainScreen(mtm) {
                let frame = screen.frame();
                let visible = screen.visibleFrame();
                let menubar_height = frame.size.height - visible.size.height - visible.origin.y;
                let scale = screen.backingScaleFactor();
                return ScreenInfo {
                    width: frame.size.width,
                    height: frame.size.height,
                    menubar_height,
                    scale,
                };
            }
        }
        // fallback for retina mac
        ScreenInfo { width: 1440.0, height: 900.0, menubar_height: 25.0, scale: 2.0 }
    })
}

#[cfg(target_os = "macos")]
fn position_window_top_right(window: &tauri::WebviewWindow, width: f64, _height: f64) {
    let info = get_screen_info();
    let padding = 10.0;
    let x = (info.width - width - padding) * info.scale;
    let y = (info.menubar_height + padding) * info.scale;
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}

#[cfg(target_os = "macos")]
fn position_window_center(window: &tauri::WebviewWindow, width: f64, height: f64) {
    let info = get_screen_info();
    let x = ((info.width - width) / 2.0) * info.scale;
    let y = ((info.height - height) / 2.0) * info.scale;
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}

#[tauri::command]
async fn set_api_key(api_key: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut agent = state.agent.lock().await;
    agent.set_api_key(api_key);
    Ok(())
}

#[tauri::command]
async fn check_api_key(state: State<'_, AppState>) -> Result<bool, String> {
    let agent = state.agent.lock().await;
    Ok(agent.has_api_key())
}

#[tauri::command(rename_all = "camelCase")]
async fn run_agent(
    instructions: String,
    model: String,
    mode: AgentMode,
    voice_mode: Option<bool>,
    history: Vec<HistoryMessage>,
    context_screenshot: Option<String>,
    conversation_id: Option<String>,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let voice = voice_mode.unwrap_or(false);
    println!("[heywork] run_agent called with: {} (model: {}, mode: {:?}, voice: {}, history: {} msgs, screenshot: {}, conv: {:?})",
        instructions, model, mode, voice, history.len(), context_screenshot.is_some(), conversation_id);

    let agent = state.agent.clone();

    {
        let agent_guard = agent.lock().await;
        if agent_guard.is_running() {
            return Err("Agent is already running".to_string());
        }
        if !agent_guard.has_api_key() {
            return Err("No API key set. Please add your Anthropic API key in onboarding or Settings.".to_string());
        }
    }

    tokio::spawn(async move {
        let agent_guard = agent.lock().await;
        match agent_guard.run(instructions, model, mode, voice, history, context_screenshot, conversation_id, app_handle).await {
            Ok(_) => println!("[heywork] Agent finished"),
            Err(e) => println!("[heywork] Agent error: {:?}", e),
        }
    });

    Ok(())
}

#[tauri::command]
fn stop_agent(state: State<'_, AppState>) -> Result<(), String> {
    state.running.store(false, std::sync::atomic::Ordering::SeqCst);
    println!("[heywork] Stop requested");
    Ok(())
}

#[tauri::command]
async fn init_agent_swarm(
    api_key: String,
    model: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut agent = state.agent.lock().await;
    agent.init_agent_swarm(api_key, model, app_handle).await;
    println!("[heywork] Agent Swarm initialized");
    Ok(())
}

#[tauri::command]
fn is_agent_running(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.running.load(std::sync::atomic::Ordering::SeqCst))
}

#[tauri::command]
async fn get_swarm_task_status(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use crate::cognitive::agent_swarm::TaskStatus;
    let agent = state.agent.lock().await;
    let swarm_guard = agent.agent_swarm.lock().await;
    if let Some(ref swarm) = *swarm_guard {
        let status: Option<TaskStatus> = swarm.get_task_status(&task_id).await;
        match status {
            Some(s) => Ok(Some(format!("{:?}", s))),
            None => Ok(None),
        }
    } else {
        Err("Agent Swarm not initialized".to_string())
    }
}

#[tauri::command]
async fn list_active_swarm_tasks(
    state: State<'_, AppState>,
) -> Result<Vec<(String, String)>, String> {
    use crate::cognitive::agent_swarm::TaskStatus;
    let agent = state.agent.lock().await;
    let swarm_guard = agent.agent_swarm.lock().await;
    if let Some(ref swarm) = *swarm_guard {
        let tasks: Vec<(String, TaskStatus)> = swarm.list_active_tasks().await;
        Ok(tasks.into_iter().map(|(id, status)| (id, format!("{:?}", status))).collect())
    } else {
        Err("Agent Swarm not initialized".to_string())
    }
}

#[tauri::command]
async fn export_skills(state: State<'_, AppState>) -> Result<String, String> {
    let agent = state.agent.lock().await;
    let cognitive = agent.cognitive.lock().await;
    cognitive.skills.export_skills()
        .map_err(|e| format!("Failed to export skills: {}", e))
}

#[tauri::command]
async fn import_skills(json: String, state: State<'_, AppState>) -> Result<usize, String> {
    let agent = state.agent.lock().await;
    let mut cognitive = agent.cognitive.lock().await;
    cognitive.skills.import_skills(&json)
        .map_err(|e| format!("Failed to import skills: {}", e))
}

#[tauri::command]
async fn list_skills(state: State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    let agent = state.agent.lock().await;
    let cognitive = agent.cognitive.lock().await;
    let skills = cognitive.skills.list_skills();
    Ok(skills.into_iter().map(|s| serde_json::json!({
        "id": s.id,
        "name": s.name,
        "description": s.description,
        "pattern": {
            "intent_keywords": s.pattern.intent_keywords,
            "app_context": s.pattern.app_context,
        },
        "success_rate": s.success_rate,
        "total_uses": s.total_uses,
    })).collect())
}

#[tauri::command]
async fn confirm_swarm_task(
    task_id: String,
    approved: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    println!("[swarm] User {} task {}", if approved { "approved" } else { "rejected" }, task_id);
    // In a full implementation, this would resume the swarm task
    // For now, we just log the confirmation
    Ok(())
}

#[tauri::command]
fn debug_log(message: String) {
    println!("[frontend] {}", message);
}

// unified window state command - frontend tells backend what size/position it needs
#[tauri::command]
fn set_window_state(app_handle: tauri::AppHandle, width: f64, height: f64, centered: bool) -> Result<(), String> {
    println!("[window] set_window_state: {}x{}, centered={}", width, height, centered);
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
            if centered {
                position_window_center(&window, width, height);
            } else {
                position_window_top_right(&window, width, height);
            }
            if let Some(panel) = MAIN_PANEL.get() {
                panel.show();
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
            if centered {
                let _ = window.center();
            }
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    Ok(())
}

// Move main panel to an absolute screen position (for JS-driven drag)
#[tauri::command]
fn move_panel_to(app_handle: tauri::AppHandle, x: f64, y: f64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        if let Some(panel) = MAIN_PANEL.get() {
            let ns_panel = panel.as_panel();
            unsafe {
                let origin = objc2_foundation::NSPoint { x, y };
                let _: () = msg_send![ns_panel, setFrameOrigin: origin];
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
        }
    }
    Ok(())
}

// voice window controls
#[tauri::command]
fn show_voice_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app_handle.get_webview_window("voice") {
            position_window_center(&window, 300.0, 300.0);
        }
        if let Some(panel) = VOICE_PANEL.get() {
            panel.show();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("voice") {
            let _ = window.center();
            let _ = window.show();
        }
    }
    Ok(())
}

#[tauri::command]
fn hide_voice_window(_app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = VOICE_PANEL.get() {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = _app_handle.get_webview_window("voice") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
fn hide_main_window(_app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = _app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

// show main window in voice response mode and emit event
#[tauri::command]
fn show_main_voice_response(app_handle: tauri::AppHandle, text: String, screenshot: Option<String>, mode: String) -> Result<(), String> {
    // emit event to main window so it can switch to voice response mode
    let _ = app_handle.emit("voice:response", serde_json::json!({
        "text": text,
        "screenshot": screenshot,
        "mode": mode,
    }));

    // show main panel (frontend will handle sizing via set_window_state)
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.show();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
    }

    Ok(())
}

// set main panel click-through (ignores mouse events)
#[tauri::command]
fn set_main_click_through(ignore: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.set_ignores_mouse_events(ignore);
    }
    Ok(())
}

#[tauri::command]
fn show_border_overlay(app_handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    if let Some(panel) = BORDER_PANEL.get() {
        panel.show();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window("border") {
        let _ = window.show();
    }
}

#[tauri::command]
fn hide_border_overlay(app_handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    if let Some(panel) = BORDER_PANEL.get() {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window("border") {
        let _ = window.hide();
    }
}

// take screenshot excluding our app windows - uses shared panels module
#[tauri::command]
fn take_screenshot_excluding_app() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        panels::take_screenshot_excluding_app()
    }

    #[cfg(not(target_os = "macos"))]
    {
        let control = computer::ComputerControl::new().map_err(|e| e.to_string())?;
        control.take_screenshot().map_err(|e| e.to_string())
    }
}

// trigger screen flash effect - plays sound as feedback
#[cfg(target_os = "macos")]
fn trigger_screen_flash() {
    std::process::Command::new("afplay")
        .arg("/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds/system/Grab.aif")
        .spawn()
        .ok();
}

#[cfg(not(target_os = "macos"))]
fn capture_screenshot_fallback() -> Option<String> {
    match computer::ComputerControl::new() {
        Ok(control) => control.take_screenshot().ok(),
        Err(_) => None,
    }
}

// hotkey triggered - capture screenshot and return base64
#[tauri::command]
fn capture_screen_for_help() -> Result<String, String> {
    let control = computer::ComputerControl::new().map_err(|e| e.to_string())?;
    let screenshot = control.take_screenshot().map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    trigger_screen_flash();

    Ok(screenshot)
}

// --- storage IPC commands ---

mod storage_cmd {
    use crate::storage::{self, Conversation, ConversationMeta};

    #[tauri::command]
    pub fn list_conversations(limit: usize, offset: usize) -> Result<Vec<ConversationMeta>, String> {
        storage::list_conversations(limit, offset)
    }

    #[tauri::command]
    pub fn load_conversation(id: String) -> Result<Option<Conversation>, String> {
        storage::load_conversation(&id)
    }

    #[tauri::command]
    pub fn create_conversation(title: String, model: String, mode: String) -> Result<String, String> {
        storage::create_conversation(title, model, mode)
    }

    #[tauri::command]
    pub fn save_conversation(conv: Conversation) -> Result<(), String> {
        storage::save_conversation(&conv)
    }

    #[tauri::command]
    pub fn delete_conversation(id: String) -> Result<(), String> {
        storage::delete_conversation(&id)
    }

    #[tauri::command]
    pub fn search_conversations(query: String, limit: usize) -> Result<Vec<ConversationMeta>, String> {
        storage::search_conversations(&query, limit)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn set_conversation_voice_mode(conversation_id: String, voice_mode: bool) -> Result<(), String> {
        storage::set_conversation_voice_mode(&conversation_id, voice_mode)
    }
}

// --- voice IPC commands ---

mod voice_cmd {
    use crate::voice::{VoiceSession, PushToTalkSession};
    #[cfg(target_os = "macos")]
    use crate::get_screen_info;
    #[cfg(target_os = "macos")]
    use crate::panels;
    use std::sync::Arc;
    use tauri::{State, Emitter};
    #[cfg(target_os = "macos")]
    use tauri::Manager;

    pub struct VoiceState {
        pub session: Arc<VoiceSession>,
    }

    pub struct PttState {
        pub session: Arc<PushToTalkSession>,
        pub screenshot: std::sync::Mutex<Option<String>>,
        pub mode: std::sync::Mutex<Option<String>>,
        pub current_session_id: std::sync::Mutex<u64>,
    }

    #[cfg(not(target_os = "macos"))]
    fn capture_screenshot_fallback() -> Option<String> {
        match crate::computer::ComputerControl::new() {
            Ok(control) => control.take_screenshot().ok(),
            Err(_) => None,
        }
    }

    #[tauri::command]
    pub async fn start_voice(
        app_handle: tauri::AppHandle,
        state: State<'_, VoiceState>,
    ) -> Result<(), String> {
        println!("[voice cmd] start_voice called");
        let api_key = match std::env::var("DEEPGRAM_API_KEY") {
            Ok(key) => {
                println!("[voice cmd] got API key (len={})", key.len());
                key
            }
            Err(e) => {
                println!("[voice cmd] DEEPGRAM_API_KEY not found: {:?}", e);
                return Err("DEEPGRAM_API_KEY not set in .env".to_string());
            }
        };
        println!("[voice cmd] starting session...");
        let result = state.session.start(api_key, app_handle).await;
        println!("[voice cmd] session.start returned: {:?}", result);
        result
    }

    #[tauri::command]
    pub fn stop_voice(state: State<'_, VoiceState>) -> Result<(), String> {
        state.session.stop();
        Ok(())
    }

    #[tauri::command]
    pub fn is_voice_running(state: State<'_, VoiceState>) -> Result<bool, String> {
        Ok(state.session.is_running())
    }

    #[tauri::command]
    pub async fn start_ptt(
        app_handle: tauri::AppHandle,
        state: State<'_, PttState>,
        mode: Option<String>,
    ) -> Result<(), String> {
        println!("[ptt cmd] start_ptt called");

        let mode_str = mode.unwrap_or_else(|| "computer".to_string());

        // capture screenshot only for computer mode (like hotkey does)
        let screenshot = if mode_str == "computer" {
            #[cfg(target_os = "macos")]
            {
                panels::take_screenshot_excluding_app_sync().ok()
            }
            #[cfg(not(target_os = "macos"))]
            {
                capture_screenshot_fallback()
            }
        } else {
            None
        };

        // store screenshot and mode
        if let Some(ss) = &screenshot {
            *state.screenshot.lock().unwrap() = Some(ss.clone());
        }
        *state.mode.lock().unwrap() = Some(mode_str.clone());

        // play recording start sound
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("afplay")
                .arg("/System/Library/Sounds/Tink.aiff")
                .spawn()
                .ok();
        }

        // show voice window centered - must run on main thread
        #[cfg(target_os = "macos")]
        {
            use crate::panels::VOICE_PANEL;
            use dispatch::Queue;
            let app_clone = app_handle.clone();
            Queue::main().exec_sync(move || {
                if let Some(window) = app_clone.get_webview_window("voice") {
                    let _ = window.set_size(tauri::LogicalSize::new(300.0, 300.0));
                    let info = get_screen_info();
                    let x = ((info.width - 300.0) / 2.0) * info.scale;
                    let y = ((info.height - 300.0) / 2.0) * info.scale;
                    let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
                }
                if let Some(panel) = VOICE_PANEL.get() {
                    panel.show();
                }
            });
        }

        // emit recording event
        let _ = app_handle.emit("ptt:recording", serde_json::json!({
            "recording": true,
            "screenshot": screenshot,
            "mode": mode_str,
            "sessionId": 0
        }));

        let api_key = std::env::var("DEEPGRAM_API_KEY")
            .map_err(|_| "DEEPGRAM_API_KEY not set in .env".to_string())?;

        let session_id = state.session.start(api_key, app_handle).await?;
        *state.current_session_id.lock().unwrap() = session_id;
        Ok(())
    }

    #[tauri::command]
    pub async fn stop_ptt(
        app_handle: tauri::AppHandle,
        state: State<'_, PttState>,
    ) -> Result<(), String> {
        println!("[ptt cmd] stop_ptt called");

        // play stop sound
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("afplay")
                .arg("/System/Library/Sounds/Pop.aiff")
                .spawn()
                .ok();
        }

        let expected_session_id = *state.current_session_id.lock().unwrap();
        let (raw_text, result_session_id) = state.session.stop().await;
        let screenshot = state.screenshot.lock().unwrap().take();
        let mode = state.mode.lock().unwrap().take();

        if result_session_id != expected_session_id {
            println!("[ptt cmd] stale result ignored: got session {} but expected {}", result_session_id, expected_session_id);
            return Ok(());
        }

        println!("[ptt cmd] result: text='{}', screenshot={}, mode={:?}, session={}", raw_text, screenshot.is_some(), mode, result_session_id);

        // emit recording stopped
        let _ = app_handle.emit("ptt:recording", serde_json::json!({
            "recording": false,
            "sessionId": result_session_id
        }));

        // emit result - frontend handles voice window visibility
        let _ = app_handle.emit("ptt:result", serde_json::json!({
            "text": raw_text,
            "screenshot": screenshot,
            "mode": mode,
            "sessionId": result_session_id
        }));

        Ok(())
    }

    #[tauri::command]
    pub fn is_ptt_running(state: State<'_, PttState>) -> Result<bool, String> {
        Ok(state.session.is_running())
    }
}

fn main() {
    // load .env
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_filename("../.env");
    }

    // init storage
    if let Err(e) = storage::init_db() {
        eprintln!("[heywork] storage init failed: {}", e);
    }

    let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut agent = Agent::new(running.clone());

    if let Some(key) = permissions::load_api_key_for_service("anthropic")
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
    {
        println!("[heywork] API key loaded");
        agent.set_api_key(key);
    }

    let running_for_shortcut = running.clone();
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyH))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyS))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyQ))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyC))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyB))
                .unwrap()
                .with_handler(move |app, shortcut, event| {
                    // PTT shortcuts - Ctrl+Shift+C (computer), Ctrl+Shift+B (browser)
                    let ptt_mode: Option<&str> = if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyC) {
                        Some("computer")
                    } else if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyB) {
                        Some("browser")
                    } else {
                        None
                    };

                    if let Some(mode) = ptt_mode {
                        match event.state {
                            ShortcutState::Pressed => {
                                println!("[ptt] pressed - starting recording (mode: {})", mode);

                                // capture screenshot only for computer mode
                                let screenshot = if mode == "computer" {
                                    #[cfg(target_os = "macos")]
                                    {
                                        panels::take_screenshot_excluding_app_sync().ok()
                                    }
                                    #[cfg(not(target_os = "macos"))]
                                    {
                                        capture_screenshot_fallback()
                                    }
                                } else {
                                    None
                                };

                                // play recording start sound
                                #[cfg(target_os = "macos")]
                                {
                                    std::process::Command::new("afplay")
                                        .arg("/System/Library/Sounds/Tink.aiff")
                                        .spawn()
                                        .ok();
                                }

                                // show voice window centered at 300x300
                            #[cfg(target_os = "macos")]
                            {
                                if let Some(window) = app.get_webview_window("voice") {
                                    let _ = window.set_size(tauri::LogicalSize::new(300.0, 300.0));
                                    let info = get_screen_info();
                                    let x = ((info.width - 300.0) / 2.0) * info.scale;
                                    let y = ((info.height - 300.0) / 2.0) * info.scale;
                                    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
                                }
                                if let Some(panel) = VOICE_PANEL.get() {
                                    panel.show();
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                if let Some(window) = app.get_webview_window("voice") {
                                    let _ = window.set_size(tauri::LogicalSize::new(300.0, 300.0));
                                    let _ = window.center();
                                    let _ = window.show();
                                }
                            }

                            // emit recording event with screenshot and mode
                            let _ = app.emit("ptt:recording", serde_json::json!({
                                "recording": true,
                                "screenshot": screenshot,
                                "mode": mode,
                                "sessionId": 0
                            }));

                                // start PTT recording via command
                                let app_clone = app.clone();
                                let screenshot_clone = screenshot.clone();
                                let mode_str = mode.to_string();
                                tauri::async_runtime::spawn(async move {
                                    if let Some(ptt_state) = app_clone.try_state::<voice_cmd::PttState>() {
                                        let api_key = match std::env::var("DEEPGRAM_API_KEY") {
                                            Ok(k) => k,
                                            Err(_) => {
                                                let _ = app_clone.emit("ptt:error", "DEEPGRAM_API_KEY not set");
                                                return;
                                            }
                                        };

                                        // store screenshot and mode
                                        if let Some(ss) = screenshot_clone {
                                            *ptt_state.screenshot.lock().unwrap() = Some(ss);
                                        }
                                        *ptt_state.mode.lock().unwrap() = Some(mode_str);

                                        match ptt_state.session.start(api_key, app_clone.clone()).await {
                                            Ok(session_id) => {
                                                *ptt_state.current_session_id.lock().unwrap() = session_id;
                                                // session started - first ptt:recording already emitted with mode
                                            }
                                            Err(e) => {
                                                println!("[ptt] start error: {}", e);
                                                let _ = app_clone.emit("ptt:error", e);
                                            }
                                        }
                                    }
                                });
                            }
                            ShortcutState::Released => {
                                println!("[ptt] released - stopping recording");

                                // play recording stop sound
                                #[cfg(target_os = "macos")]
                                {
                                    std::process::Command::new("afplay")
                                        .arg("/System/Library/Sounds/Pop.aiff")
                                        .spawn()
                                        .ok();
                                }

                                // stop recording and get result
                                let app_clone = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    if let Some(ptt_state) = app_clone.try_state::<voice_cmd::PttState>() {
                                        let expected_session_id = *ptt_state.current_session_id.lock().unwrap();
                                        let (raw_text, result_session_id) = ptt_state.session.stop().await;
                                        let screenshot = ptt_state.screenshot.lock().unwrap().take();
                                        let mode = ptt_state.mode.lock().unwrap().take();

                                        if result_session_id != expected_session_id {
                                            println!("[ptt] stale result ignored: got session {} but expected {}", result_session_id, expected_session_id);
                                            return;
                                        }

                                        println!("[ptt] result: text='{}', screenshot={}, mode={:?}, session={}", raw_text, screenshot.is_some(), mode, result_session_id);

                                        let _ = app_clone.emit("ptt:recording", serde_json::json!({
                                            "recording": false,
                                            "sessionId": result_session_id
                                        }));

                                        let _ = app_clone.emit("ptt:result", serde_json::json!({
                                            "text": raw_text,
                                            "screenshot": screenshot,
                                            "mode": mode,
                                            "sessionId": result_session_id
                                        }));
                                    }
                                });
                            }
                        }
                        return;
                    }

                    // other shortcuts only on press
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    // Cmd+Shift+H - help mode (screenshot + prompt)
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyH) {
                        let screenshot = {
                            #[cfg(target_os = "macos")]
                            {
                                panels::take_screenshot_excluding_app_sync().ok()
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                capture_screenshot_fallback()
                            }
                        };

                        #[cfg(target_os = "macos")]
                        trigger_screen_flash();

                        let _ = app.emit("hotkey-help", serde_json::json!({ "screenshot": screenshot }));
                    }

                    // Cmd+Shift+Space - spotlight mode (show centered input)
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::Space) {
                        println!("[heywork] Spotlight mode triggered");
                        let _ = app.emit("hotkey-spotlight", ());

                        #[cfg(target_os = "macos")]
                        if let Some(panel) = MAIN_PANEL.get() {
                            panel.show();
                            // make panel key window so input receives focus
                            let ns_panel = panel.as_panel();
                            unsafe {
                                let _: () = objc2::msg_send![ns_panel, makeKeyAndOrderFront: std::ptr::null::<objc2::runtime::AnyObject>()];
                            }
                        }

                        #[cfg(not(target_os = "macos"))]
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }

                    // Cmd+Shift+S - stop agent
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyS) {
                        if running_for_shortcut.load(std::sync::atomic::Ordering::SeqCst) {
                            running_for_shortcut.store(false, std::sync::atomic::Ordering::SeqCst);
                            println!("[heywork] Stop requested via shortcut");
                        }
                    }

                    // Cmd+Shift+Q - quit app
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyQ) {
                        println!("[heywork] Quit requested via shortcut");
                        app.exit(0);
                    }
                })
                .build(),
        );

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .manage(AppState {
            agent: Arc::new(Mutex::new(agent)),
            running,
        })
        .manage(voice_cmd::VoiceState {
            session: Arc::new(voice::VoiceSession::new()),
        })
        .manage(voice_cmd::PttState {
            session: Arc::new(voice::PushToTalkSession::new()),
            screenshot: std::sync::Mutex::new(None),
            mode: std::sync::Mutex::new(None),
            current_session_id: std::sync::Mutex::new(0),
        })
        .setup(|app| {
            // hide from dock - menubar app only
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            {
                // main panel
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_position(PhysicalPosition::new(-1000, -1000));

                    match window.to_panel::<HeyWorkPanel>() {
                        Ok(panel) => {
                            println!("[heywork] Main window converted to panel successfully");
                            panel.set_level(PanelLevel::Floating.value());
                            panel.set_style_mask(
                                StyleMask::empty()
                                    .borderless()
                                    .nonactivating_panel()
                                    .into(),
                            );
                            panel.set_collection_behavior(
                                CollectionBehavior::new()
                                    .full_screen_auxiliary()
                                    .can_join_all_spaces()
                                    .stationary()
                                    .into(),
                            );
                            panel.set_hides_on_deactivate(false);
                            make_panel_transparent(&panel, "main");
                            let _ = MAIN_PANEL.set(panel);
                        }
                        Err(e) => {
                            eprintln!("[heywork] ERROR: Failed to convert main window to panel: {:?}", e);
                            // Fallback: show window directly without panel
                            let _ = window.set_size(tauri::LogicalSize::new(52.0, 52.0));
                            position_window_top_right(&window, 52.0, 52.0);
                            let _ = window.set_always_on_top(true);
                            let _ = window.show();
                        }
                    }
                } else {
                    eprintln!("[heywork] ERROR: Could not find main webview window");
                }

                // voice panel
                if let Some(window) = app.get_webview_window("voice") {
                    let _ = window.set_position(PhysicalPosition::new(-1000, -1000));

                    match window.to_panel::<HeyWorkPanel>() {
                        Ok(panel) => {
                            println!("[heywork] Voice window converted to panel successfully");
                            panel.set_level(PanelLevel::Floating.value());
                            panel.set_style_mask(
                                StyleMask::empty()
                                    .borderless()
                                    .nonactivating_panel()
                                    .into(),
                            );
                            panel.set_collection_behavior(
                                CollectionBehavior::new()
                                    .full_screen_auxiliary()
                                    .can_join_all_spaces()
                                    .stationary()
                                    .into(),
                            );
                            panel.set_hides_on_deactivate(false);
                            make_panel_transparent(&panel, "voice");
                            let _ = VOICE_PANEL.set(panel);
                        }
                        Err(e) => {
                            eprintln!("[heywork] ERROR: Failed to convert voice window to panel: {:?}", e);
                        }
                    }
                }

                // border panel
                if let Some(window) = app.get_webview_window("border") {
                    let info = get_screen_info();
                    let _ = window.set_size(tauri::LogicalSize::new(info.width, info.height));
                    let _ = window.set_position(PhysicalPosition::new(0, 0));
                    println!("[heywork] Border panel sized to {}x{}", info.width, info.height);

                    match window.to_panel::<HeyWorkPanel>() {
                        Ok(panel) => {
                            println!("[heywork] Border window converted to panel successfully");
                            panel.set_level(PanelLevel::Floating.value());
                            panel.set_style_mask(
                                StyleMask::empty()
                                    .borderless()
                                    .nonactivating_panel()
                                    .into(),
                            );
                            panel.set_collection_behavior(
                                CollectionBehavior::new()
                                    .full_screen_auxiliary()
                                    .can_join_all_spaces()
                                    .stationary()
                                    .into(),
                            );
                            panel.set_hides_on_deactivate(false);
                            panel.set_ignores_mouse_events(true);
                            make_panel_transparent(&panel, "border");
                            let _ = BORDER_PANEL.set(panel);
                        }
                        Err(e) => {
                            eprintln!("[heywork] ERROR: Failed to convert border window to panel: {:?}", e);
                        }
                    }
                }

                // show main window at startup (idle size)
                if let Some(window) = app.get_webview_window("main") {
                    println!("[heywork] Positioning main window at top-right (idle: 52x52)");
                    let _ = window.set_size(tauri::LogicalSize::new(52.0, 52.0));
                    position_window_top_right(&window, 52.0, 52.0);
                    if let Some(panel) = MAIN_PANEL.get() {
                        panel.show();
                        println!("[heywork] Main panel shown via panel.show()");
                    }
                }

                // Delayed re-apply transparency at multiple intervals.
                // The WKWebView may reset drawsBackground after initial load,
                // so we hit it multiple times to be sure.
                let css_injection = r#"(function(){
                    var s = document.getElementById('heywork-force-transparent');
                    if (!s) {
                        s = document.createElement('style');
                        s.id = 'heywork-force-transparent';
                        s.textContent = 'html, body, #root { background: transparent !important; background-color: transparent !important; } html::before, html::after, body::before, body::after { display: none !important; }';
                        (document.head || document.documentElement).appendChild(s);
                    }
                    document.documentElement.style.setProperty('background', 'transparent', 'important');
                    document.body.style.setProperty('background', 'transparent', 'important');
                    var root = document.getElementById('root');
                    if (root) root.style.setProperty('background', 'transparent', 'important');
                })();"#;

                // Apply at 500ms, 1500ms, and 3000ms
                for delay_ms in [500u64, 1500, 3000] {
                    let app_handle = app.handle().clone();
                    let css_js = css_injection.to_string();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                        // Re-nuke native backgrounds
                        if let Some(panel) = MAIN_PANEL.get() {
                            make_panel_transparent(panel, &format!("main-{}ms", delay_ms));
                        }
                        if delay_ms == 500 {
                            if let Some(panel) = VOICE_PANEL.get() {
                                make_panel_transparent(panel, "voice-delayed");
                            }
                        }
                        // Inject aggressive CSS into all webviews
                        for label in &["main", "voice", "border"] {
                            if let Some(w) = app_handle.get_webview_window(label) {
                                let _ = w.eval(&css_js);
                            }
                        }
                        println!("[heywork] Delayed transparency pass at {}ms complete", delay_ms);
                    });
                }
            }

            // ── Windows / Linux: ensure main window is visible at startup ──
            // The Windows config (tauri.windows.conf.json) creates the window with
            // visible:true, transparent:false, skipTaskbar:false.
            // This block ensures the window is centered and focused on startup.
            #[cfg(not(target_os = "macos"))]
            {
                if let Some(window) = app.get_webview_window("main") {
                    println!("[heywork] Windows: Initializing main window");
                    let _ = window.set_skip_taskbar(false);
                    let _ = window.center();
                    let _ = window.show();
                    let _ = window.set_focus();
                    println!("[heywork] Windows: Main window shown and focused");
                } else {
                    eprintln!("[heywork] ERROR: Could not find main window on startup!");
                }
            }

            // tray menu with show + quit options
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(false)
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            let _ = app.emit("tray:show", ());
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();

                        #[cfg(target_os = "macos")]
                        {
                            let main_visible = MAIN_PANEL.get().map(|p| p.is_visible()).unwrap_or(false);

                            if main_visible {
                                // hide main
                                if let Some(panel) = MAIN_PANEL.get() {
                                    panel.hide();
                                }
                            } else {
                                // show main at idle size and emit event so React resets to idle
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.set_size(tauri::LogicalSize::new(52.0, 52.0));
                                    position_window_top_right(&window, 52.0, 52.0);
                                }
                                if let Some(panel) = MAIN_PANEL.get() {
                                    panel.show();
                                }
                                let _ = app.emit("tray:show", ());
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // emit focus lost event for main window (spotlight dismiss)
            if window.label() == "main" {
                if let tauri::WindowEvent::Focused(false) = event {
                    let _ = window.emit("window:blur", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            set_api_key,
            check_api_key,
            run_agent,
            stop_agent,
            init_agent_swarm,
            get_swarm_task_status,
            list_active_swarm_tasks,
            export_skills,
            import_skills,
            list_skills,
            confirm_swarm_task,
            is_agent_running,
            debug_log,
            set_window_state,
            show_voice_window,
            hide_voice_window,
            hide_main_window,
            show_main_voice_response,
            move_panel_to,
            set_main_click_through,
            show_border_overlay,
            hide_border_overlay,
            take_screenshot_excluding_app,
            capture_screen_for_help,
            storage_cmd::list_conversations,
            storage_cmd::load_conversation,
            storage_cmd::create_conversation,
            storage_cmd::save_conversation,
            storage_cmd::delete_conversation,
            storage_cmd::search_conversations,
            storage_cmd::set_conversation_voice_mode,
            voice_cmd::start_voice,
            voice_cmd::stop_voice,
            voice_cmd::is_voice_running,
            voice_cmd::start_ptt,
            voice_cmd::stop_ptt,
            voice_cmd::is_ptt_running,
            permissions::check_permissions,
            permissions::request_permission,
            permissions::open_permission_settings,
            permissions::get_browser_profile_status,
            permissions::open_browser_profile,
            permissions::open_browser_profile_url,
            permissions::clear_domain_cookies,
            permissions::reset_browser_profile,
            permissions::get_api_key_status,
            permissions::save_api_key,
            permissions::get_voice_settings,
            permissions::save_voice_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
