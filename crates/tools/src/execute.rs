use anyhow::{Context, Result, anyhow};
use context_mode_core::executor::{ExecuteOptions, PolyglotExecutor};
use context_mode_core::runtime::Language;
use context_mode_store::{ContentStore, IndexOptions, SearchMode, SourceMatchMode};
use serde_json::json;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecuteParams {
    pub language: String,
    pub code: String,
    pub timeout: Option<u64>,
    pub background: Option<bool>,
    pub intent: Option<String>,
    pub project_root: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecuteFileParams {
    pub path: String,
    pub language: String,
    pub code: String,
    pub timeout: Option<u64>,
    pub intent: Option<String>,
}

pub async fn ctx_execute(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: ExecuteParams = serde_json::from_value(params)?;
    let language = parse_language(&params.language)?;
    let project_root = params
        .project_root
        .clone()
        .unwrap_or_else(default_project_root);
    let code = instrument_code(language, &params.code, params.background.unwrap_or(false));
    let executor = PolyglotExecutor::new(project_root.clone());

    match executor
        .execute(ExecuteOptions {
            language,
            code,
            timeout_ms: params.timeout,
            background: params.background.unwrap_or(false),
            project_root,
            hard_cap_bytes: 1024 * 1024,
        })
        .await
    {
        Ok(mut result) => {
            result.stderr = strip_cm_markers(&result.stderr);
            let is_error = result.exit_code != 0 || result.timed_out;
            let text = format_exec_result(&result, params.intent.as_deref())?;
            Ok(tool_response(text, is_error))
        }
        Err(err) => {
            let text = if err.to_string().contains("timed out") {
                format!("Execution timed out: {err}")
            } else {
                format!("Execution failed: {err}")
            };
            Ok(tool_response(text, true))
        }
    }
}

pub async fn ctx_execute_file(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: ExecuteFileParams = serde_json::from_value(params)?;
    let language = parse_language(&params.language)?;
    let file_content = std::fs::read_to_string(&params.path)
        .with_context(|| format!("failed to read {}", params.path))?;
    let project_root = std::path::Path::new(&params.path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(default_project_root);
    let code = wrap_file_content(language, &file_content, &params.code)?;
    let code = instrument_code(language, &code, false);
    let executor = PolyglotExecutor::new(project_root.clone());

    match executor
        .execute(ExecuteOptions {
            language,
            code,
            timeout_ms: params.timeout,
            background: false,
            project_root,
            hard_cap_bytes: 1024 * 1024,
        })
        .await
    {
        Ok(mut result) => {
            result.stderr = strip_cm_markers(&result.stderr);
            let is_error = result.exit_code != 0 || result.timed_out;
            let text = format_exec_result(&result, params.intent.as_deref())?;
            Ok(tool_response(text, is_error))
        }
        Err(err) => {
            let text = if err.to_string().contains("timed out") {
                format!("Execution timed out: {err}")
            } else {
                format!("Execution failed: {err}")
            };
            Ok(tool_response(text, true))
        }
    }
}

fn parse_language(language: &str) -> Result<Language> {
    match language.to_ascii_lowercase().as_str() {
        "javascript" | "js" => Ok(Language::JavaScript),
        "typescript" | "ts" => Ok(Language::TypeScript),
        "python" | "py" => Ok(Language::Python),
        "shell" | "sh" | "bash" => Ok(Language::Shell),
        "ruby" | "rb" => Ok(Language::Ruby),
        "go" => Ok(Language::Go),
        "rust" | "rs" => Ok(Language::Rust),
        "php" => Ok(Language::Php),
        "perl" | "pl" => Ok(Language::Perl),
        "r" => Ok(Language::R),
        "elixir" | "exs" => Ok(Language::Elixir),
        other => Err(anyhow!("unsupported language: {other}")),
    }
}

fn instrument_code(language: Language, code: &str, background: bool) -> String {
    match language {
        Language::JavaScript | Language::TypeScript => instrument_node_code(code, background),
        _ => code.to_string(),
    }
}

fn instrument_node_code(code: &str, background: bool) -> String {
    let keepalive = if background {
        "\nsetInterval(()=>{},2147483647);"
    } else {
        ""
    };
    let template = r#"
let __cm_fs=0;
process.on('exit',()=>{if(__cm_fs>0)try{process.stderr.write('__CM_FS__:'+__cm_fs+'\n')}catch{}});
(function(){
  try{
    var f=typeof require!=='undefined'?require('fs'):null;
    if(!f)return;
    var ors=f.readFileSync;
    f.readFileSync=function(){var r=ors.apply(this,arguments);if(Buffer.isBuffer(r))__cm_fs+=r.length;else if(typeof r==='string')__cm_fs+=Buffer.byteLength(r);return r;};
    var orf=f.readFile;
    if(orf)f.readFile=function(){var a=Array.from(arguments),cb=a.pop();orf.apply(this,a.concat([function(e,d){if(!e&&d){if(Buffer.isBuffer(d))__cm_fs+=d.length;else if(typeof d==='string')__cm_fs+=Buffer.byteLength(d);}cb(e,d);}]));};
  }catch{}
})();
let __cm_net=0;
process.on('exit',()=>{if(__cm_net>0)try{process.stderr.write('__CM_NET__:'+__cm_net+'\n')}catch{}});
;(function(__cm_req){
const __cm_f=globalThis.fetch;
if(__cm_f)globalThis.fetch=async(...a)=>{const r=await __cm_f(...a);
try{const cl=r.clone();const b=await cl.arrayBuffer();__cm_net+=b.byteLength}catch{}
return r};
const __cm_hc=new Map();
const __cm_hm=new Set(['http','https','node:http','node:https']);
function __cm_wf(m,origFn){return function(...a){
  const li=a.length-1;
  if(li>=0&&typeof a[li]==='function'){const oc=a[li];a[li]=function(res){
    res.on('data',function(c){__cm_net+=c.length});oc(res);};}
  const req=origFn.apply(m,a);
  const oOn=req.on.bind(req);
  req.on=function(ev,cb,...r){
    if(ev==='response'){return oOn(ev,function(res){
      res.on('data',function(c){__cm_net+=c.length});cb(res);
    },...r);}
    return oOn(ev,cb,...r);
  };
  return req;
}}
var require=__cm_req?function(id){
  const m=__cm_req(id);
  if(!__cm_hm.has(id))return m;
  const k=id.replace('node:','');
  if(__cm_hc.has(k))return __cm_hc.get(k);
  const w=Object.create(m);
  if(typeof m.get==='function')w.get=__cm_wf(m,m.get);
  if(typeof m.request==='function')w.request=__cm_wf(m,m.request);
  __cm_hc.set(k,w);return w;
}:__cm_req;
if(__cm_req){if(__cm_req.resolve)require.resolve=__cm_req.resolve;
if(__cm_req.cache)require.cache=__cm_req.cache;}
async function __cm_main(){
__CM_USER_CODE__
}
__cm_main().catch(e=>{console.error(e);process.exitCode=1});__CM_KEEPALIVE__
})(typeof require!=='undefined'?require:null);
"#;
    template
        .replace("__CM_USER_CODE__", code)
        .replace("__CM_KEEPALIVE__", keepalive)
}

fn strip_cm_markers(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|line| !line.starts_with("__CM_NET__:") && !line.starts_with("__CM_FS__:"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_exec_result(
    result: &context_mode_core::types::ExecResult,
    intent: Option<&str>,
) -> Result<String> {
    if result.timed_out {
        return Ok("Execution timed out".to_string());
    }

    let mut stdout = summarize_stdout(&result.stdout, intent)?;
    if !result.stderr.trim().is_empty() {
        if !stdout.is_empty() {
            stdout.push_str("\n\n");
        }
        stdout.push_str("stderr:\n");
        stdout.push_str(result.stderr.trim());
    }
    if stdout.is_empty() {
        stdout = format!("Process exited with code {}", result.exit_code);
    }
    Ok(stdout)
}

fn summarize_stdout(stdout: &str, intent: Option<&str>) -> Result<String> {
    if stdout.len() <= 5000 {
        return Ok(stdout.to_string());
    }

    let mut store = ContentStore::in_memory()?;
    let indexed = store.index(IndexOptions {
        content: Some(stdout.to_string()),
        path: None,
        source: Some("execute-output".to_string()),
    })?;

    if let Some(intent) = intent {
        let results = store.search(intent, 5, None, SearchMode::Or, None, SourceMatchMode::Like)?;
        let snippets = results
            .iter()
            .map(|result| {
                crate::snippet::extract_snippet(
                    &result.content,
                    intent,
                    1500,
                    result.highlighted.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        return Ok(format!(
            "Output exceeded 5000 chars. Indexed {} chunks. Intent matches:\n{}",
            indexed.total_chunks,
            snippets.join("\n\n---\n")
        ));
    }

    Ok(format!(
        "Output exceeded 5000 chars. Indexed {} chunks from execute output. Use ctx_search to query it.",
        indexed.total_chunks
    ))
}

fn wrap_file_content(language: Language, file_content: &str, code: &str) -> Result<String> {
    let json_content = serde_json::to_string(file_content)?;
    Ok(match language {
        Language::JavaScript | Language::TypeScript => {
            format!("const FILE_CONTENT = {json_content};\n{code}")
        }
        Language::Python => format!("FILE_CONTENT = {json_content}\n{code}"),
        Language::Ruby => format!("FILE_CONTENT = {json_content}\n{code}"),
        Language::Shell => format!("FILE_CONTENT={}\n{code}", shell_quote(file_content)),
        Language::Php => format!("$FILE_CONTENT = {json_content};\n{code}"),
        Language::Perl => format!("my $FILE_CONTENT = {json_content};\n{code}"),
        Language::R => format!("FILE_CONTENT <- {json_content}\n{code}"),
        Language::Elixir => {
            format!("file_content = {json_content}\nFILE_CONTENT = file_content\n{code}")
        }
        Language::Go | Language::Rust => {
            format!("// FILE_CONTENT unavailable as a top-level constant for this language\n{code}")
        }
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn default_project_root() -> String {
    std::env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

fn tool_response(text: String, is_error: bool) -> serde_json::Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}
