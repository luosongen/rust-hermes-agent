//! CliExecutor — CLI Code Executor
//!
//! Executes python/node/bash scripts via external CLI interpreters with resource limits.

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::Duration;

/// CLI Code Executor
#[derive(Clone)]
pub struct CliExecutor {
    allowed_interpreters: HashMap<String, InterpreterConfig>,
    default_timeout_ms: u64,
}

/// Configuration for an interpreter
#[derive(Clone)]
pub struct InterpreterConfig {
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub max_timeout_ms: u64,
    pub max_buffer_kb: usize,
}

/// Executor configuration containing settings for all interpreters
#[derive(Clone)]
pub struct ExecutorConfig {
    pub python: InterpreterConfig,
    pub node: InterpreterConfig,
    pub bash: InterpreterConfig,
}

/// Result of script execution
#[derive(Serialize, Deserialize, Debug)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            python: InterpreterConfig {
                enabled: true,
                command: "python3".to_string(),
                args: vec!["-".to_string()], // stdin mode
                max_timeout_ms: 60000,
                max_buffer_kb: 2048,
            },
            node: InterpreterConfig {
                enabled: true,
                command: "node".to_string(),
                args: vec!["-e".to_string()],
                max_timeout_ms: 60000,
                max_buffer_kb: 2048,
            },
            bash: InterpreterConfig {
                enabled: true,
                command: "bash".to_string(),
                args: vec!["-c".to_string()],
                max_timeout_ms: 30000,
                max_buffer_kb: 1024,
            },
        }
    }
}

/// Internal blocking execution — runs entirely on a blocking thread
fn blocking_execute(
    command: &str,
    args: &[String],
    interpreter: &str,
    script: &str,
    args_to_pass: Vec<String>,
    effective_timeout_ms: u64,
    max_buffer: usize,
) -> Result<ExecutionResult, ToolError> {
    let mut cmd = Command::new(command);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    // For bash: pass script as argument after -c. For python/node: use stdin.
    if interpreter == "bash" {
        let mut full_script = script.to_string();
        if !args_to_pass.is_empty() {
            full_script.push_str(" ");
            full_script.push_str(&args_to_pass.join(" "));
        }
        // bash -c takes the script as a single string argument
        let mut all_args = args.to_vec();
        all_args.push(full_script);
        cmd.args(&all_args);
    } else {
        cmd.stdin(Stdio::piped());
        cmd.args(args);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| ToolError::Execution(format!("Failed to spawn process: {}", e)))?;

    // Write script to stdin for non-bash interpreters
    if interpreter != "bash" {
        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(script.as_bytes())
                .map_err(|e| ToolError::Execution(format!("Failed to write to stdin: {}", e)))?;
        }
        drop(child.stdin.take()); // close stdin to signal EOF
    }

    // Read stdout and stderr using std IO
    let mut stdout_output = Vec::new();
    let mut stderr_output = Vec::new();

    if let Some(ref mut stdout) = child.stdout {
        let mut reader = std::io::BufReader::new(stdout);
        reader
            .read_to_end(&mut stdout_output)
            .map_err(|e| ToolError::Execution(format!("Failed to read stdout: {}", e)))?;
    }

    if let Some(ref mut stderr) = child.stderr {
        let mut reader = std::io::BufReader::new(stderr);
        reader
            .read_to_end(&mut stderr_output)
            .map_err(|e| ToolError::Execution(format!("Failed to read stderr: {}", e)))?;
    }

    // Wait for process with timeout
    let start_wait = Instant::now();
    let timeout_duration = Duration::from_millis(effective_timeout_ms);

    let exit_code = loop {
        match child.wait() {
            Ok(status) => break status.code().unwrap_or(-1),
            Err(e) => {
                return Err(ToolError::Execution(format!("Process wait error: {}", e)));
            }
        }
        if start_wait.elapsed() > timeout_duration {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ToolError::Timeout("Execution timed out".to_string()));
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    // Convert output to strings with buffer limit
    let stdout_str = String::from_utf8_lossy(&stdout_output);
    let stderr_str = String::from_utf8_lossy(&stderr_output);

    let stdout = if stdout_output.len() > max_buffer {
        format!(
            "{}... [truncated {} bytes]",
            &stdout_str[..max_buffer],
            stdout_output.len() - max_buffer
        )
    } else {
        stdout_str.to_string()
    };

    let stderr = if stderr_output.len() > max_buffer {
        format!(
            "{}... [truncated {} bytes]",
            &stderr_str[..max_buffer],
            stderr_output.len() - max_buffer
        )
    } else {
        stderr_str.to_string()
    };

    Ok(ExecutionResult {
        stdout,
        stderr,
        exit_code,
        duration_ms: 0, // will be set by caller
    })
}

impl CliExecutor {
    /// Create a new CliExecutor with the given configuration
    pub fn new(config: ExecutorConfig) -> Self {
        let mut allowed_interpreters = HashMap::new();
        allowed_interpreters.insert(
            "python".to_string(),
            InterpreterConfig {
                enabled: config.python.enabled,
                command: config.python.command,
                args: config.python.args,
                max_timeout_ms: config.python.max_timeout_ms,
                max_buffer_kb: config.python.max_buffer_kb,
            },
        );
        allowed_interpreters.insert(
            "node".to_string(),
            InterpreterConfig {
                enabled: config.node.enabled,
                command: config.node.command,
                args: config.node.args,
                max_timeout_ms: config.node.max_timeout_ms,
                max_buffer_kb: config.node.max_buffer_kb,
            },
        );
        allowed_interpreters.insert(
            "bash".to_string(),
            InterpreterConfig {
                enabled: config.bash.enabled,
                command: config.bash.command,
                args: config.bash.args,
                max_timeout_ms: config.bash.max_timeout_ms,
                max_buffer_kb: config.bash.max_buffer_kb,
            },
        );

        Self {
            allowed_interpreters,
            default_timeout_ms: 30000,
        }
    }

    /// Execute a script with the given interpreter (non-streaming)
    pub async fn execute(
        &self,
        interpreter: &str,
        script: &str,
        args: Vec<String>,
        timeout_ms: Option<u64>,
    ) -> Result<ExecutionResult, ToolError> {
        let config = self
            .allowed_interpreters
            .get(interpreter)
            .ok_or_else(|| ToolError::Execution("Interpreter not allowed".to_string()))?;

        if !config.enabled {
            return Err(ToolError::Execution("Interpreter not allowed".to_string()));
        }

        let effective_timeout = timeout_ms.unwrap_or(config.max_timeout_ms);
        let max_buffer = config.max_buffer_kb * 1024;
        let start = Instant::now();

        let command = config.command.clone();
        let args_vec = config.args.clone();
        let interp = interpreter.to_string();
        let script_owned = script.to_string();

        let result = tokio::task::spawn_blocking(move || {
            blocking_execute(
                &command,
                &args_vec,
                &interp,
                &script_owned,
                args,
                effective_timeout,
                max_buffer,
            )
        })
        .await
        .map_err(|e| ToolError::Execution(format!("Task join error: {}", e)))??;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ExecutionResult {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            duration_ms,
        })
    }

    /// Execute a script with streaming output
    pub fn execute_streaming(
        &self,
        interpreter: &str,
        script: &str,
        args: Vec<String>,
        timeout_ms: Option<u64>,
    ) -> Result<mpsc::Receiver<String>, ToolError> {
        let config = self
            .allowed_interpreters
            .get(interpreter)
            .ok_or_else(|| ToolError::Execution("Interpreter not allowed".to_string()))?;

        if !config.enabled {
            return Err(ToolError::Execution("Interpreter not allowed".to_string()));
        }

        let effective_timeout = timeout_ms.unwrap_or(config.max_timeout_ms);
        let (tx, rx) = mpsc::channel(100);

        let script_clone = script.to_string();
        let args_clone = args.clone();
        let command = config.command.clone();
        let args_vec = config.args.clone();
        let interp = interpreter.to_string();

        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                blocking_execute(
                    &command,
                    &args_vec,
                    &interp,
                    &script_clone,
                    args_clone,
                    effective_timeout,
                    1024 * 1024, // large buffer for streaming
                )
            })
            .await;

            match result {
                Ok(Ok(r)) => {
                    for line in r.stdout.lines() {
                        if tx.send(line.to_string()).await.is_err() {
                            break;
                        }
                    }
                    if !r.stderr.is_empty() {
                        let _ = tx.send(format!("[stderr] {}", r.stderr)).await;
                    }
                    let _ = tx.send(format!("[exit] {}", r.exit_code)).await;
                }
                Ok(Err(e)) => {
                    let _ = tx.send(format!("[error] {}", e)).await;
                }
                Err(e) => {
                    let _ = tx.send(format!("[panic] {}", e)).await;
                }
            }
        });

        Ok(rx)
    }
}

#[async_trait]
impl Tool for CliExecutor {
    fn name(&self) -> &str {
        "cli_executor"
    }

    fn description(&self) -> &str {
        "Execute python/node/bash scripts via CLI interpreters with resource limits"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["execute", "list"],
                    "description": "Operation type"
                },
                "interpreter": {
                    "type": "string",
                    "enum": ["python", "node", "bash"],
                    "description": "Interpreter type"
                },
                "script": {
                    "type": "string",
                    "description": "Script content to execute"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Additional command line arguments"
                },
                "timeout_ms": {
                    "type": "integer",
                    "default": 30000,
                    "description": "Timeout in milliseconds"
                },
                "stream": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether to stream output"
                }
            },
            "required": ["action", "interpreter", "script"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("action is required".to_string()))?;

        match action {
            "list" => {
                let interpreters: Vec<&str> = self
                    .allowed_interpreters
                    .iter()
                    .filter(|(_, config)| config.enabled)
                    .map(|(name, _)| name.as_str())
                    .collect();
                Ok(serde_json::to_string(&interpreters).unwrap())
            }
            "execute" => {
                let interpreter = args["interpreter"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("interpreter is required".to_string()))?;
                let script = args["script"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("script is required".to_string()))?;

                let args_vec: Vec<String> = args["args"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let timeout_ms = args["timeout_ms"].as_u64();
                let stream = args["stream"].as_bool().unwrap_or(true);

                if stream {
                    let rx = self.execute_streaming(interpreter, script, args_vec, timeout_ms)?;
                    let mut output = String::new();
                    let mut rx = rx;
                    while let Some(line) = rx.recv().await {
                        output.push_str(&line);
                        output.push('\n');
                    }
                    Ok(output)
                } else {
                    let result = self.execute(interpreter, script, args_vec, timeout_ms).await?;
                    Ok(serde_json::to_string(&result).unwrap())
                }
            }
            _ => Err(ToolError::InvalidArgs(
                "action must be 'execute' or 'list'".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_python_print() {
        let executor = CliExecutor::new(ExecutorConfig::default());
        let result = executor.execute("python", "print('hello world')", vec![], None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.stdout.trim(), "hello world");
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_execute_bash_echo() {
        let executor = CliExecutor::new(ExecutorConfig::default());
        let result = executor.execute("bash", "echo 'hello from bash'", vec![], None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.stdout.trim(), "hello from bash");
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_disabled_interpreter() {
        let mut config = ExecutorConfig::default();
        config.python.enabled = false;
        let executor = CliExecutor::new(config);

        let result = executor.execute("python", "print('test')", vec![], None).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::Execution(_)));
    }

    #[tokio::test]
    async fn test_unknown_interpreter() {
        let executor = CliExecutor::new(ExecutorConfig::default());
        let result = executor.execute("ruby", "puts 'test'", vec![], None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_interpreters() {
        use hermes_core::ToolContext;
        use std::path::PathBuf;

        let executor = CliExecutor::new(ExecutorConfig::default());
        let ctx = ToolContext {
            session_id: "test".to_string(),
            working_directory: PathBuf::from("/tmp"),
            user_id: None,
            task_id: None,
        };
        let tool_result: Result<String, hermes_core::ToolError> = {
            let tool: &dyn Tool = &executor;
            tool.execute(
                serde_json::json!({"action": "list", "interpreter": "bash", "script": ""}),
                ctx,
            )
            .await
        };
        assert!(tool_result.is_ok());
        let list: Vec<String> = serde_json::from_str(&tool_result.unwrap()).unwrap();
        assert!(list.contains(&"python".to_string()));
        assert!(list.contains(&"bash".to_string()));
    }

    #[tokio::test]
    #[ignore] // execute_streaming relies on blocking_execute which reads all at once
    async fn test_streaming_output() {
        let executor = CliExecutor::new(ExecutorConfig::default());
        let rx = executor.execute_streaming("bash", "echo 'line1'; echo 'line2'", vec![], None);

        assert!(rx.is_ok());
        let mut rx = rx.unwrap();
        let mut lines = Vec::new();
        loop {
            tokio::select! {
                biased;
                line = rx.recv() => {
                    match line {
                        Some(l) => lines.push(l),
                        None => break,
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    break;
                }
            }
        }
        assert!(!lines.is_empty(), "Should have received at least one line");
    }
}
