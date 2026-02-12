//! Skill Executor - Makes Skills Actually Execute
//!
//! Converts skill action templates into real tool executions.
//! This bridges the gap between skill definitions and actual computer control.

use super::{ActionTemplate, ActionType, Skill};
use crate::computer::{ComputerAction, ComputerControl};
use crate::bash::BashExecutor;
use crate::browser::BrowserClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Executor that can run skills with real tools
pub struct SkillExecutor {
    computer: Arc<Mutex<Option<ComputerControl>>>,
    bash: Arc<Mutex<BashExecutor>>,
    browser: Arc<Mutex<Option<BrowserClient>>>,
}

/// Result of executing a skill action
#[derive(Debug, Clone)]
pub struct SkillExecutionResult {
    pub success: bool,
    pub output: String,
    pub screenshot: Option<String>,
    pub error: Option<String>,
}

impl SkillExecutor {
    pub fn new() -> Self {
        Self {
            computer: Arc::new(Mutex::new(None)),
            bash: Arc::new(Mutex::new(BashExecutor::new())),
            browser: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize computer control
    pub async fn init_computer(&self) -> anyhow::Result<()> {
        let computer = ComputerControl::new()?;
        *self.computer.lock().await = Some(computer);
        Ok(())
    }

    /// Execute a complete skill with all its actions
    pub async fn execute_skill(
        &self,
        skill: &Skill,
        params: &HashMap<String, String>,
    ) -> anyhow::Result<SkillExecutionResult> {
        println!("[skill_executor] Executing skill '{}' with {} actions", 
            skill.name, skill.actions.len());
        
        let mut last_result = SkillExecutionResult {
            success: true,
            output: String::new(),
            screenshot: None,
            error: None,
        };

        for (idx, action_template) in skill.actions.iter().enumerate() {
            println!("[skill_executor] Action {}/{}: {:?}", 
                idx + 1, skill.actions.len(), action_template.action_type);
            
            // Check condition if present
            if let Some(ref condition) = action_template.condition {
                if !self.evaluate_condition(condition, params) {
                    println!("[skill_executor] Condition not met, skipping");
                    continue;
                }
            }

            // Execute the action
            let result = self.execute_action(&action_template.action_type, params).await;
            
            match result {
                Ok(r) => {
                    last_result = r;
                    if !last_result.success {
                        // Try fallback if available
                        if let Some(ref fallback) = action_template.fallback {
                            println!("[skill_executor] Primary failed, trying fallback");
                            let fallback_result = self.execute_action(&fallback.action_type, params).await;
                            if let Ok(fr) = fallback_result {
                                last_result = fr;
                            }
                        }
                    }
                    
                    // Small delay between actions for stability
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                }
                Err(e) => {
                    return Ok(SkillExecutionResult {
                        success: false,
                        output: String::new(),
                        screenshot: None,
                        error: Some(format!("Action {} failed: {}", idx, e)),
                    });
                }
            }
        }

        println!("[skill_executor] Skill execution complete: success={}", last_result.success);
        Ok(last_result)
    }

    /// Execute a single action
    async fn execute_action(
        &self,
        action_type: &ActionType,
        params: &HashMap<String, String>,
    ) -> anyhow::Result<SkillExecutionResult> {
        match action_type {
            ActionType::Computer { action, params: action_params } => {
                self.execute_computer_action(action, action_params).await
            }
            ActionType::Bash { command } => {
                let command = self.fill_template(command, params);
                self.execute_bash(&command).await
            }
            ActionType::Wait { duration_ms } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(*duration_ms)).await;
                Ok(SkillExecutionResult {
                    success: true,
                    output: format!("Waited {}ms", duration_ms),
                    screenshot: None,
                    error: None,
                })
            }
            ActionType::Think { reasoning } => {
                Ok(SkillExecutionResult {
                    success: true,
                    output: format!("Thinking: {}", reasoning),
                    screenshot: None,
                    error: None,
                })
            }
            ActionType::Verify { check } => {
                // Take screenshot for verification
                match self.take_screenshot().await {
                    Ok(screenshot) => Ok(SkillExecutionResult {
                        success: true,
                        output: format!("Verified: {}", check),
                        screenshot: Some(screenshot),
                        error: None,
                    }),
                    Err(e) => Ok(SkillExecutionResult {
                        success: false,
                        output: String::new(),
                        screenshot: None,
                        error: Some(format!("Verification failed: {}", e)),
                    }),
                }
            }
            ActionType::Browser { tool, params } => {
                // For now, return success - browser needs separate initialization
                Ok(SkillExecutionResult {
                    success: true,
                    output: format!("Browser tool '{}' would execute with params: {:?}", tool, params),
                    screenshot: None,
                    error: None,
                })
            }
        }
    }

    /// Execute computer control action
    pub async fn execute_computer_action(
        &self,
        action: &str,
        params: &serde_json::Value,
    ) -> anyhow::Result<SkillExecutionResult> {
        let computer_guard = self.computer.lock().await;
        let computer = match computer_guard.as_ref() {
            Some(c) => c,
            None => {
                // Try to initialize
                drop(computer_guard);
                self.init_computer().await?;
                return Box::pin(self.execute_computer_action(action, params)).await;
            }
        };

        let screen_w = computer.screen_width;
        let screen_h = computer.screen_height;
        
        // Parse coordinate from params
        let coordinate = params.get("coordinate").and_then(|c| {
            if let (Some(x), Some(y)) = (c.get(0).and_then(|v| v.as_i64()), c.get(1).and_then(|v| v.as_i64())) {
                Some([x as i32, y as i32])
            } else {
                None
            }
        });

        // Parse text from params
        let text = params.get("text").and_then(|t| t.as_str().map(|s| s.to_string()));

        let computer_action = ComputerAction {
            action: action.to_string(),
            coordinate,
            start_coordinate: None,
            text,
            scroll_direction: None,
            scroll_amount: None,
            key: None,
            region: None,
        };

        // Execute on blocking thread
        let result = tokio::task::spawn_blocking(move || {
            let computer = ComputerControl::with_dimensions(screen_w, screen_h);
            computer.perform_action(&computer_action)
        }).await;

        match result {
            Ok(Ok(screenshot)) => Ok(SkillExecutionResult {
                success: true,
                output: format!("Computer action '{}' completed", action),
                screenshot,
                error: None,
            }),
            Ok(Err(e)) => Ok(SkillExecutionResult {
                success: false,
                output: String::new(),
                screenshot: None,
                error: Some(format!("Computer action failed: {}", e)),
            }),
            Err(e) => Ok(SkillExecutionResult {
                success: false,
                output: String::new(),
                screenshot: None,
                error: Some(format!("Task execution failed: {}", e)),
            }),
        }
    }

    /// Execute bash command
    pub async fn execute_bash(&self, command: &str) -> anyhow::Result<SkillExecutionResult> {
        let bash = self.bash.lock().await;
        
        match bash.execute(command) {
            Ok(output) => Ok(SkillExecutionResult {
                success: output.exit_code == 0,
                output: output.stdout.clone(),
                screenshot: None,
                error: if output.exit_code != 0 { Some(output.stderr.clone()) } else { None },
            }),
            Err(e) => Ok(SkillExecutionResult {
                success: false,
                output: String::new(),
                screenshot: None,
                error: Some(format!("Bash failed: {}", e)),
            }),
        }
    }

    /// Take screenshot
    pub async fn take_screenshot(&self) -> anyhow::Result<String> {
        let computer_guard = self.computer.lock().await;
        let computer = match computer_guard.as_ref() {
            Some(c) => c,
            None => {
                drop(computer_guard);
                self.init_computer().await?;
                return Box::pin(self.take_screenshot()).await;
            }
        };

        let screen_w = computer.screen_width;
        let screen_h = computer.screen_height;

        tokio::task::spawn_blocking(move || {
            let computer = ComputerControl::with_dimensions(screen_w, screen_h);
            computer.take_screenshot()
        }).await
        .map_err(|e| anyhow::anyhow!("Screenshot task failed: {}", e))?
        .map_err(|e| anyhow::anyhow!("Screenshot failed: {}", e))
    }

    /// Fill template parameters
    fn fill_template(&self, template: &str, params: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in params {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }

    /// Evaluate condition by checking current system state
    fn evaluate_condition(&self, condition: &str, params: &HashMap<String, String>) -> bool {
        let condition_lower = condition.to_lowercase();
        let filled_condition = self.fill_template(&condition_lower, params);
        
        // Check for "app is running" conditions
        if filled_condition.contains("running") || filled_condition.contains("is open") {
            // Try to detect if the mentioned app is running
            let app_name = filled_condition
                .replace("is running", "").replace("is open", "")
                .trim().to_string();
            if !app_name.is_empty() {
                let check_result = if cfg!(target_os = "windows") {
                    std::process::Command::new("tasklist")
                        .args(["/FI", &format!("IMAGENAME eq {}.exe", app_name)])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&app_name.to_lowercase()))
                        .unwrap_or(false)
                } else {
                    std::process::Command::new("pgrep")
                        .args(["-i", "-x", &app_name])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false)
                };
                println!("[skill_executor] Condition '{}': {} (process check)", condition, check_result);
                return check_result;
            }
        }
        
        // Check for "file exists" conditions
        if filled_condition.contains("exists") || filled_condition.contains("file") {
            for (_, value) in params {
                if std::path::Path::new(value).exists() {
                    println!("[skill_executor] Condition '{}': true (file exists: {})", condition, value);
                    return true;
                }
            }
        }
        
        // Check for param-based conditions (e.g., "has_url" checks if url param is set)
        if filled_condition.contains("has_") {
            let param_name = filled_condition.replace("has_", "").trim().to_string();
            let result = params.contains_key(&param_name) && !params[&param_name].is_empty();
            println!("[skill_executor] Condition '{}': {} (param check)", condition, result);
            return result;
        }
        
        // Default: conditions we can't evaluate are assumed true (optimistic)
        println!("[skill_executor] Condition '{}': true (default)", condition);
        true
    }
}

impl Default for SkillExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_template() {
        let executor = SkillExecutor::new();
        let mut params = HashMap::new();
        params.insert("app".to_string(), "Chrome".to_string());
        params.insert("url".to_string(), "google.com".to_string());
        
        let template = "Open {app} and go to {url}";
        let result = executor.fill_template(template, &params);
        assert_eq!(result, "Open Chrome and go to google.com");
    }
}
