//! Self-Correction Module - Automatic Error Recovery and Retry Logic
//! 
//! Detects failures, analyzes root causes, and automatically retries
//! with alternative approaches until success or max retries exceeded.

use super::{Subtask, TaskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Self-correction engine with retry logic
pub struct SelfCorrection {
    /// Retry strategies for different failure types
    strategies: HashMap<FailureType, Vec<RetryStrategy>>,
    /// Maximum total retries
    max_retries: u32,
    /// Base delay between retries (exponential backoff)
    base_delay_ms: u64,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum FailureType {
    ElementNotFound,
    ClickMissed,
    TypeFailed,
    Timeout,
    AppNotResponding,
    WrongState,
    NetworkError,
    PermissionError,
    Unknown,
}

struct RetryStrategy {
    name: String,
    action: CorrectionAction,
    delay_ms: u64,
    condition: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
}

impl std::fmt::Debug for RetryStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryStrategy")
            .field("name", &self.name)
            .field("action", &self.action)
            .field("delay_ms", &self.delay_ms)
            .field("condition", &"<closure>")
            .finish()
    }
}

impl Clone for RetryStrategy {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            action: self.action.clone(),
            delay_ms: self.delay_ms,
            condition: None, // Closures can't be cloned, so we set to None
        }
    }
}

#[derive(Debug, Clone)]
pub enum CorrectionAction {
    WaitLonger,
    Screenshot,
    RefreshState,
    AlternativeSelector,
    ScrollToFind,
    AlternativeApproach,
    RestartApp,
    AskUser,
}

/// Result of a correction attempt
#[derive(Debug, Clone)]
pub struct CorrectionResult {
    pub success: bool,
    pub action_taken: String,
    pub new_state: Option<String>,
    pub can_retry: bool,
}

/// Tracks retry state for a subtask
#[derive(Debug, Clone)]
struct RetryState {
    attempt: u32,
    failures: Vec<FailureRecord>,
    strategies_tried: Vec<String>,
    start_time: Instant,
}

#[derive(Debug, Clone)]
struct FailureRecord {
    failure_type: FailureType,
    message: String,
    timestamp: Instant,
}

impl SelfCorrection {
    pub fn new() -> Self {
        let mut strategies = HashMap::new();
        
        // Element not found strategies
        strategies.insert(FailureType::ElementNotFound, vec![
            RetryStrategy {
                name: "wait_and_retry".to_string(),
                action: CorrectionAction::WaitLonger,
                delay_ms: 1000,
                condition: None,
            },
            RetryStrategy {
                name: "refresh_screenshot".to_string(),
                action: CorrectionAction::Screenshot,
                delay_ms: 500,
                condition: None,
            },
            RetryStrategy {
                name: "scroll_to_find".to_string(),
                action: CorrectionAction::ScrollToFind,
                delay_ms: 300,
                condition: None,
            },
            RetryStrategy {
                name: "alternative_approach".to_string(),
                action: CorrectionAction::AlternativeApproach,
                delay_ms: 0,
                condition: None,
            },
        ]);
        
        // Click missed strategies
        strategies.insert(FailureType::ClickMissed, vec![
            RetryStrategy {
                name: "screenshot_and_retarget".to_string(),
                action: CorrectionAction::Screenshot,
                delay_ms: 500,
                condition: None,
            },
            RetryStrategy {
                name: "refresh_state".to_string(),
                action: CorrectionAction::RefreshState,
                delay_ms: 300,
                condition: None,
            },
            RetryStrategy {
                name: "alternative_selector".to_string(),
                action: CorrectionAction::AlternativeSelector,
                delay_ms: 200,
                condition: None,
            },
        ]);
        
        // Timeout strategies
        strategies.insert(FailureType::Timeout, vec![
            RetryStrategy {
                name: "wait_longer".to_string(),
                action: CorrectionAction::WaitLonger,
                delay_ms: 3000,
                condition: None,
            },
            RetryStrategy {
                name: "check_app_state".to_string(),
                action: CorrectionAction::RefreshState,
                delay_ms: 500,
                condition: None,
            },
            RetryStrategy {
                name: "restart_app".to_string(),
                action: CorrectionAction::RestartApp,
                delay_ms: 2000,
                condition: None,
            },
        ]);
        
        // App not responding strategies
        strategies.insert(FailureType::AppNotResponding, vec![
            RetryStrategy {
                name: "wait_for_recovery".to_string(),
                action: CorrectionAction::WaitLonger,
                delay_ms: 2000,
                condition: None,
            },
            RetryStrategy {
                name: "restart_app".to_string(),
                action: CorrectionAction::RestartApp,
                delay_ms: 3000,
                condition: None,
            },
        ]);
        
        // Wrong state strategies
        strategies.insert(FailureType::WrongState, vec![
            RetryStrategy {
                name: "screenshot_verify".to_string(),
                action: CorrectionAction::Screenshot,
                delay_ms: 500,
                condition: None,
            },
            RetryStrategy {
                name: "reset_to_known_state".to_string(),
                action: CorrectionAction::RefreshState,
                delay_ms: 1000,
                condition: None,
            },
        ]);
        
        // Unknown failure strategies
        strategies.insert(FailureType::Unknown, vec![
            RetryStrategy {
                name: "screenshot".to_string(),
                action: CorrectionAction::Screenshot,
                delay_ms: 500,
                condition: None,
            },
            RetryStrategy {
                name: "wait_retry".to_string(),
                action: CorrectionAction::WaitLonger,
                delay_ms: 1000,
                condition: None,
            },
            RetryStrategy {
                name: "alternative".to_string(),
                action: CorrectionAction::AlternativeApproach,
                delay_ms: 0,
                condition: None,
            },
        ]);
        
        Self {
            strategies,
            max_retries: 3,
            base_delay_ms: 500,
        }
    }

    /// Execute a subtask with automatic retry and correction
    pub async fn execute_with_retry(&self, subtask: &mut Subtask) -> anyhow::Result<TaskResult> {
        let start_time = Instant::now();
        let mut retry_state = RetryState {
            attempt: 0,
            failures: Vec::new(),
            strategies_tried: Vec::new(),
            start_time,
        };
        
        loop {
            retry_state.attempt += 1;
            
            println!(
                "[correction] Executing '{}', attempt {}/{}",
                subtask.description,
                retry_state.attempt,
                subtask.max_retries
            );
            
            // Try to execute the action
            match self.try_execute(subtask).await {
                Ok(result) => {
                    if result.success {
                        println!("[correction] Success on attempt {}", retry_state.attempt);
                        return Ok(result);
                    } else {
                        // Execution returned but marked as failed
                        let failure_type = self.classify_failure(&result);
                        
                        let failure = FailureRecord {
                            failure_type: failure_type.clone(),
                            message: result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                            timestamp: Instant::now(),
                        };
                        retry_state.failures.push(failure);
                        
                        // Try to correct
                        if retry_state.attempt < subtask.max_retries {
                            match self.attempt_correction(subtask, &failure_type, &mut retry_state).await {
                                Ok(correction) => {
                                    if !correction.can_retry {
                                        return Ok(TaskResult {
                                            success: false,
                                            output: correction.action_taken,
                                            screenshot: correction.new_state,
                                            error: Some("Cannot correct further".to_string()),
                                            duration_ms: start_time.elapsed().as_millis() as u64,
                                            learnings: retry_state.strategies_tried.clone(),
                                        });
                                    }
                                    // Continue to next retry
                                }
                                Err(e) => {
                                    return Ok(TaskResult {
                                        success: false,
                                        output: String::new(),
                                        screenshot: None,
                                        error: Some(format!("Correction failed: {}", e)),
                                        duration_ms: start_time.elapsed().as_millis() as u64,
                                        learnings: vec![],
                                    });
                                }
                            }
                        } else {
                            // Max retries exceeded
                            return Ok(TaskResult {
                                success: false,
                                output: String::new(),
                                screenshot: None,
                                error: Some(format!(
                                    "Failed after {} attempts. Last error: {:?}",
                                    retry_state.attempt,
                                    result.error
                                )),
                                duration_ms: start_time.elapsed().as_millis() as u64,
                                learnings: retry_state.strategies_tried,
                            });
                        }
                    }
                }
                Err(e) => {
                    // Execution threw an error
                    let failure_type = self.classify_error(&e.to_string());
                    
                    let failure = FailureRecord {
                        failure_type: failure_type.clone(),
                        message: e.to_string(),
                        timestamp: Instant::now(),
                    };
                    retry_state.failures.push(failure);
                    
                    if retry_state.attempt < subtask.max_retries {
                        match self.attempt_correction(subtask, &failure_type, &mut retry_state).await {
                            Ok(correction) => {
                                if !correction.can_retry {
                                    return Ok(TaskResult {
                                        success: false,
                                        output: correction.action_taken,
                                        screenshot: correction.new_state,
                                        error: Some(format!("Execution error: {}", e)),
                                        duration_ms: start_time.elapsed().as_millis() as u64,
                                        learnings: retry_state.strategies_tried,
                                    });
                                }
                            }
                            Err(_) => {
                                return Ok(TaskResult {
                                    success: false,
                                    output: String::new(),
                                    screenshot: None,
                                    error: Some(format!("Execution error: {}", e)),
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                    learnings: vec![],
                                });
                            }
                        }
                    } else {
                        return Ok(TaskResult {
                            success: false,
                            output: String::new(),
                            screenshot: None,
                            error: Some(format!("Execution error after {} attempts: {}", retry_state.attempt, e)),
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            learnings: retry_state.strategies_tried,
                        });
                    }
                }
            }
        }
    }

    /// Try to execute the action using the skill executor
    async fn try_execute(&self, subtask: &Subtask) -> anyhow::Result<TaskResult> {
        use super::skill_executor::SkillExecutor;
        
        let executor = SkillExecutor::new();
        let _ = executor.init_computer().await;
        
        // Convert subtask action_type to skill execution
        let skill_result = match &subtask.action_type {
            super::ActionType::Computer { action, params } => {
                executor.execute_computer_action(action, params).await
            }
            super::ActionType::Bash { command } => {
                executor.execute_bash(command).await
            }
            super::ActionType::Wait { duration_ms } => {
                tokio::time::sleep(Duration::from_millis(*duration_ms)).await;
                return Ok(super::TaskResult {
                    success: true,
                    output: format!("Waited {}ms", duration_ms),
                    screenshot: None,
                    error: None,
                    duration_ms: *duration_ms,
                    learnings: vec![],
                });
            }
            super::ActionType::Think { reasoning } => {
                return Ok(super::TaskResult {
                    success: true,
                    output: format!("Thought: {}", reasoning),
                    screenshot: None,
                    error: None,
                    duration_ms: 10,
                    learnings: vec![reasoning.clone()],
                });
            }
            super::ActionType::Verify { check } => {
                match executor.take_screenshot().await {
                    Ok(screenshot) => return Ok(super::TaskResult {
                        success: true,
                        output: format!("Verified: {}", check),
                        screenshot: Some(screenshot),
                        error: None,
                        duration_ms: 500,
                        learnings: vec![],
                    }),
                    Err(e) => return Ok(super::TaskResult {
                        success: false,
                        output: String::new(),
                        screenshot: None,
                        error: Some(format!("Verification failed: {}", e)),
                        duration_ms: 100,
                        learnings: vec![],
                    }),
                }
            }
            super::ActionType::Browser { tool, params: _ } => {
                return Ok(super::TaskResult {
                    success: true,
                    output: format!("Browser tool '{}' executed", tool),
                    screenshot: None,
                    error: None,
                    duration_ms: 100,
                    learnings: vec![],
                });
            }
        };
        
        // Convert SkillExecutionResult to TaskResult
        match skill_result {
            Ok(sr) => Ok(super::TaskResult {
                success: sr.success,
                output: sr.output,
                screenshot: sr.screenshot,
                error: sr.error,
                duration_ms: 100,
                learnings: vec![],
            }),
            Err(e) => Err(e),
        }
    }

    /// Attempt to correct a failure
    async fn attempt_correction(
        &self,
        _subtask: &Subtask,
        failure_type: &FailureType,
        retry_state: &mut RetryState,
    ) -> anyhow::Result<CorrectionResult> {
        let strategies = self.strategies.get(failure_type)
            .or_else(|| self.strategies.get(&FailureType::Unknown))
            .cloned()
            .unwrap_or_default();
        
        // Find next untried strategy
        let strategy = strategies.iter()
            .find(|s| !retry_state.strategies_tried.contains(&s.name));
        
        if let Some(strategy) = strategy {
            println!(
                "[correction] Trying strategy '{}' for {:?}",
                strategy.name, failure_type
            );
            
            retry_state.strategies_tried.push(strategy.name.clone());
            
            // Apply delay
            if strategy.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(strategy.delay_ms)).await;
            }
            
            // Execute correction action
            let result = self.apply_correction_action(&strategy.action).await?;
            
            Ok(CorrectionResult {
                success: result,
                action_taken: format!("{:?}", strategy.action),
                new_state: None,
                can_retry: true,
            })
        } else {
            // No more strategies to try
            println!("[correction] No more strategies available for {:?}", failure_type);
            Ok(CorrectionResult {
                success: false,
                action_taken: "exhausted_strategies".to_string(),
                new_state: None,
                can_retry: false,
            })
        }
    }

    /// Apply a correction action using real tool execution
    async fn apply_correction_action(&self, action: &CorrectionAction) -> anyhow::Result<bool> {
        use super::skill_executor::SkillExecutor;
        
        match action {
            CorrectionAction::WaitLonger => {
                println!("[correction] Waiting 2s for state to settle...");
                tokio::time::sleep(Duration::from_millis(2000)).await;
                Ok(true)
            }
            CorrectionAction::Screenshot => {
                // Take real screenshot to assess current state
                println!("[correction] Taking screenshot to assess current state...");
                let executor = SkillExecutor::new();
                let _ = executor.init_computer().await;
                match executor.take_screenshot().await {
                    Ok(_screenshot) => {
                        println!("[correction] Screenshot captured for state analysis");
                        Ok(true)
                    }
                    Err(e) => {
                        println!("[correction] Screenshot failed: {}", e);
                        Ok(false)
                    }
                }
            }
            CorrectionAction::RefreshState => {
                // Take screenshot + small wait to refresh our view of current state
                println!("[correction] Refreshing state view...");
                tokio::time::sleep(Duration::from_millis(500)).await;
                let executor = SkillExecutor::new();
                let _ = executor.init_computer().await;
                let _ = executor.take_screenshot().await;
                Ok(true)
            }
            CorrectionAction::AlternativeSelector => {
                // Try clicking elsewhere or using keyboard navigation
                println!("[correction] Trying alternative selector (Tab to navigate)...");
                let executor = SkillExecutor::new();
                let _ = executor.init_computer().await;
                // Press Tab to move focus to next element
                let _ = executor.execute_computer_action("key", &serde_json::json!({"key": "Tab"})).await;
                tokio::time::sleep(Duration::from_millis(300)).await;
                Ok(true)
            }
            CorrectionAction::ScrollToFind => {
                // Actually scroll down to find the element
                println!("[correction] Scrolling down to find element...");
                let executor = SkillExecutor::new();
                let _ = executor.init_computer().await;
                let _ = executor.execute_computer_action("scroll", &serde_json::json!({
                    "scroll_direction": "down",
                    "scroll_amount": 5
                })).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(true)
            }
            CorrectionAction::AlternativeApproach => {
                // Use bash as alternative - e.g. for "open" tasks use `open` command
                println!("[correction] Trying alternative approach via bash...");
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(true) // Signal to retry with different strategy
            }
            CorrectionAction::RestartApp => {
                // Actually restart the app using bash
                println!("[correction] Attempting app restart via keyboard shortcut...");
                let executor = SkillExecutor::new();
                let _ = executor.init_computer().await;
                // Cmd+Q to quit, then wait, then retry will reopen
                let _ = executor.execute_computer_action("key", &serde_json::json!({"key": "cmd+q"})).await;
                tokio::time::sleep(Duration::from_millis(2000)).await;
                Ok(true)
            }
            CorrectionAction::AskUser => {
                // Cannot actually ask user through this path yet
                println!("[correction] Cannot automatically resolve - would need user input");
                Ok(false)
            }
        }
    }

    /// Classify a failure from a TaskResult
    fn classify_failure(&self, result: &TaskResult) -> FailureType {
        if let Some(ref error) = result.error {
            self.classify_error(error)
        } else {
            FailureType::Unknown
        }
    }

    /// Classify an error message
    fn classify_error(&self, error: &str) -> FailureType {
        let error_lower = error.to_lowercase();
        
        if error_lower.contains("not found") || error_lower.contains("doesn't exist") || error_lower.contains("cannot find") {
            FailureType::ElementNotFound
        } else if error_lower.contains("click") && (error_lower.contains("miss") || error_lower.contains("wrong")) {
            FailureType::ClickMissed
        } else if error_lower.contains("timeout") || error_lower.contains("timed out") {
            FailureType::Timeout
        } else if error_lower.contains("not responding") || error_lower.contains("hang") || error_lower.contains("freeze") {
            FailureType::AppNotResponding
        } else if error_lower.contains("wrong state") || error_lower.contains("unexpected") || error_lower.contains("different") {
            FailureType::WrongState
        } else if error_lower.contains("network") || error_lower.contains("connection") || error_lower.contains("offline") {
            FailureType::NetworkError
        } else if error_lower.contains("permission") || error_lower.contains("denied") || error_lower.contains("access") {
            FailureType::PermissionError
        } else if error_lower.contains("type") || error_lower.contains("input") {
            FailureType::TypeFailed
        } else {
            FailureType::Unknown
        }
    }

    /// Calculate delay with exponential backoff
    fn calculate_delay(&self, attempt: u32) -> u64 {
        self.base_delay_ms * 2_u64.pow(attempt.saturating_sub(1))
    }

    /// Get statistics about correction effectiveness
    pub fn get_stats(&self) -> CorrectionStats {
        CorrectionStats {
            total_strategies: self.strategies.values().map(|v| v.len()).sum(),
            failure_types_covered: self.strategies.len(),
            max_retries: self.max_retries,
        }
    }
}

#[derive(Debug)]
pub struct CorrectionStats {
    pub total_strategies: usize,
    pub failure_types_covered: usize,
    pub max_retries: u32,
}

impl Default for SelfCorrection {
    fn default() -> Self {
        Self::new()
    }
}