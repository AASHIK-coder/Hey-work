# Hey work Architecture

## Overview

This document describes the advanced cognitive architecture for Hey work, designed to make the AI agent more accurate, learning-capable, and human-like in its job execution.

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                    USER INTERFACE LAYER                          │
│         (React Frontend - Chat, Settings, Visualizations)        │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                  COGNITIVE ENGINE (New)                          │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Planner   │  │   Memory    │  │    Skill Engine         │  │
│  │  (Task DAG) │  │  (Vector DB)│  │  (Learned Patterns)     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │  Reasoner   │  │  Context    │  │   Self-Correction       │  │
│  │(Chain-of-   │  │  Manager    │  │   (Retry Logic)         │  │
│  │  Thought)   │  │             │  │                         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                  EXECUTION LAYER (Enhanced)                      │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │  Computer   │  │   Browser   │  │    Bash/Terminal        │  │
│  │  (Vision)   │  │  (CDP)      │  │    (Enhanced)           │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Core Modules

### 1. Planner (`cognitive/planner.rs`)

**Purpose**: Break down high-level user requests into executable subtasks.

**Features**:
- **Template-based planning**: Predefined patterns for common tasks
- **AI-powered planning**: Dynamic plan generation for novel tasks
- **Dependency management**: DAG-based subtask execution order
- **Replanning**: Automatic adjustment when subtasks fail

**Example**:
```rust
// User: "Open Chrome and search for AI news"
// Plan:
// 1. Open Chrome (no dependencies)
// 2. Wait for Chrome to load (depends on 1)
// 3. Click address bar (depends on 2)
// 4. Type search query (depends on 3)
// 5. Press Enter (depends on 4)
```

### 2. Memory System (`cognitive/memory.rs`)

**Purpose**: Learn from past experiences and retrieve relevant context.

**Features**:
- **Semantic search**: Vector embeddings for task similarity
- **Success tracking**: Success rates for different approaches
- **User preferences**: Learn and remember user habits
- **Experience replay**: Use past successes to guide future actions

**Key Concepts**:
- `Memory`: Stores task patterns, actions, and outcomes
- `ExecutionRecord`: Captures execution details for learning
- Keyword-based indexing for fast retrieval

### 3. Skill Engine (`cognitive/skills.rs`)

**Purpose**: Build reusable skills from successful executions.

**Features**:
- **Predefined skills**: Common patterns loaded at startup
- **Skill learning**: Automatic skill extraction from successes
- **Pattern matching**: Match user requests to learned skills
- **Success tracking**: Update skill effectiveness over time

**Predefined Skills**:
- Open Chrome Browser
- Take Screenshot
- Spotlight Search
- Copy & Paste
- New Tab in Chrome
- Type URL and Navigate

**Skill Structure**:
```rust
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pattern: TaskPattern,      // When to use this skill
    pub actions: Vec<ActionTemplate>, // What actions to take
    pub success_rate: f32,         // How often it works
    pub total_uses: u32,
}
```

### 4. Reasoner (`cognitive/reasoner.rs`)

**Purpose**: Deep analysis and systematic problem solving.

**Features**:
- **Entity extraction**: Identify apps, files, URLs, people
- **Complexity assessment**: Simple, Moderate, Complex, VeryComplex
- **Approach selection**: Direct, Chain-of-Thought, Debug, Parallel
- **Hypothesis generation**: Try multiple approaches
- **Failure analysis**: Root cause analysis with suggested fixes

**Reasoning Approaches**:
- `Direct`: Simple tasks, immediate execution
- `ChainOfThought`: Complex tasks, step-by-step reasoning
- `ParallelHypotheses`: Try multiple approaches simultaneously
- `DebugAndRecover`: Systematic error recovery
- `ExploreExploit`: Learn then apply

### 5. Context Manager (`cognitive/context.rs`)

**Purpose**: Track application state and user preferences.

**Features**:
- **App state tracking**: Current app, open apps, previous app
- **System state**: Clipboard, active window, screen resolution
- **User preferences**: Learned settings with confidence scores
- **Session tracking**: Task completion stats, success rates
- **Screen analysis**: UI element detection and tracking

### 6. Self-Correction (`cognitive/correction.rs`)

**Purpose**: Automatic error detection and recovery.

**Features**:
- **Failure classification**: ElementNotFound, Timeout, WrongState, etc.
- **Retry strategies**: Different approaches for different failures
- **Exponential backoff**: Increasing delays between retries
- **Strategy exhaustion**: Fallback to alternative approaches

**Failure Types & Strategies**:
- **ElementNotFound** → Wait → Screenshot → Scroll → Alternative
- **Timeout** → Wait longer → Check state → Restart app
- **ClickMissed** → Screenshot → Retarget → Alternative selector
- **AppNotResponding** → Wait → Restart app

## How It Works

### Task Execution Flow

```
1. User Request
   ↓
2. Reasoner.analyze_request()
   - Extract intent, entities, complexity
   - Select reasoning approach
   ↓
3. Memory.search_relevant()
   - Find similar past tasks
   - Retrieve learned preferences
   ↓
4. SkillLibrary.find_matching_skills()
   - Check for applicable skills
   ↓
5. Planner.create_plan()
   - Generate subtask DAG
   - Assign dependencies
   ↓
6. For each subtask:
   a. SelfCorrection.execute_with_retry()
      - Try execution
      - If fails → Apply correction strategy
      - Retry with new approach
   b. SkillLibrary.learn_from_execution()
      - If successful → Update/create skill
   c. Memory.store_execution()
      - Record for future retrieval
   ↓
7. Task completion
   - Store in memory
   - Update statistics
   - Report to user
```

## Benefits

### 1. Accuracy Improvements

**Before**: Single-turn API calls, reactive
**After**: 
- Multi-step planning with verification
- Self-correction with multiple retry strategies
- Context-aware decision making
- Skill-based execution for known patterns

### 2. Learning Capability

**Before**: No memory between sessions
**After**:
- Remembers successful approaches
- Learns user preferences
- Builds skill library over time
- Semantic search for similar tasks

### 3. Human-like Execution

**Before**: Immediate action, no planning
**After**:
- Plans before acting
- Verifies each step
- Adapts to failures
- Uses context intelligently

## Integration with Existing Code

The cognitive engine is designed to integrate seamlessly:

```rust
// New cognitive agent
pub struct CognitiveAgent {
    cognitive: Arc<Mutex<CognitiveEngine>>,
    current_task: Arc<Mutex<Option<Task>>>,
    computer: Arc<Mutex<Option<ComputerControl>>>,
    bash: Arc<Mutex<BashExecutor>>,
}

// Can be used alongside or replace existing Agent
```

## Future Enhancements

### Phase 2: Visual Understanding
- OCR for text extraction
- UI element detection with ML
- Visual state change detection
- Icon and button recognition

### Phase 3: Advanced Learning
- Deep skill hierarchies
- Cross-task learning
- User behavior modeling
- Predictive action suggestions

### Phase 4: Collaboration
- Multi-agent coordination
- User feedback integration
- Continuous online learning
- Community skill sharing

## Usage Example

```rust
// Create cognitive agent
let agent = CognitiveAgent::new();
agent.initialize().await?;

// Process user request
let task = agent.process_request("Open Chrome and search for AI news", &app_handle).await?;

// Execute with automatic planning, memory, skills, and self-correction
while agent.get_task_status().await != Some(TaskStatus::Completed) {
    let result = agent.execute_next(&context).await?;
    
    if let Some(result) = result {
        println!("Step completed: {}", result.output);
        
        if !result.success {
            println!("Self-correcting...");
        }
    }
}
```

## Statistics & Monitoring

Each module provides statistics:

```rust
// Memory stats
MemoryStats {
    total_memories: 150,
    total_preferences: 23,
    avg_success_rate: 0.87,
}

// Skill stats
SkillStats {
    total_learned: 45,
    total_predefined: 6,
    avg_success_rate: 0.92,
}

// Correction stats
CorrectionStats {
    total_strategies: 25,
    failure_types_covered: 8,
    max_retries: 3,
}
```

## Conclusion

This architecture transforms Hey work from a simple command executor into an intelligent, learning agent that:

1. **Plans** complex tasks into manageable steps
2. **Remembers** what worked and what didn't
3. **Learns** reusable skills from experience
4. **Adapts** to failures with systematic recovery
5. **Understands** context and user preferences

The result is a more accurate, reliable, and human-like computer agent capable of handling complex, multi-step jobs autonomously.