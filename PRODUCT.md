# Hey work - Complete Product Documentation

## 🎯 Overview

**Hey work** is an AI-powered computer automation agent that controls your computer through natural language instructions. It combines a Tauri-based desktop application with Claude AI to provide intelligent computer control, web automation, and document generation.

**Key Features:**
- 🤖 **Agent Swarm** - Multiple specialized AI agents working together
- 🧠 **Cognitive Engine** - Memory, skills, and learning capabilities  
- 🎤 **Voice Mode** - Push-to-talk voice commands
- 🌐 **Web Automation** - Browser control via Chrome DevTools Protocol
- 📄 **Document Generation** - Create Word, Excel, PDF, PowerPoint files
- ⚡ **Rate Limiting** - Intelligent API management with auto-retry

---

## 🏗️ System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         USER INTERFACE LAYER                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐  │
│  │ Main Bar │ │  Voice   │ │  Swarm   │ │  Skills  │ │  Onboarding  │  │
│  │ (React)  │ │  Mode    │ │  Panel   │ │  Panel   │ │   (Wizard)   │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐                               │
│  │  Border  │ │ Spotlight│ │ Settings │                               │
│  │ (Overlay)│ │ (Search) │ │ (Config) │                               │
│  └──────────┘ └──────────┘ └──────────┘                               │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      TAURI BRIDGE (Rust Backend)                         │
│                         Command Handlers                                 │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       CORE AGENT SYSTEM                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    AGENT SWARM (Multi-Agent)                     │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐          │   │
│  │  │ Planner  │ │ Executor │ │ Verifier │ │  Critic  │          │   │
│  │  │  Agent   │ │  Agent   │ │  Agent   │ │  Agent   │          │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘          │   │
│  │  ┌──────────┐ ┌──────────┐                                     │   │
│  │  │ Recovery │ │Coordinator                                    │   │
│  │  │  Agent   │ │  Agent   │                                     │   │
│  │  └──────────┘ └──────────┘                                     │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    COGNITIVE ENGINE                              │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐          │   │
│  │  │ Planner  │ │ Memory   │ │ Skills   │ │ Reasoner │          │   │
│  │  │ (DAG)    │ │ (SQLite) │ │ (Learn)  │ │ (Analyze)│          │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘          │   │
│  │  ┌──────────┐ ┌──────────┐                                     │   │
│  │  │ Context  │ │ Self-Cor │                                     │   │
│  │  │ Manager  │ │ rection  │                                     │   │
│  │  └──────────┘ └──────────┘                                     │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                   EXECUTION LAYER                                │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐          │   │
│  │  │ Computer │ │  Bash    │ │ Browser  │ │ Python   │          │   │
│  │  │ Control  │ │Executor  │ │   CDP    │ │   Tool   │          │   │
│  │  │(Screens) │ │(Terminal)│ │(Chrome)  │ │(Doc Gen) │          │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    ANTHROPIC API (Claude AI)                             │
│  • Streaming responses with tool use                                    │
│  • Computer Use + Bash + Web Search + Web Fetch tools                   │
│  • Extended thinking for complex tasks                                  │
│  • Rate limiting with exponential backoff                               │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 🆕 First-Time Setup & Onboarding

### Onboarding Flow (New Users)

When a user launches Hey work for the first time, they are guided through onboarding:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  1. Welcome     │ →  │  2. API Key     │ →  │  3. Permissions │ →  │  4. Complete    │
│                 │    │                 │    │                 │    │                 │
│ • Intro to Hey work │ │ • Anthropic     │    │ • Accessibility │    │ • Quick tips    │
│ • What it does  │    │   API key       │    │ • Screen Record │    │ • Shortcuts     │
│ • Get started   │    │ • Secure storage│    │ • Microphone    │    │ • Start using   │
└─────────────────┘    └─────────────────┘    └─────────────────┘    └─────────────────┘
```

### API Key Setup
- Secure input with show/hide toggle
- Validation (must start with `sk-`)
- Direct link to Anthropic Console
- Skip option (can be set later in Settings)

### Permission Management
Hey work requires these permissions:

| Permission | Required For | Status |
|------------|--------------|--------|
| **Accessibility** | Mouse/keyboard control | ✅ Required |
| **Screen Recording** | Taking screenshots | ✅ Required |
| **Microphone** | Voice input | ⚪ Optional |

**Permission Gate:** During task execution, if permissions are revoked, a warning modal appears with one-click fix buttons.

---

## 🤖 Agent Swarm (Multi-Agent System)

For complex multi-step tasks, Hey work activates the **Agent Swarm** - a coordinated team of specialized AI agents:

### Specialized Agents

| Agent | Role | Responsibilities |
|-------|------|------------------|
| **Planner** | Task Analyst | Breaks down complex requests into subtasks, manages dependencies |
| **Executor** | Action Performer | Executes computer/browser/bash actions |
| **Verifier** | Quality Checker | Validates results, checks for errors |
| **Critic** | Reviewer | Reviews overall task completion, suggests improvements |
| **Recovery** | Error Handler | Handles failures, retries with alternative approaches |
| **Coordinator** | Orchestrator | Manages agent communication and task flow |
| **Specialist** | Domain Expert | Handles specific domains (e.g., data analysis) |

### Swarm Task Flow

```
User Request: "Create a monthly report from my sales data"
              │
              ▼
┌──────────────────────────────────────────────────────────────┐
│ PLANNER AGENT                                                │
│ • Analyzes request complexity                                │
│ • Creates execution plan:                                    │
│   1. Find sales data files                                   │
│   2. Analyze data with Python                                │
│   3. Generate charts                                         │
│   4. Create Word document                                    │
│   5. Verify output                                           │
└──────────────────────────────────────────────────────────────┘
              │
              ▼
┌──────────────────────────────────────────────────────────────┐
│ PARALLEL EXECUTION                                           │
│                                                              │
│  Subtask 1: Find Files ──────► EXECUTOR AGENT               │
│  Subtask 2: Analyze Data ────► SPECIALIST AGENT             │
│  Subtask 3: Create Charts ───► EXECUTOR AGENT               │
│                                                              │
│  (Dependencies managed automatically)                        │
└──────────────────────────────────────────────────────────────┘
              │
              ▼
┌──────────────────────────────────────────────────────────────┐
│ VERIFIER & CRITIC AGENTS                                     │
│ • Check document completeness                                │
│ • Verify data accuracy                                       │
│ • Score: 0.95/1.0 ✅                                         │
└──────────────────────────────────────────────────────────────┘
              │
              ▼
         Task Complete!
```

### Swarm Monitoring UI

The **Swarm Panel** (accessible via 🤖 button) provides real-time visualization:

- **Active Tasks** - See running complex tasks
- **Progress Bars** - Track completion percentage
- **Subtask List** - View individual steps with status
- **Agent Badges** - Color-coded by agent type
- **Event Stream** - Live updates (started, completed, failed, recovery)

---

## 🧠 Cognitive Architecture

### Memory System (`cognitive/memory.rs`)

```rust
pub struct MemorySystem {
    memories: Vec<Memory>,           // Learned task patterns
    user_preferences: HashMap,        // User habits
    embedding_cache: HashMap,         // For semantic search
}

pub struct Memory {
    task_pattern: String,            // What was requested
    actions: Vec<String>,            // Steps taken
    success_rate: f32,               // How often it worked
    usage_count: u32,                // Times used
}
```

**How it works:**
1. After each successful task, stores the pattern
2. Before new task, searches for similar past tasks
3. Retrieves successful approaches
4. Learns user preferences over time
5. Persisted to SQLite (`~/Library/Application Support/hey-work/memory.db`)

### Skill System (`cognitive/skills.rs`)

```rust
pub struct Skill {
    name: String,
    pattern: TaskPattern,            // When to apply
    actions: Vec<ActionTemplate>,    // What to do
    success_rate: f32,
}
```

**Built-in skills:**
- Open Chrome
- Take Screenshot
- Spotlight Search
- Copy & Paste
- New Browser Tab
- Type URL

**Auto-learning:**
- Detects repeated successful patterns
- Creates new skills automatically
- Updates success rates based on outcomes

**Skills Management UI:**
- Export skills to JSON (backup/sharing)
- Import skills from JSON
- View all learned skills
- Delete individual skills
- See success rates and usage counts

### Rate Limiter (`rate_limiter.rs`)

```rust
pub struct RateLimiter {
    token_history: VecDeque<TokenBucketEntry>,  // 60-second sliding window
    tier: RateLimitTier,                        // Build (30k) or Scale (60k)
}
```

**Features:**
- Tracks token usage per minute
- Automatic throttling at 80% of limit
- Exponential backoff on rate limit errors (2s → 4s → 8s → 16s)
- Auto-retry with context preservation

**Limits (Build Tier):**
- 30,000 input tokens/minute
- 6,000 output tokens/minute

---

## 🔄 How It Works - Complete Flow

### 1. First Launch Flow

```
User installs Hey work
        │
        ▼
┌───────────────┐
│ Check localStorage  │──→ Has "heywork_onboarding_complete"?
│ for onboarding flag │
└───────────────┘
        │
   No ──┴── Yes
   │         │
   ▼         ▼
┌────────┐  ┌────────┐
│Show    │  │Skip    │
│Onboard-│  │Onboard-│
│ing     │  │ing     │
│Wizard  │  │        │
└────────┘  └────────┘
```

### 2. User Input Flow

```
User presses ⌃⇧C (Ctrl+Shift+C)
        │
        ▼
┌───────────────┐
│  Main Window  │ ──→ Shows input field
│   (Mini UI)   │
└───────────────┘
        │
        ▼
User types: "Open Chrome and search for AI news"
        │
        ▼
┌───────────────┐
│ Tauri Command │ ──→ run_agent(instructions, model, mode, ...)
│   (Rust IPC)  │
└───────────────┘
        │
        ▼
┌─────────────────────────────────────┐
│ Permission Check                    │
│ • Accessibility granted?            │
│ • Screen Recording granted?         │
│ If missing → Show warning modal     │
└─────────────────────────────────────┘
```

### 3. Agent Processing Loop

```rust
// Agent Loop (simplified)
while running && iteration < MAX_ITERATIONS {
    
    // 1. Check if complex task → Use Agent Swarm
    if is_complex_task(&instructions) {
        swarm.submit_task(instructions).await;
    }
    
    // 2. Send conversation to Claude API
    let response = anthropic_client.send_message(messages).await;
    
    // 3. Claude decides what to do (reasoning + tool calls)
    match response.content {
        Text { text } => {
            // Show thinking/response to user
            emit("agent-stream", text);
        }
        ToolUse { name, input } => {
            // Execute the requested tool
            match name {
                "computer" => execute_computer_action(input),
                "bash" => execute_bash_command(input),
                "browser" => execute_browser_tool(input),
                "python" => execute_python_code(input),
                "speak" => synthesize_speech(input),
            }
        }
    }
    
    // 4. Send tool results back to Claude
    messages.push(tool_results);
    
    // 5. Repeat until task complete
}
```

### 4. Tool Execution Details

#### Computer Tool (Screen Control)
```rust
ComputerAction {
    action: "screenshot" | "click" | "type" | "scroll" | "key",
    coordinate: [x, y],      // Normalized 0-1000
    text: "string to type",
}
```

**How it works:**
1. Takes screenshot using `xcap` library
2. Sends to Claude for analysis
3. Claude returns action (click at [x,y], type text, etc.)
4. Uses `enigo` library to control mouse/keyboard
5. Repeats until task complete

#### Bash Tool (Terminal)
```rust
BashCommand {
    command: "open -a 'Google Chrome'",
    restart: false,
}
```

**Features:**
- Persistent bash session (state maintained across calls)
- Works with any shell command
- Fast execution for file operations, app launching

#### Browser Tool (CDP - Chrome DevTools Protocol)
```rust
BrowserAction {
    see_page: { screenshot: true },     // Get accessibility tree
    page_action: { click: "3_42" },     // Click element by UID
    browser_navigate: { go_to_url: "..." },
}
```

**How it works:**
1. Connects to Chrome via WebSocket on port 9222
2. Uses `chromiumoxide` crate for CDP communication
3. Gets accessibility tree (UIDs for each element)
4. Executes clicks, typing, navigation via CDP commands
5. Works in background without controlling mouse

#### Python Tool (Document Generation) ⭐
```rust
PythonCode {
    code: "create_document('Hello', '/path/to/file.docx')",
    save_to: "/optional/path.txt",
}
```

**Built-in helper functions:**
```python
# Document creation
create_document(content, filepath, doc_type="auto")
# Auto-detects: .docx, .xlsx, .pdf, .pptx, .txt

# Data visualization  
create_chart(data, chart_type='bar', title='Chart', save_path=None)
# Supports: bar, line, pie charts
```

**Python libraries available:**
- `python-docx` - Word documents
- `pandas` + `openpyxl` - Excel spreadsheets
- `reportlab` - PDF generation
- `python-pptx` - PowerPoint
- `matplotlib` - Charts and graphs

---

## 🎮 User Interface

### Main Bar Window
- **Trigger:** ⌃⇧C (Computer Mode)
- **Size:** 280x40px floating bar
- **Features:**
  - Text input field
  - Model selector dropdown
  - History button
  - Voice mode toggle
  - 🤖 Swarm Panel button
  - 🧠 Skills Panel button

### Swarm Panel
- **Trigger:** 🤖 button in main bar
- **Features:**
  - Active task monitoring
  - Progress visualization
  - Subtask status tracking
  - Real-time event stream
  - Agent type badges

### Skills Panel
- **Trigger:** 🧠 button in main bar
- **Features:**
  - View learned skills
  - Export skills to JSON
  - Import skills from file
  - Delete skills
  - Success rate statistics

### Voice Mode Window
- **Trigger:** Push-to-talk button
- **Features:**
  - Deepgram STT integration
  - Audio visualization
  - Hands-free operation

### Border Window
- **Purpose:** Visual indicator when agent is active
- **Appearance:** Colored border around screen
- **Colors:**
  - Blue = Processing
  - Green = Success
  - Red = Error

### Spotlight Window
- **Trigger:** ⌃⇧B (Background Mode)
- **Features:**
  - Full chat interface
  - Conversation history
  - Settings panel

### Onboarding Wizard
- **Trigger:** First launch or Settings → Reset Onboarding
- **Steps:**
  1. Welcome introduction
  2. API key setup
  3. Permission granting
  4. Quick tips & completion

---

## 🔌 API Integration

### Anthropic API

**Model:** Claude with Computer Use (2025-01-24)

**Configuration:**
```rust
const MAX_TOKENS: u32 = 8000;        // Reduced for rate limits
const THINKING_BUDGET: u32 = 2000;    // Extended thinking

// Context management
context_management: {
    clear_thinking_20251015: {
        keep: 1 thinking turn
    },
    clear_tool_uses_20250919: {
        trigger: 20000 tokens,
        keep: 3 tool uses
    }
}
```

**Tools sent to API:**
1. `computer_20250124` - Screen control
2. `bash_20250124` - Terminal
3. `web_search_20250305` - Web search
4. `web_fetch_20250910` - URL fetching
5. `speak` - TTS (custom)
6. `python` - Document generation (custom)

### TTS Integration (ElevenLabs)

```rust
pub struct TtsClient {
    api_key: String,
    voice_id: String,  // Configurable in Settings
}

// Streams audio to frontend for playback
```

---

## 📁 Data Storage

### SQLite Databases

```
~/Library/Application Support/hey-work/
├── conversations.db          # Chat history
│   └── conversations table
│       ├── messages (JSON)
│       ├── usage stats
│       └── timestamps
│
├── memory.db                 # AI memory
│   ├── memories              # Learned patterns
│   ├── user_preferences      # User habits
│   ├── task_patterns         # Keyword indexing
│   └── memory_context        # Session contexts
│
└── skills.json               # Exported skills (optional)
```

### Chrome Profile

```
~/.heywork-chrome/               # Chrome user data
    ├── Cookies
    ├── Preferences
    └── ...
```

### Local Storage (Frontend)

```javascript
localStorage.setItem("heywork_onboarding_complete", "true");
```

---

## 🚀 Performance Optimizations

### 1. Streaming Responses
- Real-time text streaming via Server-Sent Events
- Users see AI thinking immediately
- No waiting for full response

### 2. Prompt Caching
- System prompts cached via `cache_control: ephemeral`
- Reduces token usage by ~70%
- Faster subsequent requests

### 3. Context Summarization
- Old browser snapshots summarized automatically
- Keeps only interactive elements (buttons, links)
- Prevents context window overflow

### 4. Rate Limit Management
```
Status: Safe       → No delay
Status: Throttle   → Wait for token window
Status: Limited    → Exponential backoff (2s, 4s, 8s...)
```

### 5. Parallel Subtask Execution
- Agent Swarm executes independent subtasks in parallel
- Reduces total task completion time
- Dependency management ensures correct order

---

## 🛠️ Development Stack

### Frontend
- **Framework:** React 19 + TypeScript
- **Build Tool:** Vite 6
- **Styling:** Tailwind CSS 3 + Framer Motion
- **State:** Zustand
- **Icons:** Lucide React

### Backend
- **Framework:** Tauri 2 (Rust)
- **HTTP Client:** reqwest
- **Async Runtime:** Tokio
- **Database:** SQLite (rusqlite)
- **Browser:** chromiumoxide (CDP)

### AI/ML
- **LLM:** Anthropic Claude
- **STT:** Deepgram API
- **TTS:** ElevenLabs API

---

## 📋 Example Usage Scenarios

### Scenario 1: Simple Task
```
User: "Open Chrome"
Agent:
  1. bash("open -a 'Google Chrome'")
  2. computer(screenshot) - verify
  3. Done
```

### Scenario 2: Web Automation
```
User: "Search for AI news on Google"
Agent:
  1. browser_navigate("https://google.com")
  2. see_page() - get snapshot
  3. page_action(type: "AI news", into: search_box_uid)
  4. page_action(press: "Enter")
  5. screenshot() - show results
```

### Scenario 3: Document Generation ⭐
```
User: "Create a project report"
Agent:
  1. python({
       code: """
       content = '''PROJECT REPORT
       
       Executive Summary
       - Project Status: On Track
       - Timeline: 3 months
       - Budget: $50,000
       '''
       create_document(content, '/Users/aktheboss/Desktop/project_report.docx')
       """
     })
  2. Document created successfully
```

### Scenario 4: Multi-Step Complex Task (Agent Swarm)
```
User: "Find the best Italian restaurants near me and create a spreadsheet"

Agent Swarm Activation:
┌─────────────────────────────────────────┐
│ PLANNER AGENT                           │
│ Analyzes: Multi-step, multi-domain task │
└─────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────┐
│ PARALLEL EXECUTION                      │
│                                         │
│  Executor Agent  → web_search()         │
│  Specialist Agent → analyze_results()   │
│  Executor Agent  → create_excel()       │
│                                         │
│  Verifier Agent  → check_completeness() │
└─────────────────────────────────────────┘
           │
           ▼
    Spreadsheet created!
```

---

## 🔒 Security & Privacy

### Data Protection
- API keys stored in Keychain
- Conversations stored locally (SQLite)
- No cloud storage of personal data
- Chrome profile isolated (`~/.heywork-chrome`)

### Permissions
```json
{
  "permissions": [
    "core:default",
    "shell:allow-open",
    "positioner:default"
  ]
}
```

### Sandboxed Execution
- Python code runs in isolated temp file
- No network access from Python (unless explicitly allowed)
- Resource limits enforced

---

## 🐛 Debugging & Logging

### Log Locations
```
# macOS Console
log stream --predicate 'process == "hey-work"'

# Terminal (dev mode)
npm run tauri dev
# Shows [heywork], [agent], [api], [memory], [swarm] logs
```

### Key Log Prefixes
- `[heywork]` - Main app events
- `[agent]` - Agent loop actions
- `[api]` - API calls and rate limits
- `[memory]` - Memory system operations
- `[cognitive]` - Cognitive engine events
- `[swarm]` - Agent Swarm coordination

---

## 🚧 Troubleshooting

### Common Issues

**Issue:** "Rate limit hit"  
**Solution:** Wait 1 minute or upgrade to Scale tier

**Issue:** Chrome not connecting  
**Solution:** Run: `open -a "Google Chrome" --args --remote-debugging-port=9222`

**Issue:** Python tool not working  
**Solution:** Install Python libraries: `pip3 install python-docx pandas openpyxl reportlab`

**Issue:** Slow responses  
**Solution:** Check rate limit status in logs, reduce context size

**Issue:** "Missing permissions"  
**Solution:** Grant Accessibility and Screen Recording in System Settings

**Issue:** Onboarding keeps showing  
**Solution:** Complete all steps or click "Skip for now"

---

## 📝 Configuration

### Environment Variables
```bash
ANTHROPIC_API_KEY=sk-ant-...
ELEVENLABS_API_KEY=sk_...
DEEPGRAM_API_KEY=...
```

### Settings File
```json
{
  "default_model": "claude-opus-4-6",
  "default_mode": "computer",
  "voice_mode": false,
  "rate_limit_tier": "build"
}
```

### Reset Onboarding
```
Settings → Setup → Reset Onboarding
# Or delete from browser console:
localStorage.removeItem("heywork_onboarding_complete")
```

---

## 🎓 Architecture Decisions

### Why Tauri?
- Smaller bundle size vs Electron
- Native performance (Rust backend)
- Better OS integration
- Secure by default

### Why Rust?
- Memory safety
- Zero-cost abstractions
- Excellent async/await support
- Native macOS APIs access

### Why Claude?
- Best-in-class reasoning
- Native computer use capability
- Extended thinking mode
- Excellent tool use

### Why Agent Swarm?
- Better handling of complex tasks
- Parallel execution for efficiency
- Specialized agents for different domains
- Self-correction and verification

---

## 🔮 Future Roadmap

### Phase 2: Visual Understanding
- OCR for text extraction
- UI element detection with ML
- Visual state change detection

### Phase 3: Advanced Learning
- Deep skill hierarchies
- Cross-task learning
- Predictive action suggestions
- Community skill marketplace

### Phase 4: Collaboration
- Multi-agent coordination improvements
- User feedback integration
- Team skill sharing
- Distributed task execution

---

## 📞 Support

**Shortcuts:**
- `⌃⇧C` - Computer Mode
- `⌃⇧B` - Browser Mode
- `⌘⇧S` - Stop Agent
- `⌘⇧H` - Help
- `⌃⇧C` (hold) - Push-to-Talk

**Logs:** Check Console.app or terminal output

**Reset:** Delete `~/Library/Application Support/hey-work/`

**Settings:** Click gear icon or use `⌘,`

---

*Built with ❤️ using Rust, React, and Claude*  
*Version: 0.1.0*  
*Bundle ID: com.heywork.app*
