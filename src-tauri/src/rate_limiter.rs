//! Rate Limiter - Intelligent API Rate Limit Management
//!
//! Tracks token usage, implements exponential backoff, and auto-retries
//! when rate limits are hit. Ensures context/memory is preserved during retries.

use crate::storage::Usage;
use std::collections::VecDeque;

use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Anthropic rate limits — use generous defaults to avoid unnecessary throttling.
/// The API itself will return 429 if you actually hit the limit, and we handle
/// that with automatic retry + exponential backoff. No need to self-throttle.
const BUILD_TIER_INPUT_TPM: u32 = 80_000;
const BUILD_TIER_OUTPUT_TPM: u32 = 16_000;
const SCALE_TIER_INPUT_TPM: u32 = 200_000;
const SCALE_TIER_OUTPUT_TPM: u32 = 40_000;

/// Safety margin - only throttle when very close to limit
const SAFETY_MARGIN: f32 = 0.95;

/// Maximum retry attempts
const MAX_RETRIES: u32 = 5;

/// Base delay for exponential backoff (ms)
const BASE_RETRY_DELAY_MS: u64 = 1000;

/// Maximum retry delay (ms)
const MAX_RETRY_DELAY_MS: u64 = 60_000;

/// Token bucket entry
#[derive(Debug, Clone)]
struct TokenBucketEntry {
    timestamp: Instant,
    input_tokens: u32,
    output_tokens: u32,
}

/// Rate limit status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RateLimitStatus {
    /// Safe to proceed
    Safe,
    /// Approaching limit - should throttle
    Throttle,
    /// At limit - must wait
    Limited,
}

/// Retry state for preserving context
#[derive(Debug, Clone)]
pub struct RetryState {
    pub attempt: u32,
    pub last_error: String,
    pub context_snapshot: Vec<crate::api::Message>,
    pub accumulated_usage: Usage,
}

/// Intelligent rate limiter with exponential backoff
pub struct RateLimiter {
    /// Token usage history (sliding window)
    token_history: Mutex<VecDeque<TokenBucketEntry>>,
    /// Current tier
    tier: Mutex<RateLimitTier>,
    /// Current rate limit status
    status: Mutex<RateLimitStatus>,
    /// Active retry state (if any)
    retry_state: Mutex<Option<RetryState>>,
    /// Total tokens used (all time)
    total_input_tokens: Mutex<u64>,
    total_output_tokens: Mutex<u64>,
}

#[derive(Debug, Clone, Copy)]
pub enum RateLimitTier {
    Build,  // 30k input TPM
    Scale,  // 60k input TPM
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            token_history: Mutex::new(VecDeque::new()),
            tier: Mutex::new(RateLimitTier::Build),
            status: Mutex::new(RateLimitStatus::Safe),
            retry_state: Mutex::new(None),
            total_input_tokens: Mutex::new(0),
            total_output_tokens: Mutex::new(0),
        }
    }

    /// Get current rate limits based on tier
    pub async fn get_limits(&self) -> (u32, u32) {
        let tier = *self.tier.lock().await;
        match tier {
            RateLimitTier::Build => (
                (BUILD_TIER_INPUT_TPM as f32 * SAFETY_MARGIN) as u32,
                (BUILD_TIER_OUTPUT_TPM as f32 * SAFETY_MARGIN) as u32,
            ),
            RateLimitTier::Scale => (
                (SCALE_TIER_INPUT_TPM as f32 * SAFETY_MARGIN) as u32,
                (SCALE_TIER_OUTPUT_TPM as f32 * SAFETY_MARGIN) as u32,
            ),
        }
    }

    /// Update tier (call if user upgrades)
    pub async fn set_tier(&self, tier: RateLimitTier) {
        *self.tier.lock().await = tier;
        println!("[rate_limiter] Tier updated to {:?}", tier);
    }

    /// Record token usage from an API call
    pub async fn record_usage(&self, usage: &Usage) {
        let entry = TokenBucketEntry {
            timestamp: Instant::now(),
            input_tokens: usage.total_input(),
            output_tokens: usage.output_tokens,
        };

        let mut history = self.token_history.lock().await;
        history.push_back(entry);

        // Update totals
        let mut total_input = self.total_input_tokens.lock().await;
        let mut total_output = self.total_output_tokens.lock().await;
        *total_input += usage.total_input() as u64;
        *total_output += usage.output_tokens as u64;

        // Clean old entries (> 60 seconds)
        let cutoff = Instant::now() - Duration::from_secs(60);
        while let Some(front) = history.front() {
            if front.timestamp < cutoff {
                history.pop_front();
            } else {
                break;
            }
        }

        // Update status
        drop(history); // Release lock before calling update_status
        self.update_status().await;
    }

    /// Get current token usage in the sliding window
    pub async fn get_current_usage(&self) -> (u32, u32) {
        let history = self.token_history.lock().await;
        let cutoff = Instant::now() - Duration::from_secs(60);

        let input: u32 = history
            .iter()
            .filter(|e| e.timestamp >= cutoff)
            .map(|e| e.input_tokens)
            .sum();

        let output: u32 = history
            .iter()
            .filter(|e| e.timestamp >= cutoff)
            .map(|e| e.output_tokens)
            .sum();

        (input, output)
    }

    /// Update rate limit status based on current usage
    async fn update_status(&self) {
        let (current_input, current_output) = self.get_current_usage().await;
        let (limit_input, limit_output) = self.get_limits().await;

        let input_ratio = current_input as f32 / limit_input as f32;
        let output_ratio = current_output as f32 / limit_output as f32;

        let new_status = if input_ratio >= 1.0 || output_ratio >= 1.0 {
            RateLimitStatus::Limited
        } else if input_ratio >= SAFETY_MARGIN || output_ratio >= SAFETY_MARGIN {
            RateLimitStatus::Throttle
        } else {
            RateLimitStatus::Safe
        };

        let mut status = self.status.lock().await;
        if *status != new_status {
            println!(
                "[rate_limiter] Status: {:?} (input: {}/{}, output: {}/{})",
                new_status, current_input, limit_input, current_output, limit_output
            );
            *status = new_status;
        }
    }

    /// Get current status
    pub async fn get_status(&self) -> RateLimitStatus {
        *self.status.lock().await
    }

    /// Calculate wait time before next request (if throttled)
    pub async fn get_wait_time(&self) -> Duration {
        let history = self.token_history.lock().await;
        if history.is_empty() {
            return Duration::ZERO;
        }

        // Find oldest entry within window
        let now = Instant::now();
        let window_start = now - Duration::from_secs(60);

        if let Some(oldest) = history.iter().find(|e| e.timestamp >= window_start) {
            // Wait until oldest entry expires from window
            let expires_at = oldest.timestamp + Duration::from_secs(60);
            if expires_at > now {
                return expires_at - now;
            }
        }

        Duration::ZERO
    }

    /// Wait if necessary before making a request
    pub async fn throttle_if_needed(&self) {
        let status = self.get_status().await;

        match status {
            RateLimitStatus::Safe => {}
            RateLimitStatus::Throttle => {
                let wait = self.get_wait_time().await;
                if wait > Duration::ZERO {
                    println!("[rate_limiter] Throttling for {:?}", wait);
                    sleep(wait).await;
                }
            }
            RateLimitStatus::Limited => {
                let wait = self.get_wait_time().await;
                let wait = wait.max(Duration::from_secs(5));
                println!("[rate_limiter] Rate limited! Waiting for {:?}", wait);
                sleep(wait).await;
            }
        }
    }

    /// Store retry state before attempting a request
    pub async fn begin_retry_attempt(
        &self,
        context: Vec<crate::api::Message>,
    ) -> RetryState {
        let mut state = self.retry_state.lock().await;
        let new_state = RetryState {
            attempt: 1,
            last_error: String::new(),
            context_snapshot: context,
            accumulated_usage: Usage::default(),
        };
        *state = Some(new_state.clone());
        new_state
    }

    /// Update retry state after a failed attempt
    pub async fn update_retry_state(&self, error: &str, usage: Option<&Usage>) {
        let mut state = self.retry_state.lock().await;
        if let Some(ref mut s) = *state {
            s.attempt += 1;
            s.last_error = error.to_string();
            if let Some(u) = usage {
                s.accumulated_usage.input_tokens += u.input_tokens;
                s.accumulated_usage.output_tokens += u.output_tokens;
                s.accumulated_usage.cache_creation_input_tokens += u.cache_creation_input_tokens;
                s.accumulated_usage.cache_read_input_tokens += u.cache_read_input_tokens;
            }
        }
    }

    /// Get current retry state
    pub async fn get_retry_state(&self) -> Option<RetryState> {
        self.retry_state.lock().await.clone()
    }

    /// Clear retry state after success
    pub async fn clear_retry_state(&self) {
        *self.retry_state.lock().await = None;
    }

    /// Calculate exponential backoff delay
    pub fn calculate_backoff(attempt: u32) -> Duration {
        let delay = BASE_RETRY_DELAY_MS * 2_u64.pow(attempt.min(5));
        let delay = delay.min(MAX_RETRY_DELAY_MS);
        // Add jitter (±25%)
        let jitter = (delay as f32 * 0.25) as u64;
        let delay = delay + rand::random::<u64>() % (jitter * 2 + 1) - jitter;
        Duration::from_millis(delay)
    }

    /// Execute a function with automatic retry on rate limit errors
    pub async fn execute_with_retry<F, Fut, T>(
        &self,
        context: Vec<crate::api::Message>,
        operation: F,
    ) -> Result<T, String>
    where
        F: Fn(u32) -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let _state = self.begin_retry_attempt(context).await;

        for attempt in 1..=MAX_RETRIES {
            // Wait if we're hitting rate limits
            self.throttle_if_needed().await;

            match operation(attempt).await {
                Ok(result) => {
                    self.clear_retry_state().await;
                    return Ok(result);
                }
                Err(e) => {
                    let is_rate_limit = e.contains("rate limit")
                        || e.contains("429")
                        || e.contains("too many requests")
                        || e.contains("tokens per minute");

                    if !is_rate_limit || attempt >= MAX_RETRIES {
                        self.clear_retry_state().await;
                        return Err(e);
                    }

                    // Exponential backoff
                    let delay = Self::calculate_backoff(attempt);
                    println!(
                        "[rate_limiter] Rate limit hit (attempt {}/{}). Retrying in {:?}...",
                        attempt, MAX_RETRIES, delay
                    );

                    self.update_retry_state(&e, None).await;
                    sleep(delay).await;
                }
            }
        }

        self.clear_retry_state().await;
        Err("Max retries exceeded".to_string())
    }

    /// Get statistics
    pub async fn get_stats(&self) -> RateLimiterStats {
        let (current_input, current_output) = self.get_current_usage().await;
        let (limit_input, limit_output) = self.get_limits().await;
        let total_input = *self.total_input_tokens.lock().await;
        let total_output = *self.total_output_tokens.lock().await;

        RateLimiterStats {
            current_input_tpm: current_input,
            current_output_tpm: current_output,
            limit_input_tpm: limit_input,
            limit_output_tpm: limit_output,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            status: self.get_status().await,
            retry_attempts: self
                .retry_state
                .lock()
                .await
                .as_ref()
                .map(|s| s.attempt)
                .unwrap_or(0),
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for monitoring
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub current_input_tpm: u32,
    pub current_output_tpm: u32,
    pub limit_input_tpm: u32,
    pub limit_output_tpm: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub status: RateLimitStatus,
    pub retry_attempts: u32,
}

impl RateLimiterStats {
    /// Format as human-readable string
    pub fn format(&self) -> String {
        format!(
            "Rate Limit: {}/{} input TPM, {}/{} output TPM | Status: {:?} | Total: {}M input, {}M output tokens",
            self.current_input_tpm,
            self.limit_input_tpm,
            self.current_output_tpm,
            self.limit_output_tpm,
            self.status,
            self.total_input_tokens / 1_000_000,
            self.total_output_tokens / 1_000_000
        )
    }
}
