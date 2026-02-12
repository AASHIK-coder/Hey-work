//! Planner Module - Task Decomposition and Dependency Management
//! 
//! Breaks down high-level user requests into executable subtasks,
//! manages dependencies, and creates execution plans.

use super::{ActionType, Subtask, SubtaskStatus, Task, TaskContext, TaskStatus};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Request analysis result
#[derive(Debug, Clone)]
pub struct RequestAnalysis {
    pub intent: String,
    pub entities: Vec<String>,
    pub complexity: TaskComplexity,
    pub estimated_steps: u32,
    pub app_context: Option<String>,
    pub constraints: Vec<String>,
}

impl RequestAnalysis {
    /// Create from TaskAnalysis (from reasoner)
    pub fn from_task_analysis(analysis: &super::reasoner::TaskAnalysis) -> Self {
        Self {
            intent: analysis.intent.clone(),
            entities: analysis.entities.iter().map(|e| e.name.clone()).collect(),
            complexity: analysis.complexity.clone(),
            estimated_steps: analysis.estimated_steps,
            app_context: analysis.app_context.clone(),
            constraints: analysis.constraints.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskComplexity {
    Simple,     // Single action (e.g., "open Chrome")
    Moderate,   // Few steps (e.g., "search for X on Google")
    Complex,    // Multi-step (e.g., "book a flight to Paris")
    VeryComplex, // Long workflow (e.g., "create a presentation from this data")
}

/// Execution plan with DAG structure
pub struct ExecutionPlan {
    pub root_task: String,
    pub subtasks: HashMap<String, SubtaskNode>,
    pub execution_order: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SubtaskNode {
    pub subtask: Subtask,
    pub children: Vec<String>,
    pub parents: Vec<String>,
}

pub struct Planner {
    /// Templates for common task patterns
    task_templates: HashMap<String, TaskTemplate>,
}

#[derive(Debug, Clone)]
struct TaskTemplate {
    pattern: String,
    subtask_generators: Vec<SubtaskGenerator>,
}

#[derive(Debug, Clone)]
struct SubtaskGenerator {
    description_template: String,
    action_type: ActionTypeTemplate,
    dependencies: Vec<usize>, // Indices of prerequisite subtasks
}

#[derive(Debug, Clone)]
enum ActionTypeTemplate {
    Computer { action: String },
    Browser { tool: String },
    Bash { command_template: String },
}

impl Planner {
    pub fn new() -> Self {
        let mut planner = Self {
            task_templates: HashMap::new(),
        };
        planner.load_default_templates();
        planner
    }

    fn load_default_templates(&mut self) {
        // Template: Open an application
        self.task_templates.insert(
            "open_app".to_string(),
            TaskTemplate {
                pattern: "open {app}".to_string(),
                subtask_generators: vec![
                    SubtaskGenerator {
                        description_template: "Open {app} application".to_string(),
                        action_type: ActionTypeTemplate::Bash {
                            command_template: if cfg!(target_os = "windows") {
                                "start \"\" \"{app}\"".to_string()
                            } else {
                                "open -a \"{app}\"".to_string()
                            },
                        },
                        dependencies: vec![],
                    },
                    SubtaskGenerator {
                        description_template: "Wait for {app} to load".to_string(),
                        action_type: ActionTypeTemplate::Computer {
                            action: "wait".to_string(),
                        },
                        dependencies: vec![0],
                    },
                    SubtaskGenerator {
                        description_template: "Verify {app} is open".to_string(),
                        action_type: ActionTypeTemplate::Computer {
                            action: "screenshot".to_string(),
                        },
                        dependencies: vec![1],
                    },
                ],
            },
        );

        // Template: Search on web
        self.task_templates.insert(
            "web_search".to_string(),
            TaskTemplate {
                pattern: "search {query}".to_string(),
                subtask_generators: vec![
                    SubtaskGenerator {
                        description_template: "Open browser".to_string(),
                        action_type: ActionTypeTemplate::Bash {
                            command_template: if cfg!(target_os = "windows") {
                                "start chrome".to_string()
                            } else {
                                "open -a \"Google Chrome\"".to_string()
                            },
                        },
                        dependencies: vec![],
                    },
                    SubtaskGenerator {
                        description_template: "Click on address bar".to_string(),
                        action_type: ActionTypeTemplate::Browser {
                            tool: "click".to_string(),
                        },
                        dependencies: vec![0],
                    },
                    SubtaskGenerator {
                        description_template: "Type search query".to_string(),
                        action_type: ActionTypeTemplate::Browser {
                            tool: "type".to_string(),
                        },
                        dependencies: vec![1],
                    },
                    SubtaskGenerator {
                        description_template: "Press Enter to search".to_string(),
                        action_type: ActionTypeTemplate::Browser {
                            tool: "press_key".to_string(),
                        },
                        dependencies: vec![2],
                    },
                ],
            },
        );

        // Template: Find and open file
        self.task_templates.insert(
            "find_file".to_string(),
            TaskTemplate {
                pattern: "find {filename}".to_string(),
                subtask_generators: vec![
                    SubtaskGenerator {
                        description_template: "Search for file using mdfind".to_string(),
                        action_type: ActionTypeTemplate::Bash {
                            command_template: "mdfind -name \"{filename}\" | head -5".to_string(),
                        },
                        dependencies: vec![],
                    },
                    SubtaskGenerator {
                        description_template: "Open the found file".to_string(),
                        action_type: ActionTypeTemplate::Bash {
                            command_template: "open \"{found_path}\"".to_string(),
                        },
                        dependencies: vec![0],
                    },
                ],
            },
        );
    }

    /// Create a comprehensive plan for the user request
    pub async fn create_plan(
        &self,
        request: &str,
        analysis: &RequestAnalysis,
        context: &TaskContext,
    ) -> anyhow::Result<Task> {
        let task_id = Uuid::new_v4().to_string();
        
        // Try to match a template first
        let subtasks = if let Some(template) = self.match_template(request) {
            self.generate_from_template(&template, request, analysis)
        } else {
            // Use AI-powered planning for novel tasks
            self.ai_powered_planning(request, analysis, context).await?
        };

        let task = Task {
            id: task_id,
            description: request.to_string(),
            goal: analysis.intent.clone(),
            subtasks,
            context: context.clone(),
            status: TaskStatus::Planning,
            created_at: chrono::Utc::now(),
        };

        Ok(task)
    }

    fn match_template(&self, request: &str) -> Option<&TaskTemplate> {
        let request_lower = request.to_lowercase();
        
        for (_name, template) in &self.task_templates {
            let pattern_lower = template.pattern.to_lowercase();
            
            // Simple pattern matching
            if pattern_lower.contains("{app}") {
                // Check if it's an "open app" type request
                if request_lower.starts_with("open ") || request_lower.starts_with("launch ") {
                    return Some(template);
                }
            }
            
            if pattern_lower.contains("{query}") {
                if request_lower.contains("search") || request_lower.contains("find") {
                    return Some(template);
                }
            }
            
            if pattern_lower.contains("{filename}") {
                if request_lower.contains("file") || request_lower.contains("document") {
                    return Some(template);
                }
            }
        }
        
        None
    }

    fn generate_from_template(
        &self,
        template: &TaskTemplate,
        request: &str,
        analysis: &RequestAnalysis,
    ) -> Vec<Subtask> {
        let mut subtasks = Vec::new();
        let mut param_map = HashMap::new();
        
        // Extract parameters from request
        let request_lower = request.to_lowercase();
        if request_lower.starts_with("open ") {
            let app = request[5..].trim();
            param_map.insert("app", app);
        }
        
        for (idx, generator) in template.subtask_generators.iter().enumerate() {
            let description = self.fill_template(&generator.description_template, &param_map);
            let action = self.action_from_template(&generator.action_type, &param_map);
            
            let subtask = Subtask {
                id: format!("{}-{}", Uuid::new_v4(), idx),
                description,
                action_type: action,
                dependencies: generator.dependencies.iter().map(|i| format!("{}-{}", Uuid::new_v4(), i)).collect(),
                status: SubtaskStatus::Pending,
                retry_count: 0,
                max_retries: 3,
                result: None,
            };
            subtasks.push(subtask);
        }
        
        subtasks
    }

    fn fill_template(&self, template: &str, params: &HashMap<&str, &str>) -> String {
        let mut result = template.to_string();
        for (key, value) in params {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }

    fn action_from_template(
        &self,
        template: &ActionTypeTemplate,
        params: &HashMap<&str, &str>,
    ) -> ActionType {
        match template {
            ActionTypeTemplate::Computer { action } => ActionType::Computer {
                action: action.clone(),
                params: serde_json::json!({}),
            },
            ActionTypeTemplate::Browser { tool } => ActionType::Browser {
                tool: tool.clone(),
                params: serde_json::json!({}),
            },
            ActionTypeTemplate::Bash { command_template } => {
                let command = self.fill_template(command_template, params);
                ActionType::Bash { command }
            }
        }
    }

    /// Intelligent planning with real executable actions
    /// Creates context-aware subtask plans with actual commands
    async fn ai_powered_planning(
        &self,
        request: &str,
        _analysis: &RequestAnalysis,
        context: &TaskContext,
    ) -> anyhow::Result<Vec<Subtask>> {
        let mut subtasks = Vec::new();
        let request_lower = request.to_lowercase();
        
        // Log memory context if available
        if !context.relevant_memories.is_empty() {
            let memory_info = context.relevant_memories.iter()
                .map(|m| format!("\"{}\" ({:.0}%)", m.task_pattern, m.success_rate * 100.0))
                .collect::<Vec<_>>()
                .join(", ");
            println!("[planner] Memory context: {}", memory_info);
        }
        
        // Detect task category
        let is_document_task = request_lower.contains("document") || request_lower.contains("report") 
            || request_lower.contains("spreadsheet") || request_lower.contains("presentation")
            || request_lower.contains("pdf") || request_lower.contains("pptx")
            || request_lower.contains("excel") || request_lower.contains("word");
        
        let is_web_task = request_lower.contains("search") || request_lower.contains("website")
            || request_lower.contains("browse") || request_lower.contains("download")
            || request_lower.contains("google") || request_lower.contains("url");
        
        let is_file_task = request_lower.contains("file") || request_lower.contains("folder")
            || request_lower.contains("move") || request_lower.contains("copy")
            || request_lower.contains("rename") || request_lower.contains("delete");
        
        let is_app_task = request_lower.contains("open") || request_lower.contains("launch")
            || request_lower.contains("start") || request_lower.contains("close")
            || request_lower.contains("quit");
        
        if is_document_task {
            // Step 1: Screenshot to see context
            subtasks.push(self.make_subtask(
                "Capture current screen state",
                ActionType::Computer { action: "screenshot".to_string(), params: serde_json::json!({}) },
                vec![], 1,
            ));
            
            // Step 2: Use Python tool to create the document (delegated to LLM executor)
            subtasks.push(self.make_subtask(
                &format!("Use Python to create document: {}", request),
                ActionType::Think { reasoning: format!(
                    "Create this document using Python tool. Use create_professional_report, create_presentation, or create_spreadsheet helper. Task: {}", 
                    request
                )},
                vec![subtasks[0].id.clone()], 3,
            ));
            
            // Step 3: Verify file exists  
            subtasks.push(self.make_subtask(
                "Verify the document was created",
                ActionType::Bash { command: if cfg!(target_os = "windows") {
                    "dir %USERPROFILE%\\Desktop\\*.docx %USERPROFILE%\\Desktop\\*.xlsx %USERPROFILE%\\Desktop\\*.pdf 2>nul".to_string()
                } else {
                    "ls -la ~/Desktop/*.{docx,xlsx,pdf,pptx,html} 2>/dev/null | tail -5".to_string()
                } },
                vec![subtasks[1].id.clone()], 2,
            ));
        } else if is_web_task {
            // Step 1: Open Chrome
            subtasks.push(self.make_subtask(
                "Launch Google Chrome",
                ActionType::Bash { command: if cfg!(target_os = "windows") {
                    "start chrome".to_string()
                } else {
                    r#"open -a "Google Chrome""#.to_string()
                } },
                vec![], 2,
            ));
            
            // Step 2: Wait for Chrome
            subtasks.push(self.make_subtask(
                "Wait for Chrome to be ready",
                ActionType::Wait { duration_ms: 2000 },
                vec![subtasks[0].id.clone()], 1,
            ));
            
            // Step 3: Screenshot to see browser state
            subtasks.push(self.make_subtask(
                "Take screenshot to see browser state",
                ActionType::Computer { action: "screenshot".to_string(), params: serde_json::json!({}) },
                vec![subtasks[1].id.clone()], 2,
            ));
            
            // Step 4: Navigate/interact (delegated to LLM with screenshot context)
            subtasks.push(self.make_subtask(
                &format!("Complete web task: {}", request),
                ActionType::Think { reasoning: format!("Use browser to: {}", request) },
                vec![subtasks[2].id.clone()], 3,
            ));
            
            // Step 5: Verify
            subtasks.push(self.make_subtask(
                "Take screenshot to verify web task completed",
                ActionType::Verify { check: format!("Web task done: {}", request) },
                vec![subtasks[3].id.clone()], 2,
            ));
        } else if is_file_task {
            // Step 1: Explore current files
            subtasks.push(self.make_subtask(
                "List files to understand current state",
                ActionType::Bash { command: if cfg!(target_os = "windows") {
                    "dir && cd".to_string()
                } else {
                    "ls -la && pwd".to_string()
                } },
                vec![], 2,
            ));
            
            // Step 2: Execute the file operation (LLM will generate proper command)
            subtasks.push(self.make_subtask(
                &format!("Execute file operation: {}", request),
                ActionType::Think { reasoning: format!("Generate and run the bash command to: {}", request) },
                vec![subtasks[0].id.clone()], 3,
            ));
            
            // Step 3: Verify
            subtasks.push(self.make_subtask(
                "Verify file operation succeeded",
                ActionType::Bash { command: if cfg!(target_os = "windows") { "dir".to_string() } else { "ls -la".to_string() } },
                vec![subtasks[1].id.clone()], 2,
            ));
        } else if is_app_task {
            // Extract app name from request
            let app_name = extract_app_name(&request_lower);
            
            if request_lower.contains("close") || request_lower.contains("quit") {
                let quit_cmd = if cfg!(target_os = "windows") {
                    format!(r#"taskkill /IM "{}.exe" /T"#, app_name)
                } else {
                    format!(r#"osascript -e 'tell application "{}" to quit'"#, app_name)
                };
                subtasks.push(self.make_subtask(
                    &format!("Quit application: {}", app_name),
                    ActionType::Bash { command: quit_cmd },
                    vec![], 2,
                ));
            } else {
                let launch_cmd = if cfg!(target_os = "windows") {
                    format!(r#"start "" "{}""#, app_name)
                } else {
                    format!(r#"open -a "{}""#, app_name)
                };
                subtasks.push(self.make_subtask(
                    &format!("Launch application: {}", app_name),
                    ActionType::Bash { command: launch_cmd },
                    vec![], 2,
                ));
            }
            
            subtasks.push(self.make_subtask(
                "Wait for app to respond",
                ActionType::Wait { duration_ms: 1500 },
                vec![subtasks[0].id.clone()], 1,
            ));
            
            subtasks.push(self.make_subtask(
                "Verify app state",
                ActionType::Computer { action: "screenshot".to_string(), params: serde_json::json!({}) },
                vec![subtasks[1].id.clone()], 2,
            ));
        } else {
            // General task - screenshot first, then delegate to LLM
            subtasks.push(self.make_subtask(
                "Capture current screen state",
                ActionType::Computer { action: "screenshot".to_string(), params: serde_json::json!({}) },
                vec![], 2,
            ));
            
            subtasks.push(self.make_subtask(
                &format!("Analyze and plan: {}", request),
                ActionType::Think { reasoning: format!("Determine best approach to accomplish: {}", request) },
                vec![subtasks[0].id.clone()], 1,
            ));
            
            subtasks.push(self.make_subtask(
                &format!("Execute primary task: {}", request),
                ActionType::Think { reasoning: format!("Execute: {}", request) },
                vec![subtasks[1].id.clone()], 3,
            ));
            
            subtasks.push(self.make_subtask(
                "Verify task completed",
                ActionType::Verify { check: format!("Completed: {}", request) },
                vec![subtasks[2].id.clone()], 2,
            ));
        }
        
        println!("[planner] Created {} subtasks for: \"{}\"", subtasks.len(), 
            if request.len() > 60 { &request[..60] } else { request });
        
        Ok(subtasks)
    }
    
    /// Helper to create a subtask with less boilerplate
    fn make_subtask(
        &self,
        description: &str,
        action_type: ActionType,
        dependencies: Vec<String>,
        max_retries: u32,
    ) -> Subtask {
        Subtask {
            id: Uuid::new_v4().to_string(),
            description: description.to_string(),
            action_type,
            dependencies,
            status: SubtaskStatus::Pending,
            retry_count: 0,
            max_retries,
            result: None,
        }
    }

    /// Get the next subtask that's ready to execute (all dependencies satisfied)
    pub fn get_next_ready_subtask<'a>(&self, task: &'a mut Task) -> Option<&'a mut Subtask> {
        let completed_ids: HashSet<String> = task
            .subtasks
            .iter()
            .filter(|s| s.status == SubtaskStatus::Completed)
            .map(|s| s.id.clone())
            .collect();
        
        for subtask in &mut task.subtasks {
            if subtask.status == SubtaskStatus::Pending {
                let deps_satisfied = subtask
                    .dependencies
                    .iter()
                    .all(|dep_id| completed_ids.contains(dep_id));
                
                if deps_satisfied {
                    subtask.status = SubtaskStatus::Ready;
                    return Some(subtask);
                }
            }
        }
        
        None
    }

    /// Check if all subtasks are complete
    pub fn is_complete(&self, task: &Task) -> bool {
        task.subtasks.iter().all(|s| {
            s.status == SubtaskStatus::Completed || s.status == SubtaskStatus::Failed
        })
    }

    /// Get execution progress as percentage
    pub fn get_progress(&self, task: &Task) -> f32 {
        let total = task.subtasks.len();
        let completed = task
            .subtasks
            .iter()
            .filter(|s| s.status == SubtaskStatus::Completed)
            .count();
        
        if total == 0 {
            0.0
        } else {
            (completed as f32 / total as f32) * 100.0
        }
    }

    /// Replan when a subtask fails
    pub async fn replan_on_failure(
        &self,
        task: &mut Task,
        failed_subtask_id: &str,
        error: &str,
    ) -> anyhow::Result<()> {
        // Find the failed subtask
        if let Some(subtask) = task.subtasks.iter_mut().find(|s| s.id == failed_subtask_id) {
            subtask.retry_count += 1;
            
            if subtask.retry_count >= subtask.max_retries {
                subtask.status = SubtaskStatus::Failed;
                
                // Try to create an alternative approach
                let alternative = Subtask {
                    id: Uuid::new_v4().to_string(),
                    description: format!("Alternative approach for: {}", subtask.description),
                    action_type: ActionType::Think {
                        reasoning: format!("Previous approach failed with: {}. Trying alternative.", error),
                    },
                    dependencies: subtask.dependencies.clone(),
                    status: SubtaskStatus::Pending,
                    retry_count: 0,
                    max_retries: 2,
                    result: None,
                };
                
                task.subtasks.push(alternative);
            } else {
                subtask.status = SubtaskStatus::Retrying;
            }
        }
        
        Ok(())
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract app name from a request string like "open chrome" or "launch Safari"
fn extract_app_name(request: &str) -> String {
    let known_apps = [
        ("chrome", "Google Chrome"),
        ("safari", "Safari"),
        ("firefox", "Firefox"),
        ("slack", "Slack"),
        ("discord", "Discord"),
        ("spotify", "Spotify"),
        ("vscode", "Visual Studio Code"),
        ("code", "Visual Studio Code"),
        ("terminal", "Terminal"),
        ("finder", "Finder"),
        ("notes", "Notes"),
        ("mail", "Mail"),
        ("calendar", "Calendar"),
        ("messages", "Messages"),
        ("photos", "Photos"),
        ("music", "Music"),
        ("preview", "Preview"),
        ("textedit", "TextEdit"),
        ("pages", "Pages"),
        ("numbers", "Numbers"),
        ("keynote", "Keynote"),
        ("xcode", "Xcode"),
        ("figma", "Figma"),
        ("notion", "Notion"),
        ("obsidian", "Obsidian"),
        ("iterm", "iTerm"),
        ("warp", "Warp"),
        ("arc", "Arc"),
        ("zoom", "zoom.us"),
        ("teams", "Microsoft Teams"),
        ("word", "Microsoft Word"),
        ("excel", "Microsoft Excel"),
        ("powerpoint", "Microsoft PowerPoint"),
    ];
    
    let lower = request.to_lowercase();
    for (keyword, name) in &known_apps {
        if lower.contains(keyword) {
            return name.to_string();
        }
    }
    
    // Try to extract the word after "open" or "launch"
    for trigger in &["open ", "launch ", "start ", "close ", "quit "] {
        if let Some(pos) = lower.find(trigger) {
            let after = &request[pos + trigger.len()..];
            let name = after.split_whitespace().next().unwrap_or("").trim();
            if !name.is_empty() {
                // Capitalize first letter
                let mut chars = name.chars();
                if let Some(first) = chars.next() {
                    return first.to_uppercase().to_string() + chars.as_str();
                }
            }
        }
    }
    
    "Finder".to_string() // Default
}