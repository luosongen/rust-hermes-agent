# RL Training Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement RLTrainingTool with tinker-atropos integration for reinforcement learning training

**Architecture:** New RLTrainingTool in hermes-tools-extended, git submodule for tinker-atropos, process-based training execution

**Tech Stack:** Rust, tokio for async process management, serde for config/result serialization

---

## File Structure

```
crates/hermes-tools-extended/src/
├── rl_training.rs           # NEW: Main RLTrainingTool
```

```
third-party/
└── tinker-atropos/          # NEW: git submodule
```

---

## Task 1: Initialize tinker-atropos submodule

**Files:**
- Create: `third-party/tinker-atropos/` (git submodule)

- [ ] **Step 1: Add git submodule**

```bash
git submodule add git@github.com:.../tinker-atropos.git third-party/tinker-atropos
```

Note: Replace the Git URL with the actual tinker-atropos repository URL from the Python hermes-agent.

- [ ] **Step 2: Verify submodule**

```bash
ls -la third-party/tinker-atropos/
cat .gitmodules
```

Expected: Submodule directory exists with train.py and requirements.txt

- [ ] **Step 3: Commit**

```bash
git add .gitmodules third-party/tinker-atropos
git commit -m "chore: add tinker-atropos as git submodule"
```

---

## Task 2: Create RLTrainingTool core structure

**Files:**
- Create: `crates/hermes-tools-extended/src/rl_training.rs`

- [ ] **Step 1: Create data structures**

```rust
use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::Command;
use tokio::io::{AsyncBufExt, BufReader};

const RL_TRAINING_DIR: &str = ".config/hermes-agent/rl_training";
const TRAIN_SCRIPT: &str = "train.py";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub model: String,
    pub dataset: String,
    #[serde(default)]
    pub epochs: Option<u32>,
    #[serde(default)]
    pub batch_size: Option<u32>,
    #[serde(default)]
    pub learning_rate: Option<f64>,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub extra_args: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrainingState {
    Pending,
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub current_epoch: u32,
    pub total_epochs: u32,
    pub loss: Option<f64>,
    pub metrics: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStatus {
    pub id: String,
    pub model: String,
    pub dataset: String,
    pub state: TrainingState,
    pub started_at: f64,
    pub updated_at: f64,
    pub progress: Option<Progress>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    pub id: String,
    pub model_path: String,
    pub final_loss: Option<f64>,
    pub metrics: std::collections::HashMap<String, f64>,
    pub completed_at: f64,
    pub duration_secs: u64,
}
```

- [ ] **Step 2: Create RLTrainingTool struct**

```rust
#[derive(Clone)]
pub struct RLTrainingTool {
    training_dir: PathBuf,
    tinker_atropos_path: PathBuf,
    python_path: PathBuf,
}

impl RLTrainingTool {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let training_dir = PathBuf::from(home).join(RL_TRAINING_DIR);
        let tinker_atropos_path = PathBuf::from("third-party/tinker-atropos");

        // Try to find Python interpreter
        let python_path = std::env::var("PYTHON")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("PYTHON3").map(PathBuf::from))
            .unwrap_or_else(|_| PathBuf::from("python3"));

        Self {
            training_dir,
            tinker_atropos_path,
            python_path,
        }
    }

    fn training_dir_for_id(&self, id: &str) -> PathBuf {
        self.training_dir.join(id)
    }

    fn status_path(&self, id: &str) -> PathBuf {
        self.training_dir_for_id(id).join("status.json")
    }

    fn config_path(&self, id: &str) -> PathBuf {
        self.training_dir_for_id(id).join("config.json")
    }
}
```

- [ ] **Step 3: Implement Tool trait stubs**

```rust
#[async_trait]
impl Tool for RLTrainingTool {
    fn name(&self) -> &str { "rl_training" }

    fn description(&self) -> &str {
        "Reinforcement learning training tool using tinker-atropos"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {"properties": {"action": {"const": "start_training"}, "config_path": {"type": "string"}, "model": {"type": "string"}, "dataset": {"type": "string"}}, "required": ["action", "model", "dataset"]},
                {"properties": {"action": {"const": "get_status"}, "training_id": {"type": "string"}}, "required": ["action", "training_id"]},
                {"properties": {"action": {"const": "list_trainings"}}, "required": ["action"]},
                {"properties": {"action": {"const": "cancel_training"}, "training_id": {"type": "string"}}, "required": ["action", "training_id"]},
                {"properties": {"action": {"const": "get_results"}, "training_id": {"type": "string"}}, "required": ["action", "training_id"]}
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        #[serde(tag = "action", rename_all = "lowercase")]
        enum TrainingAction {
            StartTraining { config_path: Option<String>, model: String, dataset: String },
            GetStatus { training_id: String },
            ListTrainings,
            CancelTraining { training_id: String },
            GetResults { training_id: String },
        }

        let params: TrainingAction = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params {
            TrainingAction::StartTraining { config_path, model, dataset } => {
                // TODO: implement
                Ok(json!({ "status": "todo" }).to_string())
            }
            TrainingAction::GetStatus { training_id } => {
                // TODO: implement
                Ok(json!({ "status": "todo" }).to_string())
            }
            TrainingAction::ListTrainings => {
                // TODO: implement
                Ok(json!({ "status": "todo" }).to_string())
            }
            TrainingAction::CancelTraining { training_id } => {
                // TODO: implement
                Ok(json!({ "status": "todo" }).to_string())
            }
            TrainingAction::GetResults { training_id } => {
                // TODO: implement
                Ok(json!({ "status": "todo" }).to_string())
            }
        }
    }
}
```

- [ ] **Step 4: Add to lib.rs exports**

In `crates/hermes-tools-extended/src/lib.rs`:
```rust
pub mod rl_training;
pub use rl_training::RLTrainingTool;
```

And in `register_extended_tools`:
```rust
registry.register(RLTrainingTool::new());
```

- [ ] **Step 5: Build to verify compilation**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add crates/hermes-tools-extended/src/rl_training.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(rl_training): add RLTrainingTool skeleton"
```

---

## Task 3: Implement start_training action

**Files:**
- Modify: `crates/hermes-tools-extended/src/rl_training.rs`

- [ ] **Step 1: Add helper methods for status management**

```rust
impl RLTrainingTool {
    fn now() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64
    }

    async fn ensure_dir(&self) -> Result<(), ToolError> {
        tokio::fs::create_dir_all(&self.training_dir).await
            .map_err(|e| ToolError::Execution(format!("failed to create training dir: {}", e)))
    }

    async fn read_status(&self, id: &str) -> Result<TrainingStatus, ToolError> {
        let path = self.status_path(id);
        if !path.exists() {
            return Err(ToolError::Execution(format!("training '{}' not found", id)));
        }
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| ToolError::Execution(format!("failed to read status: {}", e)))?;
        serde_json::from_str(&content)
            .map_err(|e| ToolError::Execution(format!("failed to parse status: {}", e)))
    }

    async fn write_status(&self, status: &TrainingStatus) -> Result<(), ToolError> {
        let path = self.status_path(&status.id);
        let content = serde_json::to_string_pretty(status)
            .map_err(|e| ToolError::Execution(format!("failed to serialize status: {}", e)))?;
        tokio::fs::write(&path, &content).await
            .map_err(|e| ToolError::Execution(format!("failed to write status: {}", e)))
    }

    fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        format!("{:x}-{:x}", duration.as_secs(), duration.subsec_nanos())
    }
}
```

- [ ] **Step 2: Implement start_training logic**

```rust
async fn start_training(&self, config_path: Option<&str>, model: &str, dataset: &str) -> Result<String, ToolError> {
    self.ensure_dir().await?;

    // Verify tinker-atropos exists
    let train_script = self.tinker_atropos_path.join(TRAIN_SCRIPT);
    if !train_script.exists() {
        return Err(ToolError::Execution(
            "tinker-atropos not found. Run: git submodule update --init".to_string()
        ));
    }

    // Generate training ID
    let id = Self::generate_id();
    let training_dir = self.training_dir_for_id(&id);
    tokio::fs::create_dir_all(&training_dir).await
        .map_err(|e| ToolError::Execution(format!("failed to create training dir: {}", e)))?;

    // Create config
    let config = TrainingConfig {
        model: model.to_string(),
        dataset: dataset.to_string(),
        epochs: None,
        batch_size: None,
        learning_rate: None,
        output_dir: None,
        extra_args: std::collections::HashMap::new(),
    };

    let config_content = serde_json::to_string_pretty(&config)
        .map_err(|e| ToolError::Execution(format!("failed to serialize config: {}", e)))?;
    tokio::fs::write(self.config_path(&id), &config_content).await
        .map_err(|e| ToolError::Execution(format!("failed to write config: {}", e)))?;

    // Create initial status
    let now = Self::now();
    let status = TrainingStatus {
        id: id.clone(),
        model: model.to_string(),
        dataset: dataset.to_string(),
        state: TrainingState::Starting,
        started_at: now,
        updated_at: now,
        progress: None,
        error: None,
    };
    self.write_status(&status).await?;

    // Build command: python train.py --config {training_dir}/config.json
    let mut cmd = Command::new(&self.python_path);
    cmd.arg(train_script)
       .arg("--config").arg(self.config_path(&id));

    // Spawn training process
    let id_clone = id.clone();
    let training_dir_clone = training_dir.clone();
    let python_path = self.python_path.clone();

    tokio::spawn(async move {
        let mut cmd = Command::new(&python_path);
        cmd.arg(train_script)
           .arg("--config").arg(training_dir_clone.join("config.json"))
           .stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                // Store PID for cancellation
                if let Some(pid) = child.id() {
                    let pid_file = training_dir_clone.join("pid");
                    let _ = tokio::fs::write(pid_file, pid.to_string()).await;
                }

                // Wait for completion
                let status = child.wait().await;
                // Update status based on result
            }
            Err(e) => {
                // Log error
            }
        }
    });

    // Update status to Running
    let mut status = self.read_status(&id).await?;
    status.state = TrainingState::Running;
    status.updated_at = Self::now();
    self.write_status(&status).await?;

    Ok(id)
}
```

- [ ] **Step 3: Update execute to call start_training**

```rust
TrainingAction::StartTraining { config_path, model, dataset } => {
    let id = self.start_training(config_path.as_deref(), &model, &dataset).await?;
    Ok(json!({ "status": "ok", "training_id": id, "state": "Starting" }).to_string())
}
```

- [ ] **Step 4: Run build**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-tools-extended/src/rl_training.rs
git commit -m "feat(rl_training): implement start_training action"
```

---

## Task 4: Implement get_status and list_trainings actions

**Files:**
- Modify: `crates/hermes-tools-extended/src/rl_training.rs`

- [ ] **Step 1: Implement get_status**

```rust
async fn get_status(&self, training_id: &str) -> Result<TrainingStatus, ToolError> {
    let status = self.read_status(training_id).await?;
    Ok(status)
}
```

- [ ] **Step 2: Update execute for GetStatus**

```rust
TrainingAction::GetStatus { training_id } => {
    let status = self.get_status(&training_id).await?;
    Ok(json!({
        "id": status.id,
        "model": status.model,
        "dataset": status.dataset,
        "state": status.state,
        "started_at": status.started_at,
        "updated_at": status.updated_at,
        "progress": status.progress,
        "error": status.error
    }).to_string())
}
```

- [ ] **Step 3: Implement list_trainings**

```rust
async fn list_trainings(&self) -> Result<Vec<TrainingStatus>, ToolError> {
    self.ensure_dir().await?;

    let mut entries = tokio::fs::read_dir(&self.training_dir).await
        .map_err(|e| ToolError::Execution(format!("failed to read training dir: {}", e)))?;

    let mut trainings = Vec::new();
    while let Some(entry) = entries.next_entry().await
        .map_err(|e| ToolError::Execution(format!("dir read error: {}", e)))? {
        let path = entry.path();
        if path.is_dir() {
            let status_file = path.join("status.json");
            if status_file.exists() {
                if let Ok(content) = tokio::fs::read_to_string(&status_file).await {
                    if let Ok(status) = serde_json::from_str::<TrainingStatus>(&content) {
                        trainings.push(status);
                    }
                }
            }
        }
    }

    // Sort by started_at descending
    trainings.sort_by(|a, b| b.started_at.partial_cmp(&a.started_at).unwrap());

    Ok(trainings)
}
```

- [ ] **Step 4: Update execute for ListTrainings**

```rust
TrainingAction::ListTrainings => {
    let trainings = self.list_trainings().await?;
    Ok(json!({
        "trainings": trainings.iter().map(|t| json!({
            "id": t.id,
            "model": t.model,
            "state": t.state,
            "started_at": t.started_at
        })).collect::<Vec<_>>()
    }).to_string())
}
```

- [ ] **Step 5: Run build and tests**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add crates/hermes-tools-extended/src/rl_training.rs
git commit -m "feat(rl_training): implement get_status and list_trainings"
```

---

## Task 5: Implement cancel_training action

**Files:**
- Modify: `crates/hermes-tools-extended/src/rl_training.rs`

- [ ] **Step 1: Implement cancel_training**

```rust
async fn cancel_training(&self, training_id: &str) -> Result<(), ToolError> {
    let mut status = self.read_status(training_id).await?;

    if status.state != TrainingState::Running && status.state != TrainingState::Starting {
        return Err(ToolError::Execution(format!(
            "training '{}' is not running (current state: {:?})", training_id, status.state
        )));
    }

    // Read PID and send SIGTERM
    let pid_file = self.training_dir_for_id(training_id).join("pid");
    if pid_file.exists() {
        if let Ok(pid_str) = tokio::fs::read_to_string(&pid_file).await {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                // Send SIGTERM (on Unix)
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    if let Ok(_) = Command::new("kill").arg("-TERM").arg(pid.to_string()).output().await {
                        // Process signaled
                    }
                }
            }
        }
    }

    // Update status
    status.state = TrainingState::Cancelled;
    status.updated_at = Self::now();
    self.write_status(&status).await?;

    Ok(())
}
```

- [ ] **Step 2: Update execute for CancelTraining**

```rust
TrainingAction::CancelTraining { training_id } => {
    self.cancel_training(&training_id).await?;
    Ok(json!({ "status": "ok", "training_id": training_id, "state": "Cancelled" }).to_string())
}
```

- [ ] **Step 3: Run build**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-tools-extended/src/rl_training.rs
git commit -m "feat(rl_training): implement cancel_training action"
```

---

## Task 6: Implement get_results action

**Files:**
- Modify: `crates/hermes-tools-extended/src/rl_training.rs`

- [ ] **Step 1: Implement get_results**

```rust
async fn get_results(&self, training_id: &str) -> Result<TrainingResult, ToolError> {
    let status = self.read_status(training_id).await?;

    if status.state != TrainingState::Completed {
        return Err(ToolError::Execution(format!(
            "training '{}' is not completed (current state: {:?})", training_id, status.state
        )));
    }

    // Look for results file
    let results_file = self.training_dir_for_id(training_id).join("results.json");
    if !results_file.exists() {
        return Err(ToolError::Execution(format!("results for '{}' not found", training_id)));
    }

    let content = tokio::fs::read_to_string(&results_file).await
        .map_err(|e| ToolError::Execution(format!("failed to read results: {}", e)))?;

    let result: TrainingResult = serde_json::from_str(&content)
        .map_err(|e| ToolError::Execution(format!("failed to parse results: {}", e)))?;

    Ok(result)
}
```

- [ ] **Step 2: Update execute for GetResults**

```rust
TrainingAction::GetResults { training_id } => {
    let result = self.get_results(&training_id).await?;
    Ok(json!({
        "id": result.id,
        "model_path": result.model_path,
        "final_loss": result.final_loss,
        "metrics": result.metrics,
        "completed_at": result.completed_at,
        "duration_secs": result.duration_secs
    }).to_string())
}
```

- [ ] **Step 3: Run build**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-tools-extended/src/rl_training.rs
git commit -m "feat(rl_training): implement get_results action"
```

---

## Task 7: Add process monitoring and status updates

**Files:**
- Modify: `crates/hermes-tools-extended/src/rl_training.rs`

- [ ] **Step 1: Add status polling helper**

```rust
impl RLTrainingTool {
    async fn update_status_from_process(&self, id: &str) -> Result<(), ToolError> {
        let status = self.read_status(id).await?;

        // If training is not Running, nothing to update
        if status.state != TrainingState::Running {
            return Ok(());
        }

        // Check if process is still running via PID file
        let pid_file = self.training_dir_for_id(id).join("pid");
        if !pid_file.exists() {
            // No PID file means process completed or crashed
            let mut status = self.read_status(id).await?;
            status.state = TrainingState::Failed;
            status.error = Some("Process ended unexpectedly".to_string());
            status.updated_at = Self::now();
            self.write_status(&status).await?;
        }

        // TODO: Parse progress from logs if available

        Ok(())
    }
}
```

- [ ] **Step 2: Run build and tests**

Run: `cargo build -p hermes-tools-extended && cargo test -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p hermes-tools-extended`
Expected: No warnings

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-tools-extended/src/rl_training.rs
git commit -m "feat(rl_training): add process monitoring for status updates"
```

---

## Task 8: Add unit tests

**Files:**
- Create: `crates/hermes-tools-extended/tests/test_rl_training.rs`

- [ ] **Step 1: Create test file**

```rust
use hermes_tools_extended::rl_training::{TrainingConfig, TrainingState, TrainingStatus, RLTrainingTool};
use hermes_core::ToolError;

#[test]
fn test_training_config_serialization() {
    let config = TrainingConfig {
        model: "gpt2".to_string(),
        dataset: "/data/train.jsonl".to_string(),
        epochs: Some(100),
        batch_size: Some(32),
        learning_rate: Some(0.001),
        output_dir: None,
        extra_args: std::collections::HashMap::new(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: TrainingConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.model, "gpt2");
    assert_eq!(deserialized.epochs, Some(100));
}

#[test]
fn test_training_state_transitions() {
    assert_eq!(TrainingState::Pending, TrainingState::Pending);
    assert!(TrainingState::Pending != TrainingState::Running);
}

#[tokio::test]
async fn test_rl_training_tool_initialization() {
    let tool = RLTrainingTool::new();
    assert_eq!(tool.name(), "rl_training");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p hermes-tools-extended -- --nocapture`
Expected: ALL PASS

- [ ] **Step 3: Final commit**

```bash
git add crates/hermes-tools-extended/tests/test_rl_training.rs
git commit -m "feat(rl_training): add unit tests"
```

---

## Self-Review Checklist

- [ ] Spec coverage: All 5 actions have corresponding tasks
- [ ] No placeholders: All code is complete
- [ ] Type consistency: TrainingConfig, TrainingStatus, TrainingResult match spec
- [ ] Tests: Each major action has test verification
- [ ] Git submodule: tinker-atropos properly initialized
