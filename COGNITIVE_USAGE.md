# Using the New Cognitive Architecture

## Overview

The cognitive engine is now integrated into the Agent. It provides advanced capabilities like planning, memory, skills, and self-correction.

## Key Features

### 1. **Automatic Task Planning**
The agent now automatically breaks down complex requests into step-by-step plans:

```
User: "Open Chrome and search for AI news"

Plan Generated:
1. Open Chrome application
2. Wait for Chrome to load
3. Click on address bar
4. Type "AI news"
5. Press Enter
6. Verify results loaded
```

### 2. **Built-in Skills** (6 Predefined)
- ✅ Open Chrome Browser
- ✅ Take Screenshot  
- ✅ Spotlight Search (Cmd+Space)
- ✅ Copy & Paste
- ✅ New Tab in Chrome
- ✅ Type URL and Navigate

### 3. **Learning System**
The agent learns from successful executions:
- Stores successful task patterns
- Retrieves similar past experiences
- Learns user preferences
- Builds custom skills over time

### 4. **Self-Correction**
When an action fails, the agent automatically retries with different strategies:
- Element not found → Wait → Screenshot → Scroll → Alternative approach
- Timeout → Wait longer → Check state → Restart app
- Wrong coordinates → Retarget → Alternative selector

### 5. **Smart Reasoning**
- Entity extraction (apps, files, URLs, people)
- Complexity assessment (Simple → VeryComplex)
- Approach selection (Direct, Chain-of-Thought, Debug)
- Failure analysis with suggested fixes

## Usage

The cognitive features are automatically active. To use them in your code:

```rust
// Access cognitive engine from agent
let cognitive = agent.cognitive.lock().await;

// Process a request with full cognitive pipeline
let task = cognitive.process_request("Open Chrome and search for AI news").await?;

// Execute with automatic planning, memory, skills, and self-correction
while let Some(result) = cognitive.execute_next(&mut task).await? {
    println!("Step completed: {}", result.output);
    
    if !result.success {
        println!("Self-correcting...");
    }
}

// Get statistics
let memory_stats = cognitive.memory.get_stats();
let skill_stats = cognitive.skills.get_stats();
```

## Architecture

```
User Request
    ↓
┌─────────────────┐
│     Reasoner    │ → Extract intent, entities, complexity
└─────────────────┘
    ↓
┌─────────────────┐
│      Memory     │ → Find similar past tasks
└─────────────────┘
    ↓
┌─────────────────┐
│      Skills     │ → Check for applicable skills
└─────────────────┘
    ↓
┌─────────────────┐
│     Planner     │ → Create subtask DAG
└─────────────────┘
    ↓
┌─────────────────┐
│ Self-Correction │ → Execute with retry
└─────────────────┘
    ↓
┌─────────────────┐
│  Learn & Store  │ → Update skills & memory
└─────────────────┘
```

## Statistics Available

### Memory Stats
```rust
MemoryStats {
    total_memories: 150,      // Number of learned experiences
    total_preferences: 23,    // User preferences learned
    avg_success_rate: 0.87,   // Average success across memories
}
```

### Skill Stats
```rust
SkillStats {
    total_learned: 45,        // Skills learned from experience
    total_predefined: 6,      // Built-in skills
    total_skills: 51,         // Total available
    avg_success_rate: 0.92,   // Average skill success rate
}
```

### Correction Stats
```rust
CorrectionStats {
    total_strategies: 25,     // Available retry strategies
    failure_types_covered: 8, // Different failure types handled
    max_retries: 3,           // Maximum retry attempts
}
```

## Example: Complex Task Execution

```rust
// User: "Find John's email about the project and reply"

// Step 1: Reasoning extracts entities
// - Person: "John"
// - Action: "find email", "reply"
// - Topic: "project"
// Complexity: Complex

// Step 2: Memory retrieves similar past email tasks

// Step 3: Plan generated:
// 1. Open Mail app
// 2. Search for "John project"
// 3. Find recent email
// 4. Open email
// 5. Click reply
// 6. Compose response
// 7. Send email

// Step 4: Execute with self-correction
// If Mail app doesn't open → Retry with Spotlight search
// If email not found → Try searching in sent folder

// Step 5: Learn from success
// New skill created: "Reply to person's email about topic"
```

## Benefits

### Before (Original Agent)
- ❌ Single-turn API calls
- ❌ No memory between tasks
- ❌ No planning
- ❌ Simple retry only
- ❌ Reactive execution

### After (Cognitive Agent)
- ✅ Multi-step planning with verification
- ✅ Remembers successful approaches
- ✅ Builds reusable skills
- ✅ Smart self-correction
- ✅ Proactive error recovery
- ✅ Context-aware decisions

## Next Steps

To further enhance the cognitive capabilities:

1. **Visual Understanding**: Add OCR and UI element detection
2. **Deep Learning**: Use embeddings for better semantic search
3. **Multi-Agent**: Coordinate multiple agents for complex workflows
4. **User Feedback**: Learn from explicit user corrections
5. **Skill Sharing**: Share learned skills between users

## Files Added

```
src-tauri/src/cognitive/
├── mod.rs           # Core types & CognitiveEngine
├── planner.rs       # Task decomposition & DAG execution
├── memory.rs        # Learning & retrieval
├── skills.rs        # Skill library & learning
├── reasoner.rs      # Analysis & reasoning
├── context.rs       # State tracking
└── correction.rs    # Auto-retry logic
```

The cognitive engine is now fully integrated and ready to make your computer agent significantly more intelligent and capable!