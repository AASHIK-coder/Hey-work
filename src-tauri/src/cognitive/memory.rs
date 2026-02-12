//! Memory System - Long-term Learning and Context Retrieval
//! 
//! Stores successful task patterns, user preferences, and retrieved
//! relevant memories using vector embeddings for semantic search.
//! Persisted to SQLite for durability across sessions.

use super::Memory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::sync::Mutex;

/// Vector embedding for semantic search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub vector: Vec<f32>,
    pub model: String,
    pub dimensions: usize,
}

/// Memory storage with vector search capabilities
pub struct MemorySystem {
    /// In-memory storage
    memories: Vec<Memory>,
    /// User preferences learned over time
    user_preferences: HashMap<String, String>,
    /// Task patterns to memory mapping
    task_patterns: HashMap<String, Vec<String>>, // pattern -> memory_ids
    /// Simple embedding cache (in production, use a proper vector DB)
    embedding_cache: HashMap<String, Embedding>,
    /// Database connection
    db: Option<Mutex<Connection>>,
}

/// Task execution record for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub task_description: String,
    pub actions_taken: Vec<String>,
    pub success: bool,
    pub execution_time_ms: u64,
    pub context: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

impl MemorySystem {
    pub fn new() -> Self {
        Self {
            memories: Vec::new(),
            user_preferences: HashMap::new(),
            task_patterns: HashMap::new(),
            embedding_cache: HashMap::new(),
            db: None,
        }
    }

    /// Initialize database connection and load existing memories
    pub fn init(&mut self) -> anyhow::Result<()> {
        let db_path = Self::get_db_path();
        
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        
        // Create tables
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                task_pattern TEXT NOT NULL,
                actions_json TEXT NOT NULL,
                success_rate REAL NOT NULL DEFAULT 0.0,
                usage_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                embedding_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_memories_pattern ON memories(task_pattern);
            
            CREATE TABLE IF NOT EXISTS user_preferences (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS task_patterns (
                keyword TEXT NOT NULL,
                memory_id TEXT NOT NULL,
                PRIMARY KEY (keyword, memory_id),
                FOREIGN KEY (memory_id) REFERENCES memories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_patterns_keyword ON task_patterns(keyword);
            
            CREATE TABLE IF NOT EXISTS memory_context (
                session_id TEXT PRIMARY KEY,
                context_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "
        )?;

        self.db = Some(Mutex::new(conn));
        
        // Load existing memories
        self.load_memories()?;
        self.load_preferences()?;
        
        println!("[memory] Initialized with {} memories and {} preferences", 
            self.memories.len(), self.user_preferences.len());
        
        Ok(())
    }

    fn get_db_path() -> PathBuf {
        #[cfg(target_os = "macos")]
        let base = dirs::data_dir();
        #[cfg(not(target_os = "macos"))]
        let base = dirs::data_local_dir();

        base.unwrap_or_else(|| PathBuf::from("."))
            .join("hey-work")
            .join("memory.db")
    }

    fn with_db<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&Connection) -> anyhow::Result<T>,
    {
        match &self.db {
            Some(db) => {
                let guard = db.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
                f(&guard)
            }
            None => Err(anyhow::anyhow!("Database not initialized")),
        }
    }

    /// Load memories from database
    fn load_memories(&mut self) -> anyhow::Result<()> {
        // Collect memories first to avoid borrow issues
        let mut loaded_memories: Vec<Memory> = Vec::new();
        
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, task_pattern, actions_json, success_rate, usage_count, created_at, embedding_json FROM memories"
            )?;

            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let task_pattern: String = row.get(1)?;
                let actions_json: String = row.get(2)?;
                let success_rate: f64 = row.get(3)?;
                let usage_count: i64 = row.get(4)?;
                let created_at: String = row.get(5)?;
                let embedding_json: Option<String> = row.get(6)?;

                let actions: Vec<String> = serde_json::from_str(&actions_json).unwrap_or_default();
                let embedding: Option<Vec<f32>> = embedding_json
                    .and_then(|s| serde_json::from_str(&s).ok());

                Ok(Memory {
                    id,
                    task_pattern,
                    actions,
                    success_rate: success_rate as f32,
                    usage_count: usage_count as u32,
                    created_at: created_at.parse().unwrap_or_else(|_| Utc::now()),
                    embedding,
                })
            })?;

            for row in rows {
                if let Ok(memory) = row {
                    loaded_memories.push(memory);
                }
            }

            Ok(())
        })?;
        
        // Now rebuild indexes and embedding cache after with_db returns
        for memory in loaded_memories {
            let keywords = self.extract_keywords(&memory.task_pattern);
            for keyword in keywords {
                self.task_patterns
                    .entry(keyword)
                    .or_default()
                    .push(memory.id.clone());
            }
            // Rebuild embedding cache from stored embeddings or regenerate
            let embedding = if let Some(ref vec) = memory.embedding {
                Embedding {
                    vector: vec.clone(),
                    model: "trigram-hash-256".to_string(),
                    dimensions: vec.len(),
                }
            } else {
                self.generate_simple_embedding(&memory.task_pattern)
            };
            self.embedding_cache.insert(memory.id.clone(), embedding);
            self.memories.push(memory);
        }

        Ok(())
    }

    /// Load preferences from database
    fn load_preferences(&mut self) -> anyhow::Result<()> {
        let mut prefs: Vec<(String, String)> = Vec::new();
        
        self.with_db(|conn| {
            let mut stmt = conn.prepare("SELECT key, value FROM user_preferences")?;
            let rows = stmt.query_map([], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })?;

            for row in rows {
                if let Ok((k, v)) = row {
                    prefs.push((k, v));
                }
            }

            Ok(())
        })?;
        
        // Insert after with_db returns
        for (k, v) in prefs {
            self.user_preferences.insert(k, v);
        }
        
        Ok(())
    }

    /// Store a new memory from successful execution
    pub async fn store_execution(&mut self, record: ExecutionRecord) -> anyhow::Result<Memory> {
        let memory_id = Uuid::new_v4().to_string();
        
        // Generate simple keyword-based "embedding" (in production, use OpenAI/Claude embeddings)
        let embedding = self.generate_simple_embedding(&record.task_description);
        
        let memory = Memory {
            id: memory_id.clone(),
            task_pattern: record.task_description.clone(),
            actions: record.actions_taken.clone(),
            success_rate: if record.success { 1.0 } else { 0.0 },
            usage_count: 1,
            created_at: Utc::now(),
            embedding: Some(embedding.vector.clone()),
        };
        
        // Store embedding
        self.embedding_cache.insert(memory_id.clone(), embedding);
        
        // Index by keywords
        let keywords = self.extract_keywords(&record.task_description);
        for keyword in keywords {
            self.task_patterns
                .entry(keyword)
                .or_default()
                .push(memory_id.clone());
        }
        
        self.memories.push(memory.clone());
        
        // Persist to storage
        self.persist_memory(&memory).await?;
        
        Ok(memory)
    }

    /// Search for relevant memories using hybrid keyword + embedding similarity
    pub async fn search_relevant(&self, query: &str) -> anyhow::Result<Vec<Memory>> {
        let query_keywords = self.extract_keywords(query);
        let query_embedding = self.generate_simple_embedding(query);
        let query_lower = query.to_lowercase();
        let mut scored_memories: Vec<(Memory, f32)> = Vec::new();
        
        for memory in &self.memories {
            let mut score = 0.0;
            
            // 1. Keyword overlap (0-0.3)
            let memory_keywords = self.extract_keywords(&memory.task_pattern);
            let overlap: f32 = query_keywords
                .iter()
                .filter(|k| memory_keywords.contains(k))
                .count() as f32;
            
            if !query_keywords.is_empty() {
                score += (overlap / query_keywords.len() as f32) * 0.3;
            }
            
            // 2. Embedding cosine similarity (0-0.3)
            let memory_embedding = self.embedding_cache.get(&memory.id)
                .cloned()
                .unwrap_or_else(|| self.generate_simple_embedding(&memory.task_pattern));
            let cosine_sim = self.cosine_similarity(&query_embedding.vector, &memory_embedding.vector);
            score += cosine_sim * 0.3;
            
            // 3. Substring/fuzzy match (0-0.15) - catches things keyword matching misses
            let pattern_lower = memory.task_pattern.to_lowercase();
            if pattern_lower.contains(&query_lower) || query_lower.contains(&pattern_lower) {
                score += 0.15;
            } else {
                // Check if any significant words from query appear as substrings
                let word_matches = query_keywords.iter()
                    .filter(|k| pattern_lower.contains(k.as_str()))
                    .count() as f32;
                if !query_keywords.is_empty() {
                    score += (word_matches / query_keywords.len() as f32) * 0.1;
                }
            }
            
            // 4. Success rate weighting (0-0.15)
            score += memory.success_rate * 0.15;
            
            // 5. Recency bonus (0-0.05)
            let age_days = (Utc::now() - memory.created_at).num_days() as f32;
            let recency_score = (1.0 / (1.0 + age_days / 30.0)) * 0.05;
            score += recency_score;
            
            // 6. Usage frequency bonus (0-0.05)
            let usage_score = (memory.usage_count as f32 / 50.0).min(0.05);
            score += usage_score;
            
            if score > 0.15 { // Lower threshold to catch more potential matches
                scored_memories.push((memory.clone(), score));
            }
        }
        
        // Sort by score descending
        scored_memories.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Return top 5 memories
        let results: Vec<Memory> = scored_memories.into_iter().take(5).map(|(m, _)| m).collect();
        if !results.is_empty() {
            println!("[memory] Found {} relevant memories for: \"{}\"", results.len(), 
                if query.len() > 50 { &query[..50] } else { query });
        }
        Ok(results)
    }

    /// Store task intent for later retrieval
    pub async fn store_task_intent(&mut self, request: &str, task: &super::Task) -> anyhow::Result<()> {
        let record = ExecutionRecord {
            task_description: request.to_string(),
            actions_taken: task.subtasks.iter().map(|s| s.description.clone()).collect(),
            success: task.status == super::TaskStatus::Completed,
            execution_time_ms: 0, // Would track actual time
            context: HashMap::new(),
            timestamp: Utc::now(),
        };
        
        self.store_execution(record).await?;
        Ok(())
    }

    /// Learn user preference from interaction
    pub async fn learn_preference(&mut self, key: &str, value: &str) -> anyhow::Result<()> {
        self.user_preferences.insert(key.to_string(), value.to_string());
        
        // Persist preference
        self.persist_preference(key, value).await?;
        
        println!("[memory] Learned preference: {} = {}", key, value);
        Ok(())
    }

    /// Get a learned user preference
    pub fn get_preference(&self, key: &str) -> Option<&String> {
        self.user_preferences.get(key)
    }

    /// Get all preferences matching a prefix
    pub fn get_preferences_with_prefix(&self, prefix: &str) -> HashMap<String, String> {
        self.user_preferences
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Update memory success rate after reuse
    pub async fn update_memory_success(&mut self, memory_id: &str, success: bool) -> anyhow::Result<()> {
        if let Some(memory) = self.memories.iter_mut().find(|m| m.id == memory_id) {
            memory.usage_count += 1;
            
            // Update success rate with exponential moving average
            let alpha = 0.3; // Learning rate
            let new_success = if success { 1.0 } else { 0.0 };
            memory.success_rate = memory.success_rate * (1.0 - alpha) + new_success * alpha;
            
            println!(
                "[memory] Updated memory {}: success_rate={:.2}, uses={}",
                memory_id, memory.success_rate, memory.usage_count
            );
        }
        
        Ok(())
    }

    /// Extract keywords from text for indexing
    fn extract_keywords(&self, text: &str) -> Vec<String> {
        let text_lower = text.to_lowercase();
        let stop_words: std::collections::HashSet<&str> = [
            "the", "a", "an", "is", "are", "was", "were", "be", "been",
            "being", "have", "has", "had", "do", "does", "did", "will",
            "would", "could", "should", "may", "might", "must", "shall",
            "can", "need", "dare", "ought", "used", "to", "of", "in",
            "for", "on", "with", "at", "by", "from", "as", "into",
            "through", "during", "before", "after", "above", "below",
            "between", "under", "again", "further", "then", "once",
            "here", "there", "when", "where", "why", "how", "all",
            "each", "few", "more", "most", "other", "some", "such",
            "no", "nor", "not", "only", "own", "same", "so", "than",
            "too", "very", "just", "and", "but", "if", "or", "because",
            "until", "while", "this", "that", "these", "those", "i",
            "me", "my", "myself", "we", "our", "ours", "ourselves",
            "you", "your", "yours", "yourself", "yourselves", "he",
            "him", "his", "himself", "she", "her", "hers", "herself",
            "it", "its", "itself", "they", "them", "their", "theirs",
            "themselves", "what", "which", "who", "whom", "whose",
        ]
        .iter()
        .cloned()
        .collect();
        
        text_lower
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty() && w.len() > 2 && !stop_words.contains(w))
            .map(|w| w.to_string())
            .collect()
    }

    /// Generate embedding vector using character n-gram hashing
    /// Uses overlapping trigrams for better semantic matching than single-word hashing
    fn generate_simple_embedding(&self, text: &str) -> Embedding {
        let text_lower = text.to_lowercase();
        let keywords = self.extract_keywords(text);
        let dim = 256; // Higher dimensionality for better discrimination
        let mut vector = vec![0.0f32; dim];
        
        // 1. Word-level hashing (main signal)
        for keyword in keywords.iter().take(30) {
            let hash = self.simple_hash(keyword) as usize;
            let idx = hash % dim;
            vector[idx] += 1.0;
            // Spread to neighbors for semantic smoothing
            vector[(idx + 1) % dim] += 0.4;
            vector[(idx + dim - 1) % dim] += 0.4;
        }
        
        // 2. Character trigram hashing (catches partial matches, typos, similar words)
        let chars: Vec<char> = text_lower.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
        for window in chars.windows(3) {
            let trigram: String = window.iter().collect();
            let hash = self.simple_hash(&trigram) as usize;
            let idx = hash % dim;
            vector[idx] += 0.3;
        }
        
        // 3. Bigram word pairs (captures phrase-level meaning)
        let words: Vec<&str> = text_lower.split_whitespace().collect();
        for pair in words.windows(2) {
            let bigram = format!("{} {}", pair[0], pair[1]);
            let hash = self.simple_hash(&bigram) as usize;
            let idx = hash % dim;
            vector[idx] += 0.5;
        }
        
        // Normalize to unit vector for cosine similarity
        let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for x in &mut vector {
                *x /= magnitude;
            }
        }
        
        Embedding {
            vector,
            model: "trigram-hash-256".to_string(),
            dimensions: dim,
        }
    }
    
    /// Cache embedding for a memory id
    fn cache_embedding(&mut self, memory_id: &str, text: &str) {
        let embedding = self.generate_simple_embedding(text);
        self.embedding_cache.insert(memory_id.to_string(), embedding);
    }

    fn simple_hash(&self, s: &str) -> u64 {
        let mut hash: u64 = 5381;
        for byte in s.bytes() {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(byte as u64);
        }
        hash
    }

    /// Calculate cosine similarity between two vectors
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            0.0
        } else {
            dot_product / (magnitude_a * magnitude_b)
        }
    }

    /// Persist memory to storage
    async fn persist_memory(&self, memory: &Memory) -> anyhow::Result<()> {
        self.with_db(|conn| {
            let actions_json = serde_json::to_string(&memory.actions)?;
            let embedding_json = memory.embedding.as_ref()
                .map(|e| serde_json::to_string(e).unwrap_or_default());

            conn.execute(
                "INSERT OR REPLACE INTO memories (id, task_pattern, actions_json, success_rate, usage_count, created_at, embedding_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    memory.id,
                    memory.task_pattern,
                    actions_json,
                    memory.success_rate as f64,
                    memory.usage_count as i64,
                    memory.created_at.to_rfc3339(),
                    embedding_json,
                ],
            )?;
            Ok(())
        })
    }

    /// Persist preference to storage
    async fn persist_preference(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.with_db(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO user_preferences (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![key, value, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    /// Save current conversation context for later retrieval
    pub async fn save_context(&self, session_id: &str, context: &str) -> anyhow::Result<()> {
        self.with_db(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO memory_context (session_id, context_json, updated_at) VALUES (?1, ?2, ?3)",
                params![session_id, context, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    /// Load previously saved context
    pub fn load_context(&self, session_id: &str) -> anyhow::Result<Option<String>> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare("SELECT context_json FROM memory_context WHERE session_id = ?1")?;
            let result = stmt.query_row(params![session_id], |row| {
                let context: String = row.get(0)?;
                Ok(context)
            });

            match result {
                Ok(ctx) => Ok(Some(ctx)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(anyhow::anyhow!("DB error: {}", e)),
            }
        })
    }

    /// Get all recent context sessions
    pub fn list_recent_contexts(&self, limit: usize) -> anyhow::Result<Vec<(String, DateTime<Utc>)>> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT session_id, updated_at FROM memory_context ORDER BY updated_at DESC LIMIT ?1"
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                let session_id: String = row.get(0)?;
                let updated_at: String = row.get(1)?;
                let dt = updated_at.parse().unwrap_or_else(|_| Utc::now());
                Ok((session_id, dt))
            })?;

            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("DB error: {}", e))
        })
    }

    /// Clear old contexts (keep only recent)
    pub fn prune_old_contexts(&self, keep_count: usize) -> anyhow::Result<usize> {
        self.with_db(|conn| {
            // Get count
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM memory_context", [], |row| row.get(0))?;
            
            if count <= keep_count as i64 {
                return Ok(0);
            }

            // Delete oldest
            let deleted = conn.execute(
                "DELETE FROM memory_context WHERE session_id NOT IN (
                    SELECT session_id FROM memory_context ORDER BY updated_at DESC LIMIT ?1
                )",
                params![keep_count as i64],
            )?;

            Ok(deleted)
        })
    }

    /// Get statistics about the memory system
    pub fn get_stats(&self) -> MemoryStats {
        MemoryStats {
            total_memories: self.memories.len(),
            total_preferences: self.user_preferences.len(),
            avg_success_rate: if self.memories.is_empty() {
                0.0
            } else {
                self.memories.iter().map(|m| m.success_rate).sum::<f32>() / self.memories.len() as f32
            },
        }
    }
}

#[derive(Debug)]
pub struct MemoryStats {
    pub total_memories: usize,
    pub total_preferences: usize,
    pub avg_success_rate: f32,
}

impl Default for MemorySystem {
    fn default() -> Self {
        Self::new()
    }
}