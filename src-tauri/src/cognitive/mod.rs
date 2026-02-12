//! Cognitive Engine - Advanced AI Agent Architecture
//! 
//! This module provides human-like cognitive capabilities:
//! - Planning: Break down complex tasks into manageable steps
//! - Memory: Learn from past experiences and retrieve relevant context
//! - Skills: Reusable patterns of actions for common tasks
//! - Reasoning: Chain-of-thought and systematic problem solving
//! - Self-correction: Detect failures and try alternative approaches

pub mod planner;
pub mod memory;
pub mod skills;
pub mod reasoner;
pub mod context;
pub mod correction;
pub mod agent_swarm;
pub mod skill_executor;
pub mod integration;

use crate::storage::Usage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// High-level task representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub goal: String,
    pub subtasks: Vec<Subtask>,
    pub context: TaskContext,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
}

/// Individual subtask with dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    pub id: String,
    pub description: String,
    pub action_type: ActionType,
    pub dependencies: Vec<String>,
    pub status: SubtaskStatus,
    pub retry_count: u32,
    pub max_retries: u32,
    pub result: Option<TaskResult>,
}

/// Types of actions the agent can perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    Computer { action: String, params: serde_json::Value },
    Browser { tool: String, params: serde_json::Value },
    Bash { command: String },
    Think { reasoning: String },
    Wait { duration_ms: u64 },
    Verify { check: String },
}

/// Task execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Planning,
    Executing,
    Verifying,
    Completed,
    Failed,
    NeedsUserInput,
}

/// Subtask execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubtaskStatus {
    Pending,
    Ready,      // Dependencies satisfied
    Executing,
    Completed,
    Failed,
    Retrying,
}

/// Result of a task/subtask execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskResult {
    pub success: bool,
    pub output: String,
    pub screenshot: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub learnings: Vec<String>,
}

/// Context for task execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskContext {
    pub user_preferences: HashMap<String, String>,
    pub app_state: HashMap<String, serde_json::Value>,
    pub relevant_memories: Vec<Memory>,
    pub available_skills: Vec<String>,
    pub constraints: Vec<String>,
}

/// Memory entry for learned experiences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub task_pattern: String,
    pub actions: Vec<String>,
    pub success_rate: f32,
    pub usage_count: u32,
    pub created_at: DateTime<Utc>,
    pub embedding: Option<Vec<f32>>, // For semantic search
}

/// A reusable skill (learned pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pattern: TaskPattern,
    pub actions: Vec<ActionTemplate>,
    pub success_rate: f32,
    pub total_uses: u32,
    pub avg_execution_time_ms: u64,
}

/// Pattern matching for skills
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPattern {
    pub intent_keywords: Vec<String>,
    pub app_context: Option<String>,
    pub required_elements: Vec<String>,
}

/// Template for skill actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTemplate {
    pub action_type: ActionType,
    pub condition: Option<String>, // When to use this action
    pub fallback: Option<Box<ActionTemplate>>, // What to do if this fails
}

/// Cognitive engine that orchestrates all capabilities
pub struct CognitiveEngine {
    pub planner: planner::Planner,
    pub memory: memory::MemorySystem,
    pub skills: skills::SkillLibrary,
    pub reasoner: reasoner::Reasoner,
    pub context: context::ContextManager,
    pub correction: correction::SelfCorrection,
}

impl CognitiveEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            planner: planner::Planner::new(),
            memory: memory::MemorySystem::new(),
            skills: skills::SkillLibrary::new(),
            reasoner: reasoner::Reasoner::new(),
            context: context::ContextManager::new(),
            correction: correction::SelfCorrection::new(),
        };
        
        // Initialize memory persistence
        if let Err(e) = engine.memory.init() {
            println!("[cognitive] Warning: Failed to initialize memory: {}", e);
        }
        
        engine
    }

    /// Initialize with memory persistence
    pub fn init(&mut self) -> anyhow::Result<()> {
        self.memory.init()?;
        println!("[cognitive] Engine initialized with memory persistence");
        Ok(())
    }

    /// Process a high-level user request
    pub async fn process_request(&mut self, request: &str) -> anyhow::Result<Task> {
        // 1. Analyze the request with reasoning
        let analysis = self.reasoner.analyze_request(request).await?;
        
        // 2. Retrieve relevant memories
        let memories = self.memory.search_relevant(request).await?;
        
        // 3. Check for applicable skills
        let skills = self.skills.find_matching_skills(&analysis.intent).await?;
        
        // 4. Create task context
        let context = TaskContext {
            relevant_memories: memories,
            available_skills: skills.iter().map(|s| s.name.clone()).collect(),
            ..Default::default()
        };
        
        // 5. Plan the task
        let request_analysis = planner::RequestAnalysis::from_task_analysis(&analysis);
        let task = self.planner.create_plan(request, &request_analysis, &context).await?;
        
        // 6. Store in memory
        self.memory.store_task_intent(request, &task).await?;
        
        Ok(task)
    }

    /// Execute the next ready subtask
    pub async fn execute_next(&mut self, task: &mut Task) -> anyhow::Result<Option<TaskResult>> {
        // Find the index of the next ready subtask first
        let next_idx = task.subtasks.iter().position(|s| s.status == SubtaskStatus::Pending);
        
        if let Some(idx) = next_idx {
            // Check dependencies are satisfied
            let deps_satisfied = {
                let subtask = &task.subtasks[idx];
                let completed_ids: std::collections::HashSet<String> = task.subtasks
                    .iter()
                    .filter(|s| s.status == SubtaskStatus::Completed)
                    .map(|s| s.id.clone())
                    .collect();
                subtask.dependencies.iter().all(|dep_id| completed_ids.contains(dep_id))
            };
            
            if !deps_satisfied {
                return Ok(None);
            }
            
            // Mark as executing
            task.subtasks[idx].status = SubtaskStatus::Executing;
            
            // Check if we have a skill for this
            if let Some(skill) = self.skills.get_skill_for_subtask(&task.subtasks[idx]) {
                let result = self.execute_with_skill(&mut task.subtasks[idx], &skill).await?;
                return Ok(result);
            }
            
            // Execute with self-correction capability
            let result = self.correction.execute_with_retry(&mut task.subtasks[idx]).await?;
            
            // Update subtask status
            task.subtasks[idx].status = if result.success {
                SubtaskStatus::Completed
            } else if task.subtasks[idx].retry_count < task.subtasks[idx].max_retries {
                SubtaskStatus::Retrying
            } else {
                SubtaskStatus::Failed
            };
            task.subtasks[idx].result = Some(result.clone());
            
            // Learn from the execution
            if result.success {
                self.skills.learn_from_execution(task, &task.subtasks[idx], &result).await?;
            }
            
            return Ok(Some(result));
        }
        
        Ok(None)
    }

    async fn execute_with_skill(&self, subtask: &mut Subtask, skill: &Skill) -> anyhow::Result<Option<TaskResult>> {
        // Execute using learned skill patterns
        println!("[cognitive] Executing with skill: {}", skill.name);
        
        let start_time = std::time::Instant::now();
        
        // Use SkillExecutor to actually execute the skill's actions
        let executor = skill_executor::SkillExecutor::new();
        
        // Build parameters from subtask context
        let mut params = std::collections::HashMap::new();
        params.insert("description".to_string(), subtask.description.clone());
        
        // Execute the skill
        match executor.execute_skill(skill, &params).await {
            Ok(exec_result) => {
                let output = if exec_result.output.is_empty() {
                    format!("Executed skill '{}' with {} actions", skill.name, skill.actions.len())
                } else {
                    exec_result.output
                };
                let result = TaskResult {
                    success: exec_result.success,
                    output,
                    screenshot: exec_result.screenshot,
                    error: exec_result.error,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    learnings: vec![format!("Successfully used skill: {}", skill.name)],
                };
                Ok(Some(result))
            }
            Err(e) => {
                let result = TaskResult {
                    success: false,
                    output: format!("Skill execution failed: {}", e),
                    screenshot: None,
                    error: Some(e.to_string()),
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    learnings: vec![format!("Skill '{}' failed: {}", skill.name, e)],
                };
                Ok(Some(result))
            }
        }
    }
}

impl Default for CognitiveEngine {
    fn default() -> Self {
        Self::new()
    }
}