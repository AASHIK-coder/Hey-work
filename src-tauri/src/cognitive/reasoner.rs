//! Reasoner Module - Chain-of-Thought and Systematic Problem Solving
//! 
//! Provides deep reasoning capabilities for complex tasks, including
//! chain-of-thought reasoning, hypothesis generation, and systematic
//! debugging of failures.

use super::planner::TaskComplexity;
use serde::{Deserialize, Serialize};

/// Reasoning engine for complex task analysis
pub struct Reasoner {
    /// Reasoning strategies available
    strategies: Vec<ReasoningStrategy>,
}

struct ReasoningStrategy {
    name: String,
    applicable_when: Box<dyn Fn(&str) -> bool + Send + Sync>,
}

// Manual Debug implementation
impl std::fmt::Debug for ReasoningStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReasoningStrategy")
            .field("name", &self.name)
            .field("applicable_when", &"<closure>")
            .finish()
    }
}

// Manual Clone implementation
impl Clone for ReasoningStrategy {
    fn clone(&self) -> Self {
        // Create new boxed function - simplified for now
        Self {
            name: self.name.clone(),
            applicable_when: Box::new(|_| true),
        }
    }
}

/// Types of reasoning approaches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningApproach {
    /// Direct execution for simple tasks
    Direct,
    /// Step-by-step thinking
    ChainOfThought,
    /// Try multiple approaches in parallel
    ParallelHypotheses,
    /// Debug and fix errors systematically
    DebugAndRecover,
    /// Explore then exploit (learn then act)
    ExploreExploit,
}

/// Analysis result from reasoning
#[derive(Debug, Clone)]
pub struct TaskAnalysis {
    pub intent: String,
    pub entities: Vec<Entity>,
    pub complexity: TaskComplexity,
    pub estimated_steps: u32,
    pub app_context: Option<String>,
    pub constraints: Vec<String>,
    pub approach: ReasoningApproach,
    pub potential_issues: Vec<String>,
    pub suggested_verifications: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub name: String,
    pub entity_type: EntityType,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    Application,
    File,
    URL,
    Person,
    Date,
    Time,
    Location,
    Command,
    Text,
}

/// Hypothesis for parallel exploration
#[derive(Debug, Clone)]
pub struct Hypothesis {
    pub id: String,
    pub description: String,
    pub confidence: f32,
    pub approach: String,
    pub expected_result: String,
}

/// Debug analysis for failed actions
#[derive(Debug, Clone)]
pub struct DebugAnalysis {
    pub failure_cause: FailureCause,
    pub suggested_fixes: Vec<SuggestedFix>,
    pub prevention_tips: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum FailureCause {
    ElementNotFound,
    WrongCoordinates,
    TimingIssue,
    ApplicationNotReady,
    UnexpectedState,
    PermissionDenied,
    NetworkError,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SuggestedFix {
    pub description: String,
    pub confidence: f32,
    pub action: String,
}

impl Reasoner {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                ReasoningStrategy {
                    name: "direct".to_string(),
                    applicable_when: Box::new(|req| {
                        req.split_whitespace().count() < 5 && 
                        !req.contains("and") && 
                        !req.contains("then")
                    }),
                },
                ReasoningStrategy {
                    name: "chain_of_thought".to_string(),
                    applicable_when: Box::new(|req| {
                        req.len() > 50 || 
                        req.contains("find") || 
                        req.contains("search") ||
                        req.contains("complex")
                    }),
                },
                ReasoningStrategy {
                    name: "debug".to_string(),
                    applicable_when: Box::new(|req| {
                        req.contains("fix") || 
                        req.contains("error") || 
                        req.contains("not working")
                    }),
                },
            ],
        }
    }

    /// Analyze a user request to understand intent and complexity
    pub async fn analyze_request(&self, request: &str) -> anyhow::Result<TaskAnalysis> {
        let _request_lower = request.to_lowercase();
        
        // Extract entities
        let entities = self.extract_entities(request);
        
        // Determine complexity
        let complexity = self.assess_complexity(request, &entities);
        
        // Estimate steps
        let estimated_steps = self.estimate_steps(&complexity, &entities);
        
        // Detect app context
        let app_context = self.detect_app_context(request);
        
        // Extract constraints
        let constraints = self.extract_constraints(request);
        
        // Choose reasoning approach
        let approach = self.select_approach(request, &complexity);
        
        // Predict potential issues
        let potential_issues = self.predict_issues(request, &app_context);
        
        // Suggest verification steps
        let suggested_verifications = self.suggest_verifications(&entities, &app_context);
        
        let analysis = TaskAnalysis {
            intent: self.extract_intent(request),
            entities,
            complexity,
            estimated_steps,
            app_context,
            constraints,
            approach,
            potential_issues,
            suggested_verifications,
        };
        
        println!("[reasoner] Analysis: intent='{}', complexity={:?}, approach={:?}", 
            analysis.intent, analysis.complexity, analysis.approach);
        
        Ok(analysis)
    }

    /// Extract key entities from the request
    fn extract_entities(&self, request: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        let request_lower = request.to_lowercase();
        
        // Application detection
        let apps = vec![
            ("chrome", "Google Chrome"),
            ("safari", "Safari"),
            ("firefox", "Firefox"),
            ("spotify", "Spotify"),
            ("slack", "Slack"),
            ("vscode", "Visual Studio Code"),
            ("code", "Visual Studio Code"),
            ("terminal", "Terminal"),
            ("finder", "Finder"),
            ("mail", "Mail"),
            ("outlook", "Microsoft Outlook"),
            ("word", "Microsoft Word"),
            ("excel", "Microsoft Excel"),
            ("powerpoint", "Microsoft PowerPoint"),
            ("zoom", "Zoom"),
        ];
        
        for (keyword, app_name) in &apps {
            if request_lower.contains(keyword) {
                entities.push(Entity {
                    name: app_name.to_string(),
                    entity_type: EntityType::Application,
                    value: None,
                });
            }
        }
        
        // URL detection
        if request_lower.contains("http") || request_lower.contains("www.") || request_lower.contains(".com") {
            // Extract URL pattern
            let words: Vec<&str> = request.split_whitespace().collect();
            for word in words {
                if word.contains(".") && (word.contains("http") || word.contains("www") || word.contains(".com") || word.contains(".org")) {
                    entities.push(Entity {
                        name: "URL".to_string(),
                        entity_type: EntityType::URL,
                        value: Some(word.to_string()),
                    });
                }
            }
        }
        
        // File detection
        if request_lower.contains("file") || request_lower.contains("document") || request_lower.contains("open") {
            // Try to extract filename
            let file_indicators = ["file", "document", "called", "named"];
            for indicator in &file_indicators {
                if let Some(pos) = request_lower.find(indicator) {
                    let after = &request[pos + indicator.len()..];
                    let words: Vec<&str> = after.split_whitespace().take(3).collect();
                    if !words.is_empty() {
                        entities.push(Entity {
                            name: "File".to_string(),
                            entity_type: EntityType::File,
                            value: Some(words.join(" ")),
                        });
                        break;
                    }
                }
            }
        }
        
        // Person detection (simple heuristic)
        if request_lower.contains("email") || request_lower.contains("contact") || request_lower.contains("message") {
            // Look for capitalized words that might be names
            let words: Vec<&str> = request.split_whitespace().collect();
            for word in words {
                if word.len() > 2 && word.chars().next().map_or(false, |c| c.is_uppercase()) {
                    if !apps.iter().any(|(k, _)| word.to_lowercase().contains(k)) {
                        entities.push(Entity {
                            name: "Person".to_string(),
                            entity_type: EntityType::Person,
                            value: Some(word.to_string()),
                        });
                        break;
                    }
                }
            }
        }
        
        entities
    }

    /// Assess task complexity
    fn assess_complexity(&self, request: &str, entities: &[Entity]) -> TaskComplexity {
        let word_count = request.split_whitespace().count();
        let entity_count = entities.len();
        
        // Count complexity indicators
        let complexity_markers = [
            "and then", "after that", "next", "finally",
            "search for", "find", "download", "upload",
            "create", "edit", "modify", "change",
            "copy", "paste", "move", "delete",
        ];
        
        let marker_count = complexity_markers
            .iter()
            .filter(|&&m| request.to_lowercase().contains(m))
            .count();
        
        match (word_count, entity_count, marker_count) {
            (w, _, _) if w <= 4 => TaskComplexity::Simple,
            (w, e, m) if w <= 10 && e <= 2 && m <= 1 => TaskComplexity::Moderate,
            (w, e, m) if w <= 20 && e <= 3 && m <= 3 => TaskComplexity::Complex,
            _ => TaskComplexity::VeryComplex,
        }
    }

    /// Estimate number of steps needed
    fn estimate_steps(&self, complexity: &TaskComplexity, entities: &[Entity]) -> u32 {
        let base_steps = match complexity {
            TaskComplexity::Simple => 1,
            TaskComplexity::Moderate => 3,
            TaskComplexity::Complex => 5,
            TaskComplexity::VeryComplex => 8,
        };
        
        let entity_bonus = entities.len() as u32;
        
        base_steps + entity_bonus
    }

    /// Detect which application context we're in
    fn detect_app_context(&self, request: &str) -> Option<String> {
        let request_lower = request.to_lowercase();
        
        if request_lower.contains("chrome") || request_lower.contains("browser") || request_lower.contains("web") {
            Some("chrome".to_string())
        } else if request_lower.contains("finder") || request_lower.contains("file") {
            Some("finder".to_string())
        } else if request_lower.contains("terminal") || request_lower.contains("command") {
            Some("terminal".to_string())
        } else if request_lower.contains("mail") || request_lower.contains("email") {
            Some("mail".to_string())
        } else {
            None
        }
    }

    /// Extract constraints from request
    fn extract_constraints(&self, request: &str) -> Vec<String> {
        let mut constraints = Vec::new();
        let request_lower = request.to_lowercase();
        
        if request_lower.contains("quickly") || request_lower.contains("fast") {
            constraints.push("speed".to_string());
        }
        if request_lower.contains("carefully") || request_lower.contains("accurate") {
            constraints.push("accuracy".to_string());
        }
        if request_lower.contains("without") || request_lower.contains("don't") {
            constraints.push("avoid_certain_actions".to_string());
        }
        
        constraints
    }

    /// Extract the core intent
    fn extract_intent(&self, request: &str) -> String {
        let request_lower = request.to_lowercase();
        
        // Remove common prefixes
        let cleaned = request_lower
            .trim_start_matches("please ")
            .trim_start_matches("can you ")
            .trim_start_matches("could you ")
            .trim_start_matches("i want to ")
            .trim_start_matches("i need to ")
            .trim_start_matches("help me ")
            .to_string();
        
        // Extract verb + object pattern
        let words: Vec<&str> = cleaned.split_whitespace().take(5).collect();
        words.join(" ")
    }

    /// Select the best reasoning approach
    fn select_approach(&self, request: &str, complexity: &TaskComplexity) -> ReasoningApproach {
        let request_lower = request.to_lowercase();
        
        // Check for specific indicators
        if request_lower.contains("fix") || request_lower.contains("error") || request_lower.contains("not working") {
            return ReasoningApproach::DebugAndRecover;
        }
        
        if request_lower.contains("try") || request_lower.contains("maybe") || request_lower.contains("or") {
            return ReasoningApproach::ParallelHypotheses;
        }
        
        match complexity {
            TaskComplexity::Simple => ReasoningApproach::Direct,
            TaskComplexity::Moderate => ReasoningApproach::ChainOfThought,
            TaskComplexity::Complex => ReasoningApproach::ChainOfThought,
            TaskComplexity::VeryComplex => ReasoningApproach::ExploreExploit,
        }
    }

    /// Predict potential issues before execution
    fn predict_issues(&self, request: &str, app_context: &Option<String>) -> Vec<String> {
        let mut issues = Vec::new();
        let request_lower = request.to_lowercase();
        
        // Common issue patterns
        if request_lower.contains("click") && !request_lower.contains("wait") {
            issues.push("Element might not be ready - consider adding wait".to_string());
        }
        
        if request_lower.contains("search") {
            issues.push("Search results might take time to load".to_string());
        }
        
        if request_lower.contains("download") {
            issues.push("Network speed might affect download time".to_string());
        }
        
        if app_context.is_none() && request_lower.contains("app") {
            issues.push("Application might not be running".to_string());
        }
        
        issues
    }

    /// Suggest verification steps
    fn suggest_verifications(&self, entities: &[Entity], _app_context: &Option<String>) -> Vec<String> {
        let mut verifications = Vec::new();
        
        if entities.iter().any(|e| matches!(e.entity_type, EntityType::Application)) {
            verifications.push("Verify application is open and focused".to_string());
        }
        
        if entities.iter().any(|e| matches!(e.entity_type, EntityType::URL)) {
            verifications.push("Verify page loaded correctly".to_string());
        }
        
        if entities.iter().any(|e| matches!(e.entity_type, EntityType::File)) {
            verifications.push("Verify file exists and is accessible".to_string());
        }
        
        verifications.push("Take screenshot to verify final state".to_string());
        
        verifications
    }

    /// Generate hypotheses for parallel exploration
    pub fn generate_hypotheses(&self, problem: &str, _context: &str) -> Vec<Hypothesis> {
        let mut hypotheses = Vec::new();
        
        // Hypothesis 1: Direct approach
        hypotheses.push(Hypothesis {
            id: "h1".to_string(),
            description: "Try the most direct approach".to_string(),
            confidence: 0.7,
            approach: "direct".to_string(),
            expected_result: "Success on first try".to_string(),
        });
        
        // Hypothesis 2: Alternative path
        hypotheses.push(Hypothesis {
            id: "h2".to_string(),
            description: "Try alternative method".to_string(),
            confidence: 0.5,
            approach: "alternative".to_string(),
            expected_result: "Success via different path".to_string(),
        });
        
        // Hypothesis 3: More careful approach
        if problem.len() > 50 {
            hypotheses.push(Hypothesis {
                id: "h3".to_string(),
                description: "Break into smaller steps with verification".to_string(),
                confidence: 0.6,
                approach: "step_by_step".to_string(),
                expected_result: "Success with more steps".to_string(),
            });
        }
        
        hypotheses
    }

    /// Analyze a failure for debugging
    pub fn analyze_failure(&self, action: &str, error: &str, _screenshot: Option<&str>) -> DebugAnalysis {
        let error_lower = error.to_lowercase();
        
        let failure_cause = if error_lower.contains("not found") || error_lower.contains("doesn't exist") {
            FailureCause::ElementNotFound
        } else if error_lower.contains("coordinate") || error_lower.contains("position") {
            FailureCause::WrongCoordinates
        } else if error_lower.contains("timeout") || error_lower.contains("wait") {
            FailureCause::TimingIssue
        } else if error_lower.contains("not ready") || error_lower.contains("loading") {
            FailureCause::ApplicationNotReady
        } else if error_lower.contains("unexpected") || error_lower.contains("different") {
            FailureCause::UnexpectedState
        } else if error_lower.contains("permission") || error_lower.contains("denied") {
            FailureCause::PermissionDenied
        } else if error_lower.contains("network") || error_lower.contains("connection") {
            FailureCause::NetworkError
        } else {
            FailureCause::Unknown
        };

        let suggested_fixes = self.generate_fixes(&failure_cause, action);
        
        let prevention_tips = match failure_cause {
            FailureCause::ElementNotFound => vec![
                "Always take screenshot before clicking".to_string(),
                "Use more reliable selectors".to_string(),
            ],
            FailureCause::TimingIssue => vec![
                "Add explicit waits after state changes".to_string(),
                "Wait for loading indicators to disappear".to_string(),
            ],
            FailureCause::ApplicationNotReady => vec![
                "Verify application is running first".to_string(),
                "Wait for app to fully initialize".to_string(),
            ],
            _ => vec!["Add more verification steps".to_string()],
        };

        DebugAnalysis {
            failure_cause,
            suggested_fixes,
            prevention_tips,
        }
    }

    fn generate_fixes(&self, cause: &FailureCause, _action: &str) -> Vec<SuggestedFix> {
        match cause {
            FailureCause::ElementNotFound => vec![
                SuggestedFix {
                    description: "Take fresh screenshot and look for element".to_string(),
                    confidence: 0.9,
                    action: "screenshot".to_string(),
                },
                SuggestedFix {
                    description: "Try scrolling to find the element".to_string(),
                    confidence: 0.6,
                    action: "scroll".to_string(),
                },
            ],
            FailureCause::TimingIssue => vec![
                SuggestedFix {
                    description: "Wait longer for element to appear".to_string(),
                    confidence: 0.8,
                    action: "wait".to_string(),
                },
                SuggestedFix {
                    description: "Retry the action".to_string(),
                    confidence: 0.7,
                    action: "retry".to_string(),
                },
            ],
            FailureCause::ApplicationNotReady => vec![
                SuggestedFix {
                    description: "Open or focus the application first".to_string(),
                    confidence: 0.9,
                    action: "open_app".to_string(),
                },
            ],
            FailureCause::WrongCoordinates => vec![
                SuggestedFix {
                    description: "Recalculate coordinates based on current screen".to_string(),
                    confidence: 0.7,
                    action: "recalculate".to_string(),
                },
            ],
            _ => vec![
                SuggestedFix {
                    description: "Try alternative approach".to_string(),
                    confidence: 0.5,
                    action: "alternative".to_string(),
                },
            ],
        }
    }

    /// Reason about the current state and next best action
    pub fn reason_next_action(&self, _goal: &str, current_state: &str, history: &[String]) -> String {
        // Simple rule-based reasoning
        if current_state.contains("error") || current_state.contains("failed") {
            return "analyze_error_and_retry".to_string();
        }
        
        if current_state.contains("loading") || current_state.contains("wait") {
            return "wait".to_string();
        }
        
        if history.len() > 10 {
            return "consider_alternative_approach".to_string();
        }
        
        "continue_with_plan".to_string()
    }
}

impl Default for Reasoner {
    fn default() -> Self {
        Self::new()
    }
}