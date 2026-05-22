use anyhow::Result;
use context_mode_store::{ContentStore, IndexOptions, SearchMode, SourceMatchMode};
use serde_json::json;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BatchCommand {
    pub label: String,
    pub command: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BatchParams {
    pub commands: Vec<BatchCommand>,
    pub queries: Option<Vec<String>>,
    pub concurrency: Option<u64>,
    pub timeout: Option<u64>,
}

struct CommandResult {
    label: String,
    output: String,
    line_count: usize,
    exit_code: Option<i32>,
}

pub async fn ctx_batch_execute(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: BatchParams = serde_json::from_value(params)?;
    let concurrency = params.concurrency.unwrap_or(1).clamp(1, 8) as usize;
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut tasks = Vec::with_capacity(params.commands.len());

    for command in params.commands {
        let permit = semaphore.clone().acquire_owned().await?;
        let timeout_ms = params.timeout;
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            run_command(command, timeout_ms).await
        }));
    }

    let mut results = Vec::with_capacity(tasks.len());
    for task in tasks {
        results.push(task.await??);
    }

    let combined = results
        .iter()
        .map(|result| format!("## {}\n{}", result.label, result.output))
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut store = ContentStore::in_memory()?;
    let indexed = store.index(IndexOptions {
        content: Some(combined),
        path: None,
        source: Some("batch".to_string()),
    })?;

    let mut text = String::new();
    text.push_str("Commands:\n");
    for result in &results {
        text.push_str(&format!(
            "- {}: {} lines, exit {:?}\n",
            result.label, result.line_count, result.exit_code
        ));
    }
    text.push_str(&format!("Indexed {} chunks.\n", indexed.total_chunks));

    if let Some(queries) = params.queries {
        text.push_str("\nSearch results:\n");
        for query in queries {
            text.push_str(&format!("\n## {query}\n"));
            let matches =
                store.search(&query, 5, None, SearchMode::Or, None, SourceMatchMode::Like)?;
            if matches.is_empty() {
                text.push_str("No matching sections found.\n");
            } else {
                for result in matches {
                    let snippet = crate::snippet::extract_snippet(
                        &result.content,
                        &query,
                        1500,
                        result.highlighted.as_deref(),
                    );
                    text.push_str(&format!(
                        "### {} ({})\n{}\n",
                        result.title, result.source, snippet
                    ));
                }
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": results.iter().any(|result| result.exit_code.unwrap_or(-1) != 0),
    }))
}

async fn run_command(command: BatchCommand, timeout_ms: Option<u64>) -> Result<CommandResult> {
    let mut cmd = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(&command.command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&command.command);
        cmd
    };

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let execution = cmd.output();
    let effective_timeout = timeout_ms.unwrap_or(30_000);
    let output = match timeout(Duration::from_millis(effective_timeout), execution).await {
        Ok(output) => output?,
        Err(_) => {
            let output = format!("Command timed out after {effective_timeout} ms");
            return Ok(CommandResult {
                label: command.label,
                line_count: output.lines().count(),
                output,
                exit_code: None,
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stderr.trim().is_empty() {
        stdout.to_string()
    } else if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    };
    let line_count = combined.lines().count();

    Ok(CommandResult {
        label: command.label,
        output: combined,
        line_count,
        exit_code: output.status.code(),
    })
}
