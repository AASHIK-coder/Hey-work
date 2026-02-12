//! Skill System - Learned Reusable Action Patterns
//! 
//! Skills are reusable patterns of actions that the agent learns from
//! successful task executions. They enable the agent to handle similar
//! tasks more efficiently over time.

use super::{ActionTemplate, ActionType, Skill, Subtask, Task, TaskPattern, TaskResult};
use super::skill_executor::{SkillExecutor, SkillExecutionResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Library of learned skills
pub struct SkillLibrary {
    /// All learned skills
    skills: Vec<Skill>,
    /// Index by intent keywords
    intent_index: HashMap<String, Vec<String>>, // keyword -> skill_ids
    /// Index by app context
    app_index: HashMap<String, Vec<String>>, // app -> skill_ids
    /// Predefined skills loaded at startup
    predefined_skills: Vec<Skill>,
}

/// Skill creation from successful execution
#[derive(Debug, Clone)]
pub struct SkillCandidate {
    pub name: String,
    pub description: String,
    pub pattern: TaskPattern,
    pub actions: Vec<ActionTemplate>,
    pub source_task: String,
}

/// Configuration for skill learning
#[derive(Debug, Clone)]
pub struct LearningConfig {
    pub min_success_rate: f32,
    pub min_usage_count: u32,
    pub max_skills: usize,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            min_success_rate: 0.7,
            min_usage_count: 2,
            max_skills: 1000,
        }
    }
}

impl SkillLibrary {
    pub fn new() -> Self {
        let mut library = Self {
            skills: Vec::new(),
            intent_index: HashMap::new(),
            app_index: HashMap::new(),
            predefined_skills: Vec::new(),
        };
        
        library.load_predefined_skills();
        library
    }

    /// Load predefined skills for common tasks
    fn load_predefined_skills(&mut self) {
        let predefined = vec![
            Skill {
                id: "skill_open_chrome".to_string(),
                name: "Open Chrome Browser".to_string(),
                description: "Launches Google Chrome and waits for it to be ready".to_string(),
                pattern: TaskPattern {
                    intent_keywords: vec!["open".to_string(), "chrome".to_string(), "browser".to_string(), "google".to_string()],
                    app_context: Some("system".to_string()),
                    required_elements: vec![],
                },
                actions: vec![
                    ActionTemplate {
                        action_type: ActionType::Bash {
                            command: if cfg!(target_os = "windows") {
                                r#"start chrome"#.to_string()
                            } else {
                                r#"open -a "Google Chrome""#.to_string()
                            },
                        },
                        condition: None,
                        fallback: Some(Box::new(ActionTemplate {
                            action_type: ActionType::Bash {
                                command: if cfg!(target_os = "windows") {
                                    r#"start "" "C:\Program Files\Google\Chrome\Application\chrome.exe""#.to_string()
                                } else {
                                    "/Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome &".to_string()
                                },
                            },
                            condition: None,
                            fallback: None,
                        })),
                    },
                    ActionTemplate {
                        action_type: ActionType::Wait { duration_ms: 2000 },
                        condition: None,
                        fallback: None,
                    },
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "screenshot".to_string(),
                            params: serde_json::json!({}),
                        },
                        condition: None,
                        fallback: None,
                    },
                ],
                success_rate: 1.0,
                total_uses: 0,
                avg_execution_time_ms: 3000,
            },
            
            Skill {
                id: "skill_screenshot".to_string(),
                name: "Take Screenshot".to_string(),
                description: "Captures the current screen state".to_string(),
                pattern: TaskPattern {
                    intent_keywords: vec!["screenshot".to_string(), "capture".to_string(), "screen".to_string(), "see".to_string(), "look".to_string()],
                    app_context: None,
                    required_elements: vec![],
                },
                actions: vec![
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "screenshot".to_string(),
                            params: serde_json::json!({}),
                        },
                        condition: None,
                        fallback: None,
                    },
                ],
                success_rate: 0.99,
                total_uses: 0,
                avg_execution_time_ms: 500,
            },
            
            Skill {
                id: "skill_search_spotlight".to_string(),
                name: "Search with Spotlight".to_string(),
                description: "Opens Spotlight search and searches for an item".to_string(),
                pattern: TaskPattern {
                    intent_keywords: vec!["search".to_string(), "spotlight".to_string(), "find".to_string(), "open".to_string()],
                    app_context: Some("system".to_string()),
                    required_elements: vec!["query".to_string()],
                },
                actions: vec![
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "key".to_string(),
                            params: serde_json::json!({"text": "command+space"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                    ActionTemplate {
                        action_type: ActionType::Wait { duration_ms: 500 },
                        condition: None,
                        fallback: None,
                    },
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "type".to_string(),
                            params: serde_json::json!({"text": "{query}"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                    ActionTemplate {
                        action_type: ActionType::Wait { duration_ms: 300 },
                        condition: None,
                        fallback: None,
                    },
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "key".to_string(),
                            params: serde_json::json!({"text": "return"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                ],
                success_rate: 0.95,
                total_uses: 0,
                avg_execution_time_ms: 2000,
            },
            
            Skill {
                id: "skill_copy_paste".to_string(),
                name: "Copy and Paste".to_string(),
                description: "Selects all text and copies it to clipboard".to_string(),
                pattern: TaskPattern {
                    intent_keywords: vec!["copy".to_string(), "paste".to_string(), "clipboard".to_string(), "select".to_string(), "all".to_string()],
                    app_context: None,
                    required_elements: vec![],
                },
                actions: vec![
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "key".to_string(),
                            params: serde_json::json!({"text": "command+a"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "key".to_string(),
                            params: serde_json::json!({"text": "command+c"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                ],
                success_rate: 0.98,
                total_uses: 0,
                avg_execution_time_ms: 300,
            },
            
            Skill {
                id: "skill_new_tab_chrome".to_string(),
                name: "New Tab in Chrome".to_string(),
                description: "Opens a new tab in Chrome browser".to_string(),
                pattern: TaskPattern {
                    intent_keywords: vec!["new".to_string(), "tab".to_string(), "chrome".to_string(), "browser".to_string()],
                    app_context: Some("chrome".to_string()),
                    required_elements: vec![],
                },
                actions: vec![
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "key".to_string(),
                            params: serde_json::json!({"text": "command+t"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                ],
                success_rate: 0.99,
                total_uses: 0,
                avg_execution_time_ms: 200,
            },
            
            Skill {
                id: "skill_type_url".to_string(),
                name: "Type URL and Navigate".to_string(),
                description: "Clicks address bar, types URL, and presses enter".to_string(),
                pattern: TaskPattern {
                    intent_keywords: vec!["go".to_string(), "to".to_string(), "navigate".to_string(), "url".to_string(), "website".to_string()],
                    app_context: Some("chrome".to_string()),
                    required_elements: vec!["url".to_string()],
                },
                actions: vec![
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "click".to_string(),
                            params: serde_json::json!({"coordinate": [640, 60]}), // Address bar area
                        },
                        condition: None,
                        fallback: Some(Box::new(ActionTemplate {
                            action_type: ActionType::Computer {
                                action: "key".to_string(),
                                params: serde_json::json!({"text": "command+l"}),
                            },
                            condition: None,
                            fallback: None,
                        })),
                    },
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "type".to_string(),
                            params: serde_json::json!({"text": "{url}"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                    ActionTemplate {
                        action_type: ActionType::Computer {
                            action: "key".to_string(),
                            params: serde_json::json!({"text": "return"}),
                        },
                        condition: None,
                        fallback: None,
                    },
                ],
                success_rate: 0.92,
                total_uses: 0,
                avg_execution_time_ms: 1500,
            },
        ];
        
        for skill in predefined {
            self.index_skill(&skill);
            self.predefined_skills.push(skill);
        }
        
        println!("[skills] Loaded {} predefined skills", self.predefined_skills.len());
    }

    /// Find skills matching the given intent
    pub async fn find_matching_skills(&self, intent: &str) -> anyhow::Result<Vec<Skill>> {
        let intent_lower = intent.to_lowercase();
        let keywords: Vec<&str> = intent_lower.split_whitespace().collect();
        
        let mut scored_skills: Vec<(Skill, f32)> = Vec::new();
        
        // Check predefined skills first
        for skill in &self.predefined_skills {
            let score = self.calculate_match_score(skill, &keywords, &intent_lower);
            if score > 0.3 {
                scored_skills.push((skill.clone(), score));
            }
        }
        
        // Check learned skills
        for skill in &self.skills {
            let score = self.calculate_match_score(skill, &keywords, &intent_lower);
            if score > 0.3 {
                scored_skills.push((skill.clone(), score));
            }
        }
        
        // Sort by score descending
        scored_skills.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Return top matches
        Ok(scored_skills.into_iter().take(3).map(|(s, _)| s).collect())
    }

    /// Calculate how well a skill matches the intent
    fn calculate_match_score(&self, skill: &Skill, keywords: &[&str], intent: &str) -> f32 {
        let mut score = 0.0;
        
        // Keyword matching
        let pattern_keywords = &skill.pattern.intent_keywords;
        let mut keyword_matches = 0;
        for kw in keywords {
            if pattern_keywords.iter().any(|pk| pk.to_lowercase().contains(kw)) {
                keyword_matches += 1;
            }
        }
        
        if !keywords.is_empty() {
            score += (keyword_matches as f32 / keywords.len() as f32) * 0.5;
        }
        
        // Name/description match
        if skill.name.to_lowercase().contains(intent) || 
           skill.description.to_lowercase().contains(intent) {
            score += 0.3;
        }
        
        // Success rate weighting
        score += skill.success_rate * 0.2;
        
        score
    }

    /// Get a skill for a specific subtask
    pub fn get_skill_for_subtask(&self, subtask: &Subtask) -> Option<Skill> {
        // Check if any skill's pattern matches this subtask
        let description_lower = subtask.description.to_lowercase();
        
        // Check predefined first
        for skill in &self.predefined_skills {
            if self.skill_matches_subtask(skill, &description_lower) {
                return Some(skill.clone());
            }
        }
        
        // Check learned skills
        for skill in &self.skills {
            if self.skill_matches_subtask(skill, &description_lower) {
                return Some(skill.clone());
            }
        }
        
        None
    }

    fn skill_matches_subtask(&self, skill: &Skill, description: &str) -> bool {
        // Check if skill name or keywords match
        if skill.name.to_lowercase().contains(description) {
            return true;
        }
        
        for keyword in &skill.pattern.intent_keywords {
            if description.contains(&keyword.to_lowercase()) {
                return true;
            }
        }
        
        false
    }

    /// Learn a new skill from successful execution
    pub async fn learn_from_execution(
        &mut self,
        task: &Task,
        subtask: &Subtask,
        result: &TaskResult,
    ) -> anyhow::Result<()> {
        // Check if this execution is worth learning from
        if !result.success {
            return Ok(());
        }
        
        // Check if similar skill already exists
        let exists = self.skills.iter().any(|s| {
            s.pattern.intent_keywords.iter().any(|k| {
                subtask.description.to_lowercase().contains(&k.to_lowercase())
            })
        });
        
        if exists {
            // Update existing skill
            self.update_existing_skill(&subtask.description, result).await?;
        } else {
            // Create new skill candidate
            let candidate = self.create_skill_candidate(task, subtask, result).await?;
            
            // Validate and add if good enough
            if self.validate_skill_candidate(&candidate) {
                let skill = self.candidate_to_skill(candidate);
                self.add_skill(skill).await?;
            }
        }
        
        Ok(())
    }

    async fn update_existing_skill(&mut self, description: &str, result: &TaskResult) -> anyhow::Result<()> {
        for skill in &mut self.skills {
            if skill.pattern.intent_keywords.iter().any(|k| description.to_lowercase().contains(&k.to_lowercase())) {
                skill.total_uses += 1;
                
                // Update success rate
                let alpha = 0.2;
                let new_success = if result.success { 1.0 } else { 0.0 };
                skill.success_rate = skill.success_rate * (1.0 - alpha) + new_success * alpha;
                
                // Update avg execution time
                skill.avg_execution_time_ms = 
                    (skill.avg_execution_time_ms * (skill.total_uses as u64 - 1) + result.duration_ms) 
                    / skill.total_uses as u64;
                
                println!(
                    "[skills] Updated skill {}: uses={}, success_rate={:.2}",
                    skill.name, skill.total_uses, skill.success_rate
                );
                break;
            }
        }
        Ok(())
    }

    async fn create_skill_candidate(
        &self,
        task: &Task,
        subtask: &Subtask,
        result: &TaskResult,
    ) -> anyhow::Result<SkillCandidate> {
        let keywords = self.extract_keywords(&subtask.description);
        
        Ok(SkillCandidate {
            name: subtask.description.clone(),
            description: format!("Learned skill from task: {}", task.description),
            pattern: TaskPattern {
                intent_keywords: keywords,
                app_context: task.context.app_state.get("current_app").map(|v| v.as_str().unwrap_or("").to_string()),
                required_elements: vec![],
            },
            actions: vec![ActionTemplate {
                action_type: subtask.action_type.clone(),
                condition: None,
                fallback: None,
            }],
            source_task: task.id.clone(),
        })
    }

    fn validate_skill_candidate(&self, candidate: &SkillCandidate) -> bool {
        // Must have meaningful keywords
        if candidate.pattern.intent_keywords.len() < 2 {
            return false;
        }
        
        // Must have actions
        if candidate.actions.is_empty() {
            return false;
        }
        
        true
    }

    fn candidate_to_skill(&self, candidate: SkillCandidate) -> Skill {
        Skill {
            id: format!("skill_learned_{}", Uuid::new_v4().to_string()[..8].to_string()),
            name: candidate.name,
            description: candidate.description,
            pattern: candidate.pattern,
            actions: candidate.actions,
            success_rate: 0.8, // Initial confidence
            total_uses: 1,
            avg_execution_time_ms: 0,
        }
    }

    async fn add_skill(&mut self, skill: Skill) -> anyhow::Result<()> {
        println!("[skills] Learned new skill: {}", skill.name);
        
        // Persist to storage first
        self.persist_skill(&skill).await?;
        
        self.index_skill(&skill);
        self.skills.push(skill);
        
        Ok(())
    }

    fn index_skill(&mut self, skill: &Skill) {
        // Index by keywords
        for keyword in &skill.pattern.intent_keywords {
            self.intent_index
                .entry(keyword.clone())
                .or_default()
                .push(skill.id.clone());
        }
        
        // Index by app context
        if let Some(ref app) = skill.pattern.app_context {
            self.app_index
                .entry(app.clone())
                .or_default()
                .push(skill.id.clone());
        }
    }

    fn extract_keywords(&self, text: &str) -> Vec<String> {
        let text_lower = text.to_lowercase();
        let stop_words: std::collections::HashSet<&str> = [
            "the", "a", "an", "is", "are", "was", "were", "to", "of", "in",
            "for", "on", "with", "and", "or", "if", "then", "else",
        ]
        .iter()
        .cloned()
        .collect();
        
        text_lower
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty() && w.len() > 2 && !stop_words.contains(w))
            .map(|w| w.to_string())
            .take(5)
            .collect()
    }

    async fn persist_skill(&self, skill: &Skill) -> anyhow::Result<()> {
        // Save to SQLite skills database
        let db_path = {
            let app_dir = dirs::data_local_dir()
                .or_else(dirs::data_dir)
                .unwrap_or_else(std::env::temp_dir)
                .join("hey-work");
            let _ = std::fs::create_dir_all(&app_dir);
            app_dir.join("skills.db")
        };
        
        match rusqlite::Connection::open(&db_path) {
            Ok(conn) => {
                conn.execute(
                    "CREATE TABLE IF NOT EXISTS learned_skills (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL,
                        description TEXT,
                        pattern_json TEXT,
                        actions_json TEXT,
                        success_rate REAL,
                        total_uses INTEGER,
                        created_at TEXT,
                        updated_at TEXT
                    )",
                    [],
                ).map_err(|e| anyhow::anyhow!("Failed to create skills table: {}", e))?;
                
                let pattern_json = serde_json::to_string(&skill.pattern).unwrap_or_default();
                let actions_json = serde_json::to_string(&skill.actions).unwrap_or_default();
                let now = chrono::Utc::now().to_rfc3339();
                
                conn.execute(
                    "INSERT OR REPLACE INTO learned_skills (id, name, description, pattern_json, actions_json, success_rate, total_uses, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        skill.id, skill.name, skill.description,
                        pattern_json, actions_json,
                        skill.success_rate as f64, skill.total_uses as i64,
                        now, now
                    ],
                ).map_err(|e| anyhow::anyhow!("Failed to persist skill: {}", e))?;
                
                println!("[skills] Persisted skill to database: {} ({})", skill.name, skill.id);
            }
            Err(e) => {
                println!("[skills] Failed to open skills database: {}", e);
            }
        }
        
        Ok(())
    }

    /// Get skill statistics
    pub fn get_stats(&self) -> SkillStats {
        let total_learned = self.skills.len();
        let total_predefined = self.predefined_skills.len();
        
        SkillStats {
            total_learned,
            total_predefined,
            total_skills: total_learned + total_predefined,
            avg_success_rate: if self.skills.is_empty() {
                0.0
            } else {
                self.skills.iter().map(|s| s.success_rate).sum::<f32>() / self.skills.len() as f32
            },
        }
    }
    
    /// Export all learned skills to JSON
    pub fn export_skills(&self) -> anyhow::Result<String> {
        let export_data = SkillExport {
            version: "1.0".to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            skills: self.skills.clone(),
        };
        
        let json = serde_json::to_string_pretty(&export_data)?;
        Ok(json)
    }
    
    /// Import skills from JSON
    pub fn import_skills(&mut self, json: &str) -> anyhow::Result<usize> {
        let export_data: SkillExport = serde_json::from_str(json)?;
        
        let mut imported = 0;
        for skill in export_data.skills {
            // Skip if skill with same ID already exists
            if self.skills.iter().any(|s| s.id == skill.id) {
                continue;
            }
            
            // Add skill
            self.index_skill(&skill);
            self.skills.push(skill);
            imported += 1;
        }
        
        println!("[skills] Imported {} skills", imported);
        Ok(imported)
    }
    
    /// Get all skills for display
    pub fn get_all_skills(&self) -> &[Skill] {
        &self.skills
    }
    
    /// List all skills (both predefined and learned) for UI display
    pub fn list_skills(&self) -> Vec<Skill> {
        let mut all_skills = Vec::with_capacity(self.predefined_skills.len() + self.skills.len());
        all_skills.extend(self.predefined_skills.clone());
        all_skills.extend(self.skills.clone());
        all_skills
    }
    
    /// Delete a skill by ID
    pub fn delete_skill(&mut self, skill_id: &str) -> bool {
        let before = self.skills.len();
        self.skills.retain(|s| s.id != skill_id);
        self.skills.len() < before
    }

    /// Execute a skill with real tools
    pub async fn execute_skill(
        &self,
        skill: &Skill,
        params: &HashMap<String, String>,
    ) -> anyhow::Result<SkillExecutionResult> {
        let executor = SkillExecutor::new();
        executor.execute_skill(skill, params).await
    }

    /// Find and execute a skill by matching against a request
    pub async fn try_execute_matching_skill(
        &self,
        request: &str,
    ) -> Option<(Skill, SkillExecutionResult)> {
        // Find matching skills
        let intent_lower = request.to_lowercase();
        let keywords: Vec<&str> = intent_lower.split_whitespace().collect();
        
        // Check predefined skills first
        for skill in &self.predefined_skills {
            let score = self.calculate_match_score(skill, &keywords, &intent_lower);
            if score > 0.7 {
                println!("[skills] High confidence match: {} (score: {:.2})", skill.name, score);
                
                // Extract parameters from request
                let params = self.extract_params_from_request(request, skill);
                
                // Execute the skill
                match self.execute_skill(skill, &params).await {
                    Ok(result) => return Some((skill.clone(), result)),
                    Err(e) => {
                        println!("[skills] Skill execution failed: {}", e);
                        continue;
                    }
                }
            }
        }
        
        None
    }

    /// Extract parameters from request based on skill pattern
    fn extract_params_from_request(&self, request: &str, skill: &Skill) -> HashMap<String, String> {
        let mut params = HashMap::new();
        let request_lower = request.to_lowercase();
        
        // Extract app name for open_app pattern
        if skill.id == "skill_open_chrome" || skill.pattern.intent_keywords.contains(&"open".to_string()) {
            // Try to extract app name after "open" or "launch"
            for prefix in ["open ", "launch ", "start "] {
                if let Some(pos) = request_lower.find(prefix) {
                    let after = &request[pos + prefix.len()..];
                    let app_name = after.split_whitespace().next().unwrap_or("");
                    if !app_name.is_empty() {
                        params.insert("app".to_string(), app_name.to_string());
                        break;
                    }
                }
            }
        }
        
        // Extract URL for navigation
        if skill.id == "skill_type_url" {
            // Look for URL patterns
            for word in request.split_whitespace() {
                if word.contains(".") && (word.contains("http") || word.contains("www") || word.contains(".com") || word.contains(".org")) {
                    params.insert("url".to_string(), word.to_string());
                    break;
                }
            }
        }
        
        // Extract query for spotlight
        if skill.id == "skill_search_spotlight" {
            for prefix in ["search for ", "find ", "spotlight "] {
                if let Some(pos) = request_lower.find(prefix) {
                    let query = &request[pos + prefix.len()..];
                    if !query.is_empty() {
                        params.insert("query".to_string(), query.to_string());
                        break;
                    }
                }
            }
        }
        
        params
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SkillExport {
    version: String,
    exported_at: String,
    skills: Vec<Skill>,
}

#[derive(Debug)]
pub struct SkillStats {
    pub total_learned: usize,
    pub total_predefined: usize,
    pub total_skills: usize,
    pub avg_success_rate: f32,
}

impl Default for SkillLibrary {
    fn default() -> Self {
        Self::new()
    }
}