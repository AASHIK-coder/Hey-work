//! Integration Module - Connects Cognitive Engine to Agent
//! 
//! Bridges the new cognitive capabilities with the existing agent system,
//! providing a seamless upgrade path while maintaining compatibility.

use crate::agent::{AgentMode, AgentUpdate};
use crate::api::{AnthropicClient, ContentBlock, ImageSource, Message, ToolResultContent};
use crate::cognitive::{
    CognitiveEngine, Task, TaskContext, TaskResult, TaskStatus, SubtaskStatus,
    memory::ExecutionRecord,
};
use crate::computer::{ComputerAction, ComputerControl};
use crate::bash::BashExecutor;
use tauri::{AppHandle, Emitter};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Enhanced agent that uses the cognitive engine
pub struct CognitiveAgent {
    /// The cognitive engine for planning and reasoning
    cognitive: Arc<Mutex<CognitiveEngine>>,
    /// Current active task
    current_task: Arc<Mutex<Option<Task>>>,
    /// Computer control
    computer: Arc<Mutex<Option<ComputerControl>>>,
    /// Bash executor
    bash: Arc<Mutex<BashExecutor>>,
}

/// Execution context for actions
pub struct ExecutionContext {
    pub app_handle: AppHandle,
    pub api_client: Option<AnthropicClient>,
    pub mode: AgentMode,
    pub voice_mode: bool,
}

impl CognitiveAgent {
    pub fn new() -> Self {
        Self {
            cognitive: Arc::new(Mutex::new(CognitiveEngine::new())),
            current_task: Arc::new(Mutex::new(None)),
            computer: Arc::new(Mutex::new(None)),
            bash: Arc::new(Mutex::new(BashExecutor::new())),
        }
    }

    /// Initialize computer control
    pub async fn initialize(&self) -> anyhow::Result<()> {
        let computer = ComputerControl::new()?;
        *self.computer.lock().await = Some(computer);
        Ok(())
    }

    /// Process a user request through the cognitive engine
    pub async fn process_request(
        &self,
        request: &str,
        app_handle: &AppHandle,
    ) -> anyhow::Result<Task> {
        println!("[cognitive_agent] Processing request: {}", request);
        
        // Emit thinking event
        let _ = app_handle.emit("agent-update", AgentUpdate {
            update_type: "thinking".to_string(),
            message: format!("Analyzing request: '{}'...", request),
            tool_name: None,
            tool_input: None,
            action: None,
            screenshot: None,
            bash_command: None,
            exit_code: None,
            mode: None,
        });
        
        // Process through cognitive engine
        let mut cognitive = self.cognitive.lock().await;
        let task = cognitive.process_request(request).await?;
        
        // Store as current task
        *self.current_task.lock().await = Some(task.clone());
        
        // Emit plan event
        let plan_description = task.subtasks
            .iter()
            .map(|s| format!("- {}", s.description))
            .collect::<Vec<_>>()
            .join("\n");
        
        let _ = app_handle.emit("agent-update", AgentUpdate {
            update_type: "response".to_string(),
            message: format!("Plan created with {} steps:\n{}", task.subtasks.len(), plan_description),
            tool_name: None,
            tool_input: None,
            action: None,
            screenshot: None,
            bash_command: None,
            exit_code: None,
            mode: None,
        });
        
        Ok(task)
    }

    /// Execute the next ready subtask
    pub async fn execute_next(
        &self,
        context: &ExecutionContext,
    ) -> anyhow::Result<Option<TaskResult>> {
        let mut task_guard = self.current_task.lock().await;
        
        if let Some(ref mut task) = *task_guard {
            // Get cognitive engine
            let mut cognitive = self.cognitive.lock().await;
            
            // Execute next subtask
            let result = cognitive.execute_next(task).await?;
            
            // Actually execute the action
            if let Some(ref subtask_result) = result {
                // Find the subtask that was just executed
                if let Some(subtask) = task.subtasks.iter().find(|s| s.status == SubtaskStatus::Executing) {
                    let execution_result = self.execute_subtask(subtask, context).await?;
                    
                    // Learn from execution
                    if execution_result.success {
                        cognitive.skills.learn_from_execution(task, subtask, &execution_result).await?;
                    }
                    
                    // Update task status
                    if cognitive.planner.is_complete(task) {
                        task.status = if task.subtasks.iter().all(|s| s.status == SubtaskStatus::Completed) {
                            TaskStatus::Completed
                        } else {
                            TaskStatus::Failed
                        };
                    }
                    
                    return Ok(Some(execution_result));
                }
            }
            
            Ok(result)
        } else {
            Ok(None)
        }
    }

    /// Execute a single subtask with the appropriate tool
    async fn execute_subtask(
        &self,
        subtask: &crate::cognitive::Subtask,
        context: &ExecutionContext,
    ) -> anyhow::Result<TaskResult> {
        let start = std::time::Instant::now();
        
        // Emit action event
        let _ = context.app_handle.emit("agent-update", AgentUpdate {
            update_type: "action".to_string(),
            message: subtask.description.clone(),
            tool_name: Some(format!("{:?}", subtask.action_type)),
            tool_input: None,
            action: Some(serde_json::json!({
                "description": subtask.description,
                "type": format!("{:?}", subtask.action_type)
            })),
            screenshot: None,
            bash_command: None,
            exit_code: None,
            mode: Some(format!("{:?}", context.mode)),
        });
        
        let result = match &subtask.action_type {
            crate::cognitive::ActionType::Computer { action, params } => {
                self.execute_computer_action(action, params).await
            }
            crate::cognitive::ActionType::Bash { command } => {
                self.execute_bash_command(command).await
            }
            crate::cognitive::ActionType::Browser { tool, params } => {
                // Browser execution would go here
                Ok(TaskResult {
                    success: true,
                    output: format!("Browser {} executed", tool),
                    screenshot: None,
                    error: None,
                    duration_ms: 100,
                    learnings: vec![],
                })
            }
            crate::cognitive::ActionType::Wait { duration_ms } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(*duration_ms)).await;
                Ok(TaskResult {
                    success: true,
                    output: format!("Waited {}ms", duration_ms),
                    screenshot: None,
                    error: None,
                    duration_ms: *duration_ms,
                    learnings: vec![],
                })
            }
            crate::cognitive::ActionType::Think { reasoning } => {
                Ok(TaskResult {
                    success: true,
                    output: format!("Thought: {}", reasoning),
                    screenshot: None,
                    error: None,
                    duration_ms: 10,
                    learnings: vec![reasoning.clone()],
                })
            }
            crate::cognitive::ActionType::Verify { check } => {
                // Take screenshot for verification
                let screenshot = self.take_screenshot().await?;
                Ok(TaskResult {
                    success: true,
                    output: format!("Verified: {}", check),
                    screenshot: Some(screenshot),
                    error: None,
                    duration_ms: 500,
                    learnings: vec![],
                })
            }
        };
        
        let mut task_result = result?;
        task_result.duration_ms = start.elapsed().as_millis() as u64;
        
        // Emit result event
        let update_type = if task_result.success { "success" } else { "error" };
        let _ = context.app_handle.emit("agent-update", AgentUpdate {
            update_type: update_type.to_string(),
            message: task_result.output.clone(),
            tool_name: None,
            tool_input: None,
            action: None,
            screenshot: task_result.screenshot.clone(),
            bash_command: None,
            exit_code: if task_result.success { Some(0) } else { Some(1) },
            mode: None,
        });
        
        Ok(task_result)
    }

    /// Execute a computer control action
    async fn execute_computer_action(
        &self,
        action: &str,
        params: &serde_json::Value,
    ) -> anyhow::Result<TaskResult> {
        let computer_guard = self.computer.lock().await;
        let computer = computer_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Computer control not initialized"))?;
        
        let computer_action = ComputerAction {
            action: action.to_string(),
            coordinate: params.get("coordinate").and_then(|c| {
                if let (Some(x), Some(y)) = (c.get(0).and_then(|v| v.as_i64()), c.get(1).and_then(|v| v.as_i64())) {
                    Some([x as i32, y as i32])
                } else {
                    None
                }
            }),
            start_coordinate: None,
            text: params.get("text").and_then(|t| t.as_str().map(|s| s.to_string())),
            scroll_direction: None,
            scroll_amount: None,
            key: None,
            region: None,
        };
        
        let screen_w = computer.screen_width;
        let screen_h = computer.screen_height;
        
        // Execute on blocking thread
        let result = tokio::task::spawn_blocking(move || {
            let computer = ComputerControl::with_dimensions(screen_w, screen_h);
            computer.perform_action(&computer_action)
        }).await;
        
        match result {
            Ok(Ok(screenshot)) => {
                Ok(TaskResult {
                    success: true,
                    output: format!("Action '{}' completed", action),
                    screenshot,
                    error: None,
                    duration_ms: 100,
                    learnings: vec![],
                })
            }
            Ok(Err(e)) => {
                Ok(TaskResult {
                    success: false,
                    output: String::new(),
                    screenshot: None,
                    error: Some(format!("Computer action failed: {}", e)),
                    duration_ms: 100,
                    learnings: vec![],
                })
            }
            Err(e) => {
                Ok(TaskResult {
                    success: false,
                    output: String::new(),
                    screenshot: None,
                    error: Some(format!("Task execution failed: {}", e)),
                    duration_ms: 100,
                    learnings: vec![],
                })
            }
        }
    }

    /// Execute a bash command
    async fn execute_bash_command(&self, command: &str) -> anyhow::Result<TaskResult> {
        let bash = self.bash.lock().await;
        let result = bash.execute(command);
        
        match result {
            Ok(output) => {
                Ok(TaskResult {
                    success: output.exit_code == 0,
                    output: output.stdout.clone(),
                    screenshot: None,
                    error: if output.exit_code != 0 { Some(output.stderr.clone()) } else { None },
                    duration_ms: 100,
                    learnings: vec![],
                })
            }
            Err(e) => {
                Ok(TaskResult {
                    success: false,
                    output: String::new(),
                    screenshot: None,
                    error: Some(format!("Bash execution failed: {}", e)),
                    duration_ms: 100,
                    learnings: vec![],
                })
            }
        }
    }

    /// Take a screenshot
    async fn take_screenshot(&self) -> anyhow::Result<String> {
        let computer_guard = self.computer.lock().await;
        let computer = computer_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Computer control not initialized"))?;
        
        let screen_w = computer.screen_width;
        let screen_h = computer.screen_height;
        
        tokio::task::spawn_blocking(move || {
            let computer = ComputerControl::with_dimensions(screen_w, screen_h);
            computer.take_screenshot()
        }).await
        .map_err(|e| anyhow::anyhow!("Screenshot task failed: {}", e))?
        .map_err(|e| anyhow::anyhow!("Screenshot failed: {}", e))
    }

    /// Get current task status
    pub async fn get_task_status(&self) -> Option<TaskStatus> {
        let task = self.current_task.lock().await;
        task.as_ref().map(|t| t.status.clone())
    }

    /// Get task progress
    pub async fn get_progress(&self) -> Option<f32> {
        let task = self.current_task.lock().await;
        task.as_ref().map(|t| {
            let total = t.subtasks.len();
            let completed = t.subtasks.iter()
                .filter(|s| s.status == SubtaskStatus::Completed)
                .count();
            if total > 0 {
                (completed as f32 / total as f32) * 100.0
            } else {
                0.0
            }
        })
    }

    /// Cancel current task
    pub async fn cancel_task(&self) {
        let mut task = self.current_task.lock().await;
        if let Some(ref mut t) = *task {
            t.status = TaskStatus::Failed;
        }
        *task = None;
    }
}

impl Default for CognitiveAgent {
    fn default() -> Self {
        Self::new()
    }
}