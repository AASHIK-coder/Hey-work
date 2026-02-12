//! Agent Swarm - Multi-Agent System for Complex Task Handling
//!
//! Inspired by Kimi Agent Swarm and OK Computer:
//! - Multiple specialized agents working together
//! - Task decomposition with dependency management
//! - Parallel execution of independent subtasks
//! - Verification and self-correction loops
//! - Human-in-the-loop for ambiguous tasks


use crate::api::{AnthropicClient, ContentBlock, Message, StreamEvent};
use crate::storage::Usage;
use crate::computer::ComputerControl;
use crate::bash::BashExecutor;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

/// Types of specialized agents in the swarm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    /// Analyzes requests and creates execution plans
    Planner,
    /// Executes actions (computer, bash, browser)
    Executor,
    /// Verifies results match expectations
    Verifier,
    /// Critiques execution and suggests improvements
    Critic,
    /// Handles errors and recovery strategies
    Recovery,
    /// Coordinates between other agents
    Coordinator,
    /// Handles document generation and data processing
    Specialist,
}

impl AgentType {
    pub fn system_prompt(&self) -> &'static str {
        match self {
            AgentType::Planner => PLANNER_PROMPT,
            AgentType::Executor => EXECUTOR_PROMPT,
            AgentType::Verifier => VERIFIER_PROMPT,
            AgentType::Critic => CRITIC_PROMPT,
            AgentType::Recovery => RECOVERY_PROMPT,
            AgentType::Coordinator => COORDINATOR_PROMPT,
            AgentType::Specialist => SPECIALIST_PROMPT,
        }
    }
}

/// A task that can be decomposed into subtasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexTask {
    pub id: String,
    pub description: String,
    pub goal: String,
    pub subtasks: Vec<SubTask>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub max_parallel: usize,
    pub require_verification: bool,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Individual subtask with dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub parent_id: Option<String>,
    pub description: String,
    pub agent_type: AgentType,
    pub dependencies: Vec<String>,
    pub status: SubTaskStatus,
    pub result: Option<TaskResult>,
    pub verification_result: Option<VerificationResult>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub estimated_duration_ms: u64,
}

/// Result of executing a subtask
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskResult {
    pub success: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<ToolCallRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub screenshots: Vec<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    #[serde(skip)]
    pub tokens_used: Usage,
}

/// Record of a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: String,
    pub timestamp: DateTime<Utc>,
}

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub passed: bool,
    pub score: f32, // 0.0 to 1.0
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Planning,
    Executing,
    Verifying,
    Completed,
    Failed,
    NeedsUserInput,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubTaskStatus {
    Pending,
    Ready,
    Executing,
    Completed,
    Failed,
    Verifying,
    NeedsRetry,
    Blocked,
}

/// The Agent Swarm orchestrator
pub struct AgentSwarm {
    /// Active tasks
    tasks: Arc<RwLock<HashMap<String, ComplexTask>>>,
    /// Task queue for scheduling
    task_queue: Arc<Mutex<VecDeque<String>>>,
    /// Agent execution engines
    executors: HashMap<AgentType, AgentExecutor>,
    /// Event channel for UI updates
    event_tx: mpsc::UnboundedSender<SwarmEvent>,
    /// Configuration
    config: SwarmConfig,
    /// Statistics
    stats: Arc<RwLock<SwarmStats>>,
    /// Real execution tools
    computer: Arc<Mutex<Option<ComputerControl>>>,
    bash: Arc<Mutex<BashExecutor>>,
}

/// Configuration for the swarm
#[derive(Debug, Clone)]
pub struct SwarmConfig {
    /// Maximum parallel subtasks
    pub max_parallel: usize,
    /// Enable verification after each step
    pub verification_enabled: bool,
    /// Enable critic review
    pub critic_enabled: bool,
    /// Auto-retry failed tasks
    pub auto_retry: bool,
    /// Max retries per subtask
    pub max_retries: u32,
    /// Timeout for subtask execution (seconds)
    pub subtask_timeout_secs: u64,
    /// Enable parallel execution where possible
    pub parallel_execution: bool,
    /// Require human confirmation for destructive actions
    pub confirm_destructive: bool,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_parallel: 3,
            verification_enabled: true,
            critic_enabled: true,
            auto_retry: true,
            max_retries: 3,
            subtask_timeout_secs: 120,
            parallel_execution: true,
            confirm_destructive: true,
        }
    }
}

/// Statistics tracking
#[derive(Debug, Default, Clone)]
pub struct SwarmStats {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub subtasks_executed: u64,
    pub verifications_passed: u64,
    pub verifications_failed: u64,
    pub retries_triggered: u64,
    pub avg_task_duration_ms: u64,
}

/// Events emitted by the swarm
#[derive(Debug, Clone)]
pub enum SwarmEvent {
    TaskStarted { task_id: String, description: String },
    TaskPlanning { task_id: String, agent: AgentType },
    SubTaskStarted { task_id: String, subtask_id: String, agent: AgentType },
    SubTaskCompleted { task_id: String, subtask_id: String, result: TaskResult },
    SubTaskFailed { task_id: String, subtask_id: String, error: String },
    VerificationCompleted { task_id: String, subtask_id: String, passed: bool, score: f32 },
    CriticReview { task_id: String, issues: Vec<String>, suggestions: Vec<String> },
    TaskCompleted { task_id: String, success: bool },
    NeedsUserInput { task_id: String, question: String },
    RecoveryAttempt { task_id: String, subtask_id: String, strategy: String },
}

/// Individual agent executor
pub struct AgentExecutor {
    agent_type: AgentType,
    api_key: String,
    model: String,
}

impl AgentSwarm {
    pub fn new(api_key: String, model: String, event_tx: mpsc::UnboundedSender<SwarmEvent>) -> Self {
        let mut executors = HashMap::new();
        
        for agent_type in [
            AgentType::Planner,
            AgentType::Executor,
            AgentType::Verifier,
            AgentType::Critic,
            AgentType::Recovery,
            AgentType::Coordinator,
            AgentType::Specialist,
        ] {
            executors.insert(agent_type, AgentExecutor {
                agent_type,
                api_key: api_key.clone(),
                model: model.clone(),
            });
        }
        
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            task_queue: Arc::new(Mutex::new(VecDeque::new())),
            executors,
            event_tx,
            config: SwarmConfig::default(),
            stats: Arc::new(RwLock::new(SwarmStats::default())),
            computer: Arc::new(Mutex::new(None)),
            bash: Arc::new(Mutex::new(BashExecutor::new())),
        }
    }

    /// Initialize execution tools (computer control)
    async fn init_tools(&self) -> anyhow::Result<()> {
        let mut computer_guard = self.computer.lock().await;
        if computer_guard.is_none() {
            match ComputerControl::new() {
                Ok(computer) => {
                    *computer_guard = Some(computer);
                    println!("[swarm] Computer control initialized");
                }
                Err(e) => {
                    println!("[swarm] Failed to initialize computer control: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Submit a new complex task to the swarm
    pub async fn submit_task(&self, description: String) -> String {
        let task_id = Uuid::new_v4().to_string();
        
        let task = ComplexTask {
            id: task_id.clone(),
            description: description.clone(),
            goal: description.clone(),
            subtasks: Vec::new(),
            status: TaskStatus::Pending,
            created_at: chrono::Utc::now(),
            max_parallel: self.config.max_parallel,
            require_verification: self.config.verification_enabled,
            metadata: HashMap::new(),
        };
        
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_id.clone(), task);
        }
        
        {
            let mut queue = self.task_queue.lock().await;
            queue.push_back(task_id.clone());
        }
        
        let _ = self.event_tx.send(SwarmEvent::TaskStarted {
            task_id: task_id.clone(),
            description,
        });
        
        // Start processing
        let swarm = Arc::new(self.clone_swarm());
        let task_id_clone = task_id.clone();
        tokio::spawn(async move {
            swarm.process_task(task_id_clone).await;
        });
        
        task_id
    }

    /// Process a task through the swarm
    async fn process_task(&self, task_id: String) {
        // Initialize tools first
        let _ = self.init_tools().await;
        
        // Phase 1: Planning
        self.plan_task(task_id.clone()).await;
        
        // Phase 2: Execution
        self.execute_task(task_id.clone()).await;
        
        // Phase 3: Verification & Review
        if self.config.critic_enabled {
            self.critic_review(task_id.clone()).await;
        }
        
        // Mark completion
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                let all_success = task.subtasks.iter().all(|st| 
                    st.status == SubTaskStatus::Completed
                );
                task.status = if all_success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                };
            }
        }
        
        let _ = self.event_tx.send(SwarmEvent::TaskCompleted {
            task_id,
            success: true,
        });
    }

    /// Phase 1: Decompose task into subtasks using Planner agent
    async fn plan_task(&self, task_id: String) {
        let _ = self.event_tx.send(SwarmEvent::TaskPlanning {
            task_id: task_id.clone(),
            agent: AgentType::Planner,
        });
        
        let description = {
            let tasks = self.tasks.read().await;
            tasks.get(&task_id).map(|t| t.description.clone())
        };
        
        if let Some(desc) = description {
            // Use Planner agent to create execution plan
            let plan = self.create_execution_plan(&desc).await;
            
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = TaskStatus::Executing;
                task.subtasks = plan;
            }
        }
    }

    /// Create execution plan with dependencies
    async fn create_execution_plan(&self, description: &str) -> Vec<SubTask> {
        let _planner = self.executors.get(&AgentType::Planner).unwrap();
        
        // Analyze task complexity and create subtasks
        let analysis = self.analyze_task_complexity(description).await;
        
        let mut subtasks = Vec::new();
        
        // Create subtasks based on analysis
        for (idx, step) in analysis.steps.iter().enumerate() {
            let subtask = SubTask {
                id: format!("{}_step_{}", Uuid::new_v4(), idx),
                parent_id: None,
                description: step.description.clone(),
                agent_type: step.agent_type,
                dependencies: step.dependencies.clone(),
                status: if step.dependencies.is_empty() {
                    SubTaskStatus::Ready
                } else {
                    SubTaskStatus::Blocked
                },
                result: None,
                verification_result: None,
                retry_count: 0,
                max_retries: self.config.max_retries,
                created_at: chrono::Utc::now(),
                started_at: None,
                completed_at: None,
                estimated_duration_ms: step.estimated_duration_ms,
            };
            subtasks.push(subtask);
        }
        
        subtasks
    }

    /// Analyze task and determine best approach using LLM
    async fn analyze_task_complexity(&self, description: &str) -> TaskAnalysis {
        // Try to use LLM for intelligent task decomposition
        if let Some(planner) = self.executors.get(&AgentType::Planner) {
            let client = crate::api::AnthropicClient::new(
                planner.api_key.clone(),
                planner.model.clone(),
            );
            
            let prompt = format!(
                r#"Decompose this task into 2-6 concrete, executable steps. For each step, specify the agent type and any dependencies.

Task: "{}"

Return a JSON object:
{{
  "complexity": "simple" | "moderate" | "complex",
  "parallelizable": true/false,
  "steps": [
    {{
      "description": "Specific action description",
      "agent_type": "Planner" | "Executor" | "Specialist" | "Verifier",
      "depends_on": [],
      "estimated_ms": 5000
    }}
  ]
}}

Agent types:
- Planner: Analysis, planning, research
- Executor: Computer actions (click, type, screenshot), bash commands, app launching
- Specialist: Document generation, data processing, Python code
- Verifier: Check results, take screenshot to verify

Be specific in descriptions. For "Open Chrome and search", the steps should be:
1. Executor: "Run bash: open -a 'Google Chrome'"
2. Executor: "Take screenshot to see current state"
3. Executor: "Click on search bar and type query"
4. Verifier: "Take screenshot to verify search results appear"

Return ONLY JSON."#,
                description
            );
            
            let messages = vec![crate::api::Message {
                role: "user".to_string(),
                content: vec![crate::api::ContentBlock::Text { text: prompt }],
            }];
            
            if let Ok(result) = client.complete(None, messages, None).await {
                let text = result.content.iter()
                    .filter_map(|b| if let crate::api::ContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
                    .collect::<String>();
                
                // Parse JSON response
                if let Some(start) = text.find('{') {
                    if let Some(end) = text.rfind('}') {
                        let json_str = &text[start..=end];
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                            let complexity = match parsed.get("complexity").and_then(|v| v.as_str()).unwrap_or("moderate") {
                                "simple" => TaskComplexity::Simple,
                                "complex" => TaskComplexity::Complex,
                                _ => TaskComplexity::Moderate,
                            };
                            
                            let parallelizable = parsed.get("parallelizable").and_then(|v| v.as_bool()).unwrap_or(false);
                            
                            if let Some(steps_arr) = parsed.get("steps").and_then(|v| v.as_array()) {
                                let mut steps = Vec::new();
                                let mut total_ms = 0u64;
                                
                                for step in steps_arr {
                                    let desc = step.get("description").and_then(|v| v.as_str()).unwrap_or("Execute task");
                                    let agent = match step.get("agent_type").and_then(|v| v.as_str()).unwrap_or("Executor") {
                                        "Planner" => AgentType::Planner,
                                        "Specialist" => AgentType::Specialist,
                                        "Verifier" => AgentType::Verifier,
                                        "Critic" => AgentType::Critic,
                                        _ => AgentType::Executor,
                                    };
                                    let est_ms = step.get("estimated_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
                                    let deps: Vec<String> = step.get("depends_on")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default();
                                    
                                    total_ms += est_ms;
                                    steps.push(AnalysisStep {
                                        description: desc.to_string(),
                                        agent_type: agent,
                                        dependencies: deps,
                                        estimated_duration_ms: est_ms,
                                    });
                                }
                                
                                if !steps.is_empty() {
                                    println!("[swarm] LLM decomposed task into {} steps", steps.len());
                                    return TaskAnalysis {
                                        complexity,
                                        steps,
                                        parallelizable,
                                        requires_verification: true,
                                        estimated_total_duration_ms: total_ms,
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback: simple sequential plan
        println!("[swarm] Using fallback task decomposition");
        TaskAnalysis {
            complexity: TaskComplexity::Moderate,
            steps: vec![
                AnalysisStep {
                    description: format!("Take screenshot to observe current state"),
                    agent_type: AgentType::Executor,
                    dependencies: vec![],
                    estimated_duration_ms: 3000,
                },
                AnalysisStep {
                    description: format!("Execute main task: {}", description),
                    agent_type: AgentType::Executor,
                    dependencies: vec![],
                    estimated_duration_ms: 10000,
                },
                AnalysisStep {
                    description: "Verify task completion by taking screenshot".to_string(),
                    agent_type: AgentType::Verifier,
                    dependencies: vec![],
                    estimated_duration_ms: 3000,
                },
            ],
            parallelizable: false,
            requires_verification: true,
            estimated_total_duration_ms: 16000,
        }
    }

    /// Phase 2: Execute subtasks
    async fn execute_task(&self, task_id: String) {
        loop {
            // Get ready subtasks
            let ready_subtasks = {
                let tasks = self.tasks.read().await;
                if let Some(task) = tasks.get(&task_id) {
                    task.subtasks
                        .iter()
                        .filter(|st| st.status == SubTaskStatus::Ready)
                        .map(|st| st.id.clone())
                        .collect::<Vec<_>>()
                } else {
                    break;
                }
            };
            
            if ready_subtasks.is_empty() {
                // Check if all done or blocked
                let all_done = {
                    let tasks = self.tasks.read().await;
                    if let Some(task) = tasks.get(&task_id) {
                        task.subtasks.iter().all(|st| {
                            matches!(st.status, SubTaskStatus::Completed | SubTaskStatus::Failed)
                        })
                    } else {
                        true
                    }
                };
                
                if all_done {
                    break;
                }
                
                // Update blocked tasks
                self.update_blocked_tasks(task_id.clone()).await;
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            
            // Execute ready subtasks (parallel if enabled)
            if self.config.parallel_execution && ready_subtasks.len() > 1 {
                let mut handles = Vec::new();
                
                for subtask_id in ready_subtasks.iter().take(self.config.max_parallel) {
                    let swarm = Arc::new(self.clone_swarm());
                    let tid = task_id.clone();
                    let sid = subtask_id.clone();
                    
                    let handle = tokio::spawn(async move {
                        swarm.execute_subtask(tid, sid).await;
                    });
                    handles.push(handle);
                }
                
                for handle in handles {
                    let _ = handle.await;
                }
            } else {
                // Sequential execution
                for subtask_id in ready_subtasks {
                    self.execute_subtask(task_id.clone(), subtask_id).await;
                }
            }
        }
    }

    /// Execute a single subtask
    async fn execute_subtask(&self, task_id: String, subtask_id: String) {
        // Get subtask details
        let subtask_opt = {
            let tasks = self.tasks.read().await;
            if let Some(task) = tasks.get(&task_id) {
                task.subtasks.iter().find(|st| st.id == subtask_id).cloned()
            } else {
                None
            }
        };
        
        if let Some(subtask) = subtask_opt {
            // Mark as executing
            {
                let mut tasks = self.tasks.write().await;
                if let Some(task) = tasks.get_mut(&task_id) {
                    if let Some(st) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
                        st.status = SubTaskStatus::Executing;
                        st.started_at = Some(chrono::Utc::now());
                    }
                }
            }
            
            let _ = self.event_tx.send(SwarmEvent::SubTaskStarted {
                task_id: task_id.clone(),
                subtask_id: subtask_id.clone(),
                agent: subtask.agent_type,
            });
            
            // Execute with timeout
            let timeout = Duration::from_secs(self.config.subtask_timeout_secs);
            let result = tokio::time::timeout(
                timeout,
                self.run_agent_executor(&subtask)
            ).await;
            
            match result {
                Ok(Ok(task_result)) => {
                    // Success
                    let mut tasks = self.tasks.write().await;
                    if let Some(task) = tasks.get_mut(&task_id) {
                        if let Some(st) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
                            st.status = SubTaskStatus::Completed;
                            st.completed_at = Some(chrono::Utc::now());
                            st.result = Some(task_result.clone());
                        }
                    }
                    
                    let _ = self.event_tx.send(SwarmEvent::SubTaskCompleted {
                        task_id: task_id.clone(),
                        subtask_id: subtask_id.clone(),
                        result: task_result,
                    });
                    
                    // Trigger verification if enabled
                    if self.config.verification_enabled {
                        self.verify_subtask(task_id.clone(), subtask_id.clone()).await;
                    }
                }
                Ok(Err(e)) => {
                    // Execution error
                    self.handle_subtask_error(task_id.clone(), subtask_id.clone(), e).await;
                }
                Err(_) => {
                    // Timeout
                    self.handle_subtask_error(
                        task_id.clone(),
                        subtask_id.clone(),
                        "Execution timeout".to_string()
                    ).await;
                }
            }
        }
    }

    /// Run the appropriate agent executor with REAL TOOL EXECUTION
    async fn run_agent_executor(&self, subtask: &SubTask) -> Result<TaskResult, String> {
        let executor = self.executors.get(&subtask.agent_type)
            .ok_or("Executor not found")?;
        
        let start_time = std::time::Instant::now();
        
        // Try to parse and execute the subtask description as a real tool call
        let description_lower = subtask.description.to_lowercase();
        
        // Check for computer actions
        if description_lower.contains("screenshot") || description_lower.contains("take a screenshot") {
            return self.execute_screenshot().await;
        }
        
        if description_lower.contains("click") {
            // Try to parse click coordinates or element
            // For now, use a default center click or parse from description
            return self.execute_click(&description_lower).await;
        }
        
        if description_lower.contains("type") || description_lower.contains("enter") {
            // Try to extract text to type
            return self.execute_type(&subtask.description).await;
        }
        
        // Check for bash commands
        if description_lower.starts_with("open ") || description_lower.contains("run ") || 
           description_lower.contains("execute ") || description_lower.contains("launch ") {
            // Extract command from description
            let command = self.extract_command(&subtask.description);
            if !command.is_empty() {
                return self.execute_bash(&command).await;
            }
        }
        
        // For analysis/planning tasks, use LLM
        if matches!(subtask.agent_type, AgentType::Planner | AgentType::Critic | AgentType::Verifier) {
            return self.execute_llm_task(executor, subtask).await;
        }
        
        // Default: Try to interpret and execute using LLM
        println!("[swarm] Using LLM to interpret task: {}", subtask.description);
        return self.execute_llm_task(executor, subtask).await
    }

    /// Execute screenshot tool
    async fn execute_screenshot(&self) -> Result<TaskResult, String> {
        let computer_guard = self.computer.lock().await;
        let computer = match computer_guard.as_ref() {
            Some(c) => c,
            None => return Err("Computer control not initialized".to_string()),
        };
        
        let screen_w = computer.screen_width;
        let screen_h = computer.screen_height;
        
        let result = tokio::task::spawn_blocking(move || {
            let computer = ComputerControl::with_dimensions(screen_w, screen_h);
            computer.take_screenshot()
        }).await;
        
        match result {
            Ok(Ok(screenshot)) => Ok(TaskResult {
                success: true,
                output: "Screenshot captured".to_string(),
                screenshots: vec![screenshot.clone()],
                tool_calls: vec![ToolCallRecord {
                    tool_name: "computer".to_string(),
                    input: serde_json::json!({"action": "screenshot"}),
                    output: "Screenshot captured".to_string(),
                    timestamp: chrono::Utc::now(),
                }],
                ..Default::default()
            }),
            Ok(Err(e)) => Err(format!("Screenshot failed: {}", e)),
            Err(e) => Err(format!("Task failed: {}", e)),
        }
    }

    /// Execute click action - uses LLM to determine WHERE to click via screenshot analysis
    async fn execute_click(&self, description: &str) -> Result<TaskResult, String> {
        // Step 1: Take a screenshot so the LLM can see what's on screen
        let screenshot_result = self.execute_screenshot().await?;
        let screenshot_b64 = screenshot_result.screenshots.first()
            .cloned()
            .ok_or("Failed to capture screenshot for click target analysis")?;
        
        // Step 2: Try to parse coordinates from description (e.g. "click at [300, 400]")
        let parsed_coords = parse_coordinates_from_text(description);
        
        let (x, y) = if let Some((px, py)) = parsed_coords {
            (px, py)
        } else {
            // Step 3: Ask LLM to identify click target from screenshot
            let executor = self.executors.values().next()
                .ok_or("No executor available")?;
            let client = crate::api::AnthropicClient::new(
                executor.api_key.clone(), executor.model.clone(),
            );
            
            let messages = vec![crate::api::Message {
                role: "user".to_string(),
                content: vec![
                    crate::api::ContentBlock::Image {
                        source: crate::api::ImageSource {
                            source_type: "base64".to_string(),
                            media_type: "image/jpeg".to_string(),
                            data: screenshot_b64.clone(),
                        },
                    },
                    crate::api::ContentBlock::Text {
                        text: format!(
                            "Look at this screenshot. I need to click on the element described as: \"{}\"\n\n\
                            Return ONLY a JSON object with the coordinates: {{\"x\": <number>, \"y\": <number>}}\n\
                            Coordinates should be in the range 0-1000 (normalized screen space).\n\
                            Return ONLY JSON, nothing else.",
                            description
                        ),
                    },
                ],
            }];
            
            match client.complete(None, messages, None).await {
                Ok(result) => {
                    let text = result.content.iter()
                        .filter_map(|b| if let crate::api::ContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
                        .collect::<String>();
                    
                    // Parse coordinates from LLM response
                    if let Some(start) = text.find('{') {
                        if let Some(end) = text.rfind('}') {
                            let json_str = &text[start..=end];
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                                let cx = parsed.get("x").and_then(|v| v.as_i64()).unwrap_or(500) as i32;
                                let cy = parsed.get("y").and_then(|v| v.as_i64()).unwrap_or(500) as i32;
                                (cx, cy)
                            } else {
                                (500, 500) // Fallback to center
                            }
                        } else {
                            (500, 500)
                        }
                    } else {
                        (500, 500)
                    }
                }
                Err(e) => {
                    println!("[swarm] LLM click analysis failed: {}, using center", e);
                    (500, 500)
                }
            }
        };
        
        println!("[swarm] Clicking at [{}, {}] for: {}", x, y, description);
        
        let computer_guard = self.computer.lock().await;
        let computer = match computer_guard.as_ref() {
            Some(c) => c,
            None => return Err("Computer control not initialized".to_string()),
        };
        
        let screen_w = computer.screen_width;
        let screen_h = computer.screen_height;
        
        let action = crate::computer::ComputerAction {
            action: "click".to_string(),
            coordinate: Some([x, y]),
            start_coordinate: None,
            text: None,
            scroll_direction: None,
            scroll_amount: None,
            key: None,
            region: None,
        };
        
        let result = tokio::task::spawn_blocking(move || {
            let computer = ComputerControl::with_dimensions(screen_w, screen_h);
            computer.perform_action(&action)
        }).await;
        
        match result {
            Ok(Ok(screenshot)) => Ok(TaskResult {
                success: true,
                output: format!("Clicked at [{}, {}]", x, y),
                screenshots: screenshot.map(|s| vec![s]).unwrap_or_default(),
                tool_calls: vec![ToolCallRecord {
                    tool_name: "computer".to_string(),
                    input: serde_json::json!({"action": "click", "coordinate": [x, y]}),
                    output: format!("Clicked at [{}, {}]", x, y),
                    timestamp: chrono::Utc::now(),
                }],
                ..Default::default()
            }),
            Ok(Err(e)) => Err(format!("Click failed: {}", e)),
            Err(e) => Err(format!("Task failed: {}", e)),
        }
    }

    /// Execute type action
    async fn execute_type(&self, description: &str) -> Result<TaskResult, String> {
        // Extract text to type - simple heuristic
        let text = if let Some(pos) = description.find('"') {
            if let Some(end) = description[pos+1..].find('"') {
                description[pos+1..pos+1+end].to_string()
            } else {
                "typed text".to_string()
            }
        } else {
            "typed text".to_string()
        };
        
        let computer_guard = self.computer.lock().await;
        let computer = match computer_guard.as_ref() {
            Some(c) => c,
            None => return Err("Computer control not initialized".to_string()),
        };
        
        let screen_w = computer.screen_width;
        let screen_h = computer.screen_height;
        
        let action = crate::computer::ComputerAction {
            action: "type".to_string(),
            coordinate: None,
            start_coordinate: None,
            text: Some(text.clone()),
            scroll_direction: None,
            scroll_amount: None,
            key: None,
            region: None,
        };
        
        let result = tokio::task::spawn_blocking(move || {
            let computer = ComputerControl::with_dimensions(screen_w, screen_h);
            computer.perform_action(&action)
        }).await;
        
        match result {
            Ok(Ok(screenshot)) => Ok(TaskResult {
                success: true,
                output: format!("Typed: '{}'", text),
                screenshots: screenshot.map(|s| vec![s]).unwrap_or_default(),
                tool_calls: vec![ToolCallRecord {
                    tool_name: "computer".to_string(),
                    input: serde_json::json!({"action": "type", "text": text}),
                    output: format!("Typed: '{}'", text),
                    timestamp: chrono::Utc::now(),
                }],
                ..Default::default()
            }),
            Ok(Err(e)) => Err(format!("Type failed: {}", e)),
            Err(e) => Err(format!("Task failed: {}", e)),
        }
    }

    /// Execute bash command
    async fn execute_bash(&self, command: &str) -> Result<TaskResult, String> {
        let bash = self.bash.lock().await;
        
        match bash.execute(command) {
            Ok(output) => Ok(TaskResult {
                success: output.exit_code == 0,
                output: output.stdout.clone(),
                error: if output.exit_code != 0 { Some(output.stderr.clone()) } else { None },
                tool_calls: vec![ToolCallRecord {
                    tool_name: "bash".to_string(),
                    input: serde_json::json!({"command": command}),
                    output: output.stdout,
                    timestamp: chrono::Utc::now(),
                }],
                ..Default::default()
            }),
            Err(e) => Err(format!("Bash execution failed: {}", e)),
        }
    }

    /// Execute LLM-based task (for planning/analysis)
    async fn execute_llm_task(&self, executor: &AgentExecutor, subtask: &SubTask) -> Result<TaskResult, String> {
        let client = crate::api::AnthropicClient::new(
            executor.api_key.clone(), 
            executor.model.clone()
        );
        
        let system_prompt = self.get_agent_system_prompt(executor.agent_type);
        
        let messages = vec![crate::api::Message {
            role: "user".to_string(),
            content: vec![crate::api::ContentBlock::Text { 
                text: format!("Execute this task: {}", subtask.description) 
            }],
        }];
        
        match client.complete(Some(system_prompt), messages, None).await {
            Ok(result) => {
                let output = result.content.iter()
                    .filter_map(|block| {
                        if let crate::api::ContentBlock::Text { text } = block {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                
                Ok(TaskResult {
                    success: true,
                    output: if output.is_empty() { 
                        format!("[{:?}] Task completed", subtask.agent_type) 
                    } else { 
                        output 
                    },
                    ..Default::default()
                })
            }
            Err(e) => {
                Err(format!("LLM API error: {}", e))
            }
        }
    }

    /// Extract command from description
    fn extract_command(&self, description: &str) -> String {
        let lower = description.to_lowercase();
        
        // Try to extract app name for "open" commands
        if lower.starts_with("open ") {
            let after_open = &description[5..];
            let app_name = after_open.split_whitespace().next().unwrap_or("");
            if !app_name.is_empty() {
                return format!(r#"open -a "{}""#, app_name);
            }
        }
        
        // Default: return description as-is if it looks like a command
        description.to_string()
    }
    
    /// Get system prompt for agent type
    fn get_agent_system_prompt(&self, agent_type: AgentType) -> String {
        match agent_type {
            AgentType::Planner => PLANNER_PROMPT.to_string(),
            AgentType::Executor => EXECUTOR_PROMPT.to_string(),
            AgentType::Verifier => VERIFIER_PROMPT.to_string(),
            AgentType::Critic => CRITIC_PROMPT.to_string(),
            AgentType::Recovery => RECOVERY_PROMPT.to_string(),
            AgentType::Coordinator => COORDINATOR_PROMPT.to_string(),
            AgentType::Specialist => SPECIALIST_PROMPT.to_string(),
        }
    }

    /// Handle subtask errors with recovery
    async fn handle_subtask_error(&self, task_id: String, subtask_id: String, error: String) {
        let should_retry = {
            let tasks = self.tasks.read().await;
            if let Some(task) = tasks.get(&task_id) {
                if let Some(st) = task.subtasks.iter().find(|s| s.id == subtask_id) {
                    st.retry_count < st.max_retries && self.config.auto_retry
                } else {
                    false
                }
            } else {
                false
            }
        };
        
        if should_retry {
            // Attempt recovery
            let _ = self.event_tx.send(SwarmEvent::RecoveryAttempt {
                task_id: task_id.clone(),
                subtask_id: subtask_id.clone(),
                strategy: "Retry with modified approach".to_string(),
            });
            
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                if let Some(st) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
                    st.retry_count += 1;
                    st.status = SubTaskStatus::Ready; // Retry
                }
            }
        } else {
            // Mark as failed
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                if let Some(st) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
                    st.status = SubTaskStatus::Failed;
                    st.result = Some(TaskResult {
                        success: false,
                        output: error.clone(),
                        tool_calls: vec![],
                        screenshots: vec![],
                        error: Some(error.clone()),
                        duration_ms: 0,
                        tokens_used: Usage::default(),
                    });
                }
            }
            
            let _ = self.event_tx.send(SwarmEvent::SubTaskFailed {
                task_id,
                subtask_id,
                error,
            });
        }
    }

    /// Verify subtask result using LLM
    async fn verify_subtask(&self, task_id: String, subtask_id: String) {
        // Get the subtask result to verify
        let (subtask_desc, subtask_result) = {
            let tasks = self.tasks.read().await;
            if let Some(task) = tasks.get(&task_id) {
                if let Some(st) = task.subtasks.iter().find(|s| s.id == subtask_id) {
                    (st.description.clone(), st.result.clone())
                } else {
                    (String::new(), None)
                }
            } else {
                (String::new(), None)
            }
        };
        
        let verification = if let Some(ref result) = subtask_result {
            // Try LLM-based verification
            if let Some(verifier) = self.executors.get(&AgentType::Verifier) {
                let client = crate::api::AnthropicClient::new(
                    verifier.api_key.clone(),
                    verifier.model.clone(),
                );
                
                let prompt = format!(
                    r#"Verify this task execution result. Return JSON only.

Task: "{}"
Result success: {}
Output: "{}"
Error: {:?}

Return: {{"passed": true/false, "score": 0.0-1.0, "issues": ["issue1"], "suggestions": ["suggestion1"]}}"#,
                    subtask_desc,
                    result.success,
                    &result.output[..result.output.len().min(500)],
                    result.error
                );
                
                let messages = vec![crate::api::Message {
                    role: "user".to_string(),
                    content: vec![crate::api::ContentBlock::Text { text: prompt }],
                }];
                
                match client.complete(Some(VERIFIER_PROMPT.to_string()), messages, None).await {
                    Ok(api_result) => {
                        let text = api_result.content.iter()
                            .filter_map(|b| if let crate::api::ContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
                            .collect::<String>();
                        
                        // Parse JSON response
                        if let Some(start) = text.find('{') {
                            if let Some(end) = text.rfind('}') {
                                let json_str = &text[start..=end];
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    let passed = parsed.get("passed").and_then(|v| v.as_bool()).unwrap_or(result.success);
                                    let score = parsed.get("score").and_then(|v| v.as_f64()).unwrap_or(if result.success { 0.8 } else { 0.3 }) as f32;
                                    let issues: Vec<String> = parsed.get("issues")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default();
                                    let suggestions: Vec<String> = parsed.get("suggestions")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default();
                                    
                                    VerificationResult { passed, score, issues, suggestions }
                                } else {
                                    VerificationResult {
                                        passed: result.success,
                                        score: if result.success { 0.8 } else { 0.3 },
                                        issues: vec![],
                                        suggestions: vec![],
                                    }
                                }
                            } else {
                                VerificationResult {
                                    passed: result.success,
                                    score: if result.success { 0.8 } else { 0.3 },
                                    issues: vec![],
                                    suggestions: vec![],
                                }
                            }
                        } else {
                            VerificationResult {
                                passed: result.success,
                                score: if result.success { 0.8 } else { 0.3 },
                                issues: vec![],
                                suggestions: vec![],
                            }
                        }
                    }
                    Err(_) => {
                        // Fallback: base verification on result success
                        VerificationResult {
                            passed: result.success,
                            score: if result.success { 0.75 } else { 0.2 },
                            issues: if result.success { vec![] } else { vec!["Task reported failure".to_string()] },
                            suggestions: vec![],
                        }
                    }
                }
            } else {
                // No verifier executor available
                VerificationResult {
                    passed: result.success,
                    score: if result.success { 0.75 } else { 0.2 },
                    issues: vec![],
                    suggestions: vec![],
                }
            }
        } else {
            VerificationResult {
                passed: false,
                score: 0.0,
                issues: vec!["No result to verify".to_string()],
                suggestions: vec!["Re-execute the task".to_string()],
            }
        };
        
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            if let Some(st) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
                st.verification_result = Some(verification.clone());
            }
        }
        
        let _ = self.event_tx.send(SwarmEvent::VerificationCompleted {
            task_id,
            subtask_id,
            passed: verification.passed,
            score: verification.score,
        });
    }

    /// Phase 3: Critic review using LLM
    async fn critic_review(&self, task_id: String) {
        // Gather task results for review
        let task_summary = {
            let tasks = self.tasks.read().await;
            if let Some(task) = tasks.get(&task_id) {
                let subtask_summaries: Vec<String> = task.subtasks.iter().map(|st| {
                    let status = format!("{:?}", st.status);
                    let output = st.result.as_ref().map(|r| r.output.clone()).unwrap_or_default();
                    let output_preview = if output.len() > 200 { &output[..200] } else { &output };
                    format!("- {} [{}]: {}", st.description, status, output_preview)
                }).collect();
                Some((task.description.clone(), subtask_summaries.join("\n")))
            } else {
                None
            }
        };
        
        let (issues, suggestions) = if let Some((desc, summary)) = task_summary {
            if let Some(critic) = self.executors.get(&AgentType::Critic) {
                let client = crate::api::AnthropicClient::new(
                    critic.api_key.clone(),
                    critic.model.clone(),
                );
                
                let prompt = format!(
                    r#"Review this task execution and provide feedback. Return JSON only.

Original task: "{}"
Subtask results:
{}

Return: {{"issues": ["issue1", "issue2"], "suggestions": ["suggestion1", "suggestion2"]}}"#,
                    desc, summary
                );
                
                let messages = vec![crate::api::Message {
                    role: "user".to_string(),
                    content: vec![crate::api::ContentBlock::Text { text: prompt }],
                }];
                
                match client.complete(Some(CRITIC_PROMPT.to_string()), messages, None).await {
                    Ok(result) => {
                        let text = result.content.iter()
                            .filter_map(|b| if let crate::api::ContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
                            .collect::<String>();
                        
                        if let Some(start) = text.find('{') {
                            if let Some(end) = text.rfind('}') {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                                    let issues: Vec<String> = parsed.get("issues")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default();
                                    let suggestions: Vec<String> = parsed.get("suggestions")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default();
                                    (issues, suggestions)
                                } else {
                                    (vec![], vec!["Task completed".to_string()])
                                }
                            } else {
                                (vec![], vec!["Task completed".to_string()])
                            }
                        } else {
                            (vec![], vec!["Task completed".to_string()])
                        }
                    }
                    Err(_) => (vec![], vec!["Task completed - critic review unavailable".to_string()]),
                }
            } else {
                (vec![], vec!["Task completed".to_string()])
            }
        } else {
            (vec!["Task not found".to_string()], vec![])
        };
        
        let _ = self.event_tx.send(SwarmEvent::CriticReview {
            task_id,
            issues,
            suggestions,
        });
    }

    /// Update blocked tasks based on dependencies
    async fn update_blocked_tasks(&self, task_id: String) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            let completed_ids: Vec<String> = task.subtasks
                .iter()
                .filter(|st| st.status == SubTaskStatus::Completed)
                .map(|st| st.id.clone())
                .collect();
            
            for st in task.subtasks.iter_mut() {
                if st.status == SubTaskStatus::Blocked {
                    let all_deps_met = st.dependencies.iter().all(|dep| 
                        completed_ids.contains(dep)
                    );
                    if all_deps_met {
                        st.status = SubTaskStatus::Ready;
                    }
                }
            }
        }
    }

    /// Get task status
    pub async fn get_task_status(&self, task_id: &str) -> Option<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).map(|t| t.status)
    }
    
    /// Get full task details including subtasks
    pub async fn get_task_details(&self, task_id: &str) -> Option<ComplexTask> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }
    
    /// List all active tasks
    pub async fn list_active_tasks(&self) -> Vec<(String, TaskStatus)> {
        let tasks = self.tasks.read().await;
        tasks
            .iter()
            .filter(|(_, t)| t.status != TaskStatus::Completed && t.status != TaskStatus::Failed)
            .map(|(id, t)| (id.clone(), t.status))
            .collect()
    }

    /// Get swarm statistics
    pub async fn get_stats(&self) -> SwarmStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Clone swarm for spawning tasks - PROPERLY clones executors
    fn clone_swarm(&self) -> Self {
        let mut executors = HashMap::new();
        for (agent_type, executor) in &self.executors {
            executors.insert(*agent_type, AgentExecutor {
                agent_type: executor.agent_type,
                api_key: executor.api_key.clone(),
                model: executor.model.clone(),
            });
        }
        
        Self {
            tasks: self.tasks.clone(),
            task_queue: self.task_queue.clone(),
            executors,
            event_tx: self.event_tx.clone(),
            config: self.config.clone(),
            stats: self.stats.clone(),
            computer: self.computer.clone(),
            bash: self.bash.clone(),
        }
    }
}

/// Parse coordinates like [300, 400] or (300, 400) or "at 300, 400" from text
fn parse_coordinates_from_text(text: &str) -> Option<(i32, i32)> {
    // Try [x, y] format
    if let Some(start) = text.find('[') {
        if let Some(end) = text[start..].find(']') {
            let inner = &text[start + 1..start + end];
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 2 {
                if let (Ok(x), Ok(y)) = (parts[0].trim().parse::<i32>(), parts[1].trim().parse::<i32>()) {
                    return Some((x, y));
                }
            }
        }
    }
    
    // Try (x, y) format
    if let Some(start) = text.find('(') {
        if let Some(end) = text[start..].find(')') {
            let inner = &text[start + 1..start + end];
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 2 {
                if let (Ok(x), Ok(y)) = (parts[0].trim().parse::<i32>(), parts[1].trim().parse::<i32>()) {
                    return Some((x, y));
                }
            }
        }
    }
    
    None
}

// Supporting structs
#[derive(Debug, Clone)]
struct TaskAnalysis {
    complexity: TaskComplexity,
    steps: Vec<AnalysisStep>,
    parallelizable: bool,
    requires_verification: bool,
    estimated_total_duration_ms: u64,
}

#[derive(Debug, Clone)]
struct AnalysisStep {
    description: String,
    agent_type: AgentType,
    dependencies: Vec<String>,
    estimated_duration_ms: u64,
}

#[derive(Debug, Clone)]
enum TaskComplexity {
    Simple,
    Moderate,
    Complex,
    VeryComplex,
}



// Agent System Prompts
const PLANNER_PROMPT: &str = r#"You are the Planner Agent in an AI Agent Swarm.

Your role is to analyze complex user requests and create detailed execution plans.

Responsibilities:
1. Decompose complex tasks into logical subtasks
2. Identify dependencies between subtasks
3. Estimate complexity and duration
4. Assign appropriate specialized agents to each subtask
5. Identify potential failure points and risks

When planning:
- Break down tasks into atomic, verifiable steps
- Maximize parallel execution where possible
- Include verification steps for critical actions
- Consider error recovery paths

Output format: Structured execution plan with subtasks, dependencies, and agent assignments."#;

const EXECUTOR_PROMPT: &str = r#"You are the Executor Agent in an AI Agent Swarm.

Your role is to execute actions on the computer with high precision.

Capabilities:
- Computer control (screenshots, clicks, typing)
- Bash command execution
- Browser automation via CDP
- File operations

Guidelines:
1. Always verify before destructive actions
2. Take screenshots to confirm state
3. Handle errors gracefully
4. Report exact results, not interpretations
5. Use the most efficient method (bash > computer > browser)

Be precise and methodical. Verify each action succeeded before proceeding."#;

const VERIFIER_PROMPT: &str = r#"You are the Verifier Agent in an AI Agent Swarm.

Your role is to check that executed actions achieved the intended result.

Verification criteria:
1. Did the action complete successfully?
2. Is the system in the expected state?
3. Are there any side effects or issues?
4. Does the output match expectations?

Output: Pass/Fail with confidence score (0.0-1.0), specific issues found, and improvement suggestions."#;

const CRITIC_PROMPT: &str = r#"You are the Critic Agent in an AI Agent Swarm.

Your role is to review completed tasks and identify improvements.

Review aspects:
1. Efficiency: Could this have been done faster?
2. Correctness: Were all requirements met?
3. Robustness: Will this handle edge cases?
4. User experience: Was the interaction smooth?

Provide constructive feedback for future improvements."#;

const RECOVERY_PROMPT: &str = r#"You are the Recovery Agent in an AI Agent Swarm.

Your role is to handle failures and develop recovery strategies.

When errors occur:
1. Analyze the failure type (element not found, timeout, wrong state, etc.)
2. Determine root cause
3. Propose recovery strategies:
   - Retry with wait
   - Alternative approach
   - User intervention
   - Partial completion

Prioritize graceful degradation over complete failure."#;

const COORDINATOR_PROMPT: &str = r#"You are the Coordinator Agent in an AI Agent Swarm.

Your role is to manage communication between agents and ensure smooth workflow.

Responsibilities:
1. Monitor task progress
2. Resolve agent conflicts
3. Allocate resources
4. Escalate to human when needed
5. Maintain task context across agents

Ensure the swarm operates cohesively toward the goal."#;

const SPECIALIST_PROMPT: &str = r#"You are the Specialist Agent in an AI Agent Swarm.

Your role is to handle document generation and data processing tasks.

Expertise:
- Document creation (Word, Excel, PDF, PowerPoint)
- Data visualization (charts, graphs)
- Data transformation (CSV, JSON, XML)
- Report generation
- Code generation

Use Python with appropriate libraries for efficient document processing."#;
