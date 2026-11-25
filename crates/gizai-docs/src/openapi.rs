//! The docs page's API section from an OpenAPI document the project already has (yodocs' src/content/openapi.ts):
//! each operation's method, path, summary, tags, the security it needs, its parameters, request body and responses,
//! and the reusable schemas, so the page can open a `$ref` without asking again. JSON only; a YAML spec is named as a
//! problem. Without a spec, the routes Hersir reads fill the section (`index::routes_api`).
use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::index::{Api, ApiResponse, Endpoint, Parameter, Problem};

/// Where a project keeps its OpenAPI document, best first (yodocs' SPECS).
const SPECS: [&str; 5] = ["openapi.json", "openapi.yaml", "openapi.yml", "docs/openapi.json", "public/openapi.json"];
const METHODS: [&str; 8] = ["get", "post", "put", "patch", "delete", "head", "options", "trace"];
/// A spec bigger than this is left out, and named as a problem.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// The project's OpenAPI document, if git tracks a JSON one, and what kept it from being read.
pub fn read(repo: &Path, files: &[String]) -> (Option<Api>, Vec<Problem>) {
    let tracked: Vec<&str> = SPECS.iter().copied().filter(|s| files.iter().any(|f| f == s)).collect();
    let Some(file) = tracked.iter().find(|s| s.ends_with(".json")) else {
        let problems = tracked.iter().map(|yaml| Problem { file: yaml.to_string(), line: None,
            message: "is a YAML spec, and Hersir reads JSON only for now. Convert it to JSON to see its endpoints.".into() }).collect();
        return (None, problems);
    };
    let too_big = std::fs::symlink_metadata(repo.join(file)).is_ok_and(|m| m.len() > MAX_BYTES);
    if too_big {
        return (None, vec![Problem { file: file.to_string(), line: None, message: "is over 8 MB, so it was left out".into() }]);
    }
    match crate::read_text(repo, file, MAX_BYTES as usize) {
        Some(text) => parse(file, &text),
        None => (None, vec![]),
    }
}

/// An OpenAPI (or Swagger) document as the page's API: its endpoints by path, then method.
pub fn parse(file: &str, text: &str) -> (Option<Api>, Vec<Problem>) {
    let problem = |message: String| (None, vec![Problem { file: file.into(), line: None, message }]);
    let doc: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return problem(format!("is not valid JSON: {e}")),
    };
    if doc.get("openapi").and_then(Value::as_str).unwrap_or_default().is_empty() && doc.get("swagger").is_none() {
        return problem("has no \"openapi\" version field, so it was skipped".into());
    }
    let text_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut endpoints = vec![];
    for (path, item) in doc.get("paths").and_then(Value::as_object).into_iter().flatten() {
        for method in METHODS {
            let Some(op) = item.get(method).filter(|o| o.is_object()) else { continue };
            let list = |k: &str| op.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
            endpoints.push(Endpoint {
                id: format!("api/{method}{path}"),
                method: method.to_uppercase(),
                path: path.clone(),
                summary: text_of(op, "summary"),
                description: text_of(op, "description"),
                tags: list("tags").iter().map(|t| t.as_str().map_or_else(|| t.to_string(), str::to_string)).collect(),
                auth: list("security").iter().filter_map(Value::as_object).flat_map(|s| s.keys().cloned()).collect(),
                parameters: list("parameters").iter().map(|p| Parameter {
                    name: text_of(p, "name"),
                    location: text_of(p, "in"),
                    required: p.get("required") == Some(&Value::Bool(true)),
                    description: text_of(p, "description"),
                }).collect(),
                request_body: op.get("requestBody").cloned().unwrap_or(Value::Null),
                responses: op.get("responses").and_then(Value::as_object).into_iter().flatten().map(|(status, r)| ApiResponse {
                    status: status.clone(),
                    description: text_of(r, "description"),
                    schema: r.get("content").and_then(Value::as_object).and_then(|c| c.values().next())
                        .and_then(|first| first.get("schema")).cloned().unwrap_or(Value::Null),
                }).collect(),
            });
        }
    }
    endpoints.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.method.cmp(&b.method)));
    let info = doc.get("info").cloned().unwrap_or(Value::Null);
    let schemas: BTreeMap<String, Value> = doc.pointer("/components/schemas").and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    let title = Some(text_of(&info, "title")).filter(|t| !t.is_empty()).unwrap_or_else(|| "API".into());
    (Some(Api { title, version: text_of(&info, "version"), file: file.into(), endpoints, schemas }), vec![])
}
