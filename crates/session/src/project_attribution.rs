use crate::types::{AttributionContext, AttributionSource, ProjectAttribution, SessionEvent};
use regex::Regex;
use std::path::{Path, PathBuf};

pub const PROJECT_ATTRIBUTION_VERSION: i32 = 1;
pub const HIGH_CONFIDENCE_THRESHOLD: f64 = 0.80;

pub mod attribution_confidence {
    pub const DIRECT_EVENT_PROJECT_DIR: f64 = 1.0;
    pub const PATH_SIGNAL: f64 = 0.90;
    pub const CWD: f64 = 0.75;
    pub const LATEST_SESSION_PROJECT: f64 = 0.55;
    pub const UNKNOWN: f64 = 0.0;
}

pub fn resolve_project_attribution(
    event: &SessionEvent,
    context: &AttributionContext,
) -> ProjectAttribution {
    if let Some(project_dir) = event
        .project_dir
        .as_deref()
        .map(normalize_project_dir)
        .filter(|s| !s.is_empty())
    {
        return ProjectAttribution {
            project_dir,
            source: AttributionSource::DirectEventProjectDir,
            confidence: attribution_confidence::DIRECT_EVENT_PROJECT_DIR,
            reason: "event project_dir".to_string(),
        };
    }

    if let Some(path) = extract_path_signal(event).and_then(|p| absolutize_path(&p, context)) {
        if let Some(project_dir) = infer_project_from_absolute_path(&path, context) {
            return ProjectAttribution {
                project_dir,
                source: AttributionSource::PathSignal,
                confidence: attribution_confidence::PATH_SIGNAL,
                reason: "path signal".to_string(),
            };
        }
    }

    if let Some(cwd) = context
        .cwd
        .as_deref()
        .map(normalize_project_dir)
        .filter(|s| !s.is_empty())
    {
        return ProjectAttribution {
            project_dir: cwd,
            source: AttributionSource::Cwd,
            confidence: attribution_confidence::CWD,
            reason: "cwd fallback".to_string(),
        };
    }

    if let Some(project_dir) = context
        .latest_session_project_dir
        .as_deref()
        .map(normalize_project_dir)
        .filter(|s| !s.is_empty())
    {
        return ProjectAttribution {
            project_dir,
            source: AttributionSource::LatestSessionProject,
            confidence: attribution_confidence::LATEST_SESSION_PROJECT,
            reason: "latest session project".to_string(),
        };
    }

    ProjectAttribution {
        project_dir: String::new(),
        source: AttributionSource::Unknown,
        confidence: attribution_confidence::UNKNOWN,
        reason: "no project signal".to_string(),
    }
}

pub fn resolve_project_attributions(
    events: &[SessionEvent],
    context: &AttributionContext,
) -> Vec<ProjectAttribution> {
    events
        .iter()
        .map(|event| resolve_project_attribution(event, context))
        .collect()
}

pub fn confidence_to_percent(confidence: f64) -> i32 {
    (clamp_confidence(confidence) * 100.0).round() as i32
}

pub fn is_high_confidence_attribution(confidence: f64) -> bool {
    clamp_confidence(confidence) >= HIGH_CONFIDENCE_THRESHOLD
}

pub fn normalize_project_dir(project_dir: &str) -> String {
    project_dir
        .trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn clamp_confidence(confidence: f64) -> f64 {
    if confidence.is_finite() {
        confidence.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn extract_path_signal(event: &SessionEvent) -> Option<String> {
    let text = format!("{}\n{}", event.event_type, event.data);
    let re = Regex::new(r#"(?i)(?:file_path|path|cwd|source)[:=]\s*["']?([^"'\s,;<>]+)"#).ok()?;
    re.captures(&text)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .or_else(|| {
            let path_re = Regex::new(r#"(?:[A-Za-z]:)?[/\\][^\s"'<>]+"#).ok()?;
            path_re.find(&text).map(|m| m.as_str().to_string())
        })
}

fn absolutize_path(raw_path: &str, context: &AttributionContext) -> Option<String> {
    let normalized = normalize_project_dir(raw_path);
    if normalized.is_empty() {
        return None;
    }
    let path = Path::new(&normalized);
    if path.is_absolute() || Regex::new(r"^[A-Za-z]:/").ok()?.is_match(&normalized) {
        return Some(normalized);
    }
    context
        .cwd
        .as_ref()
        .map(|cwd| normalize_project_dir(&PathBuf::from(cwd).join(path).to_string_lossy()))
}

fn infer_project_from_absolute_path(path: &str, context: &AttributionContext) -> Option<String> {
    let normalized = normalize_project_dir(path);
    let roots: Vec<String> = context
        .known_project_roots
        .iter()
        .map(|r| normalize_project_dir(r))
        .filter(|r| !r.is_empty())
        .collect();
    roots
        .iter()
        .filter(|root| normalized == **root || normalized.starts_with(&format!("{}/", root)))
        .max_by_key(|root| root.len())
        .cloned()
        .or_else(|| {
            context
                .cwd
                .as_deref()
                .map(normalize_project_dir)
                .filter(|cwd| normalized.starts_with(cwd))
        })
}
