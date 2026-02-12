//! Context Manager - Dynamic Context and State Tracking
//! 
//! Manages the current application state, user preferences, and
//! provides relevant context for decision making.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};

// Placeholder type for async traits
pub type AnyhowResult<T> = anyhow::Result<T>;

/// Manages context for the cognitive engine
pub struct ContextManager {
    /// Current application state
    current_state: Arc<Mutex<AppState>>,
    /// User preference cache
    preferences: Arc<Mutex<HashMap<String, Preference>>>,
    /// Session history
    session: Arc<Mutex<Session>>,
    /// Screen state cache
    screen_state: Arc<Mutex<ScreenState>>,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub current_app: Option<String>,
    pub previous_app: Option<String>,
    pub open_apps: Vec<String>,
    pub system_state: SystemState,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemState {
    pub clipboard_content: Option<String>,
    pub active_window_title: Option<String>,
    pub screen_resolution: (u32, u32),
    pub is_dark_mode: bool,
    pub volume_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub key: String,
    pub value: String,
    pub confidence: f32,
    pub learned_from: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub start_time: DateTime<Utc>,
    pub tasks_completed: u32,
    pub tasks_failed: u32,
    pub total_actions: u32,
    pub avg_task_duration_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ScreenState {
    pub last_screenshot: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub detected_elements: Vec<UIElement>,
    pub text_content: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UIElement {
    pub element_type: ElementType,
    pub label: Option<String>,
    pub coordinates: (i32, i32),
    pub size: (u32, u32),
    pub is_interactive: bool,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElementType {
    Button,
    TextField,
    Link,
    Image,
    Menu,
    Window,
    Dialog,
    Unknown,
}

/// Context snapshot for a specific point in time
#[derive(Debug, Clone)]
pub struct ContextSnapshot {
    pub app_state: AppState,
    pub preferences: HashMap<String, String>,
    pub recent_actions: Vec<String>,
    pub screen_summary: String,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            current_state: Arc::new(Mutex::new(AppState::default())),
            preferences: Arc::new(Mutex::new(HashMap::new())),
            session: Arc::new(Mutex::new(Session {
                start_time: Utc::now(),
                ..Default::default()
            })),
            screen_state: Arc::new(Mutex::new(ScreenState::default())),
        }
    }

    /// Update current application
    pub fn update_current_app(&self, app_name: &str) {
        let mut state = self.current_state.lock().unwrap();
        state.previous_app = state.current_app.clone();
        state.current_app = Some(app_name.to_string());
        state.last_updated = Utc::now();
        
        if !state.open_apps.contains(&app_name.to_string()) {
            state.open_apps.push(app_name.to_string());
        }
        
        println!("[context] App changed: {:?} -> {}", state.previous_app, app_name);
    }

    /// Get current application
    pub fn get_current_app(&self) -> Option<String> {
        let state = self.current_state.lock().unwrap();
        state.current_app.clone()
    }

    /// Record an app being opened
    pub fn record_app_opened(&self, app_name: &str) {
        let mut state = self.current_state.lock().unwrap();
        if !state.open_apps.contains(&app_name.to_string()) {
            state.open_apps.push(app_name.to_string());
        }
    }

    /// Record an app being closed
    pub fn record_app_closed(&self, app_name: &str) {
        let mut state = self.current_state.lock().unwrap();
        state.open_apps.retain(|a| a != app_name);
        
        if state.current_app.as_ref() == Some(&app_name.to_string()) {
            state.current_app = None;
        }
    }

    /// Update system state
    pub fn update_system_state(&self, update: impl FnOnce(&mut SystemState)) {
        let mut state = self.current_state.lock().unwrap();
        update(&mut state.system_state);
        state.last_updated = Utc::now();
    }

    /// Get clipboard content
    pub fn get_clipboard(&self) -> Option<String> {
        let state = self.current_state.lock().unwrap();
        state.system_state.clipboard_content.clone()
    }

    /// Set clipboard content
    pub fn set_clipboard(&self, content: &str) {
        let mut state = self.current_state.lock().unwrap();
        state.system_state.clipboard_content = Some(content.to_string());
    }

    /// Learn or update a preference
    pub fn learn_preference(&self, key: &str, value: &str, confidence: f32, source: &str) {
        let mut prefs = self.preferences.lock().unwrap();
        
        let pref = Preference {
            key: key.to_string(),
            value: value.to_string(),
            confidence,
            learned_from: source.to_string(),
            timestamp: Utc::now(),
        };
        
        // Only update if confidence is higher or significantly newer
        if let Some(existing) = prefs.get(key) {
            if confidence > existing.confidence || 
               (Utc::now() - existing.timestamp).num_days() > 7 {
                prefs.insert(key.to_string(), pref);
                println!("[context] Updated preference: {} = {}", key, value);
            }
        } else {
            prefs.insert(key.to_string(), pref);
            println!("[context] Learned preference: {} = {}", key, value);
        }
    }

    /// Get a preference
    pub fn get_preference(&self, key: &str) -> Option<String> {
        let prefs = self.preferences.lock().unwrap();
        prefs.get(key).map(|p| p.value.clone())
    }

    /// Get all preferences
    pub fn get_all_preferences(&self) -> HashMap<String, String> {
        let prefs = self.preferences.lock().unwrap();
        prefs.iter().map(|(k, v)| (k.clone(), v.value.clone())).collect()
    }

    /// Update screen state from analysis
    pub fn update_screen_state(&self, screenshot: Option<String>, elements: Vec<UIElement>, text: Vec<String>) {
        let mut state = self.screen_state.lock().unwrap();
        state.last_screenshot = screenshot;
        state.detected_elements = elements;
        state.text_content = text;
        state.timestamp = Utc::now();
    }

    /// Get current screen elements
    pub fn get_screen_elements(&self) -> Vec<UIElement> {
        let state = self.screen_state.lock().unwrap();
        state.detected_elements.clone()
    }

    /// Find element by type and optional label
    pub fn find_element(&self, element_type: ElementType, label: Option<&str>) -> Option<UIElement> {
        let state = self.screen_state.lock().unwrap();
        
        state.detected_elements.iter().find(|e| {
            let type_matches = std::mem::discriminant(&e.element_type) == std::mem::discriminant(&element_type);
            let label_matches = match label {
                Some(l) => e.label.as_ref().map_or(false, |el| el.contains(l)),
                None => true,
            };
            type_matches && label_matches
        }).cloned()
    }

    /// Record task completion
    pub fn record_task_completed(&self, duration_ms: u64) {
        let mut session = self.session.lock().unwrap();
        session.tasks_completed += 1;
        session.total_actions += 1;
        
        // Update average duration
        let total = session.tasks_completed as u64;
        session.avg_task_duration_ms = 
            (session.avg_task_duration_ms * (total - 1) + duration_ms) / total;
    }

    /// Record task failure
    pub fn record_task_failed(&self) {
        let mut session = self.session.lock().unwrap();
        session.tasks_failed += 1;
    }

    /// Record action execution
    pub fn record_action(&self) {
        let mut session = self.session.lock().unwrap();
        session.total_actions += 1;
    }

    /// Get session statistics
    pub fn get_session_stats(&self) -> SessionStats {
        let session = self.session.lock().unwrap();
        let state = self.current_state.lock().unwrap();
        
        SessionStats {
            duration_minutes: (Utc::now() - session.start_time).num_minutes() as u32,
            tasks_completed: session.tasks_completed,
            tasks_failed: session.tasks_failed,
            success_rate: if session.tasks_completed + session.tasks_failed > 0 {
                session.tasks_completed as f32 / (session.tasks_completed + session.tasks_failed) as f32
            } else {
                0.0
            },
            total_actions: session.total_actions,
            open_apps: state.open_apps.clone(),
            current_app: state.current_app.clone(),
        }
    }

    /// Create a context snapshot
    pub fn create_snapshot(&self) -> ContextSnapshot {
        let state = self.current_state.lock().unwrap();
        let prefs = self.preferences.lock().unwrap();
        
        ContextSnapshot {
            app_state: state.clone(),
            preferences: prefs.iter().map(|(k, v)| (k.clone(), v.value.clone())).collect(),
            recent_actions: Vec::new(), // Would be populated from action history
            screen_summary: self.generate_screen_summary(),
        }
    }

    fn generate_screen_summary(&self) -> String {
        let screen = self.screen_state.lock().unwrap();
        let state = self.current_state.lock().unwrap();
        
        let mut summary = String::new();
        
        if let Some(ref app) = state.current_app {
            summary.push_str(&format!("Current app: {}. ", app));
        }
        
        summary.push_str(&format!("Screen has {} elements. ", screen.detected_elements.len()));
        
        let interactive = screen.detected_elements.iter().filter(|e| e.is_interactive).count();
        summary.push_str(&format!("{} interactive elements. ", interactive));
        
        if !screen.text_content.is_empty() {
            summary.push_str(&format!("Detected text: {} snippets. ", screen.text_content.len()));
        }
        
        summary
    }

    /// Get context for making decisions
    pub fn get_decision_context(&self) -> DecisionContext {
        let state = self.current_state.lock().unwrap();
        let prefs = self.preferences.lock().unwrap();
        let session = self.session.lock().unwrap();
        
        DecisionContext {
            current_app: state.current_app.clone(),
            open_apps: state.open_apps.clone(),
            relevant_preferences: prefs.iter()
                .filter(|(_, p)| p.confidence > 0.7)
                .map(|(k, v)| (k.clone(), v.value.clone()))
                .collect(),
            session_duration_minutes: (Utc::now() - session.start_time).num_minutes() as u32,
            recent_success_rate: if session.tasks_completed + session.tasks_failed > 0 {
                session.tasks_completed as f32 / (session.tasks_completed + session.tasks_failed) as f32
            } else {
            0.0
            },
        }
    }

    /// Clear session data (for privacy)
    pub fn clear_session(&self) {
        let mut session = self.session.lock().unwrap();
        *session = Session {
            start_time: Utc::now(),
            ..Default::default()
        };
        
        let mut state = self.current_state.lock().unwrap();
        state.system_state.clipboard_content = None;
    }
}

#[derive(Debug, Clone)]
pub struct SessionStats {
    pub duration_minutes: u32,
    pub tasks_completed: u32,
    pub tasks_failed: u32,
    pub success_rate: f32,
    pub total_actions: u32,
    pub open_apps: Vec<String>,
    pub current_app: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub current_app: Option<String>,
    pub open_apps: Vec<String>,
    pub relevant_preferences: HashMap<String, String>,
    pub session_duration_minutes: u32,
    pub recent_success_rate: f32,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}