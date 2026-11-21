//! The API section from the project's own OpenAPI document, as yodocs read it: each operation by path then method,
//! with its summary, tags, security, parameters, request body, responses and the reusable schemas; JSON only (a YAML
//! spec named as a problem); a document without an openapi version, or not JSON, named as a problem.
use std::path::Path;

use gizai_docs::openapi::{parse, read};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "STS", "version": "1.4" },
  "paths": {
    "/api/v1/leads": {
      "post": {
        "summary": "Create a lead", "description": "Called by the web server.", "tags": ["Leads"],
        "security": [{ "bearerAuth": [] }],
        "requestBody": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Lead" } } } },
        "responses": { "201": { "description": "Lead created", "content": { "application/json": { "schema": { "type": "object" } } } },
                       "401": { "description": "Missing or wrong token" } }
      },
      "get": { "summary": "List leads", "parameters": [{ "name": "page", "in": "query", "required": false, "description": "From 1" }] }
    },
    "/api/v1/domains/{domain}": { "delete": { "summary": "Take off sale", "parameters": [{ "name": "domain", "in": "path", "required": true }] } }
  },
  "components": { "schemas": { "Lead": { "type": "object", "required": ["domain"] } } }
}"##;

#[test]
fn each_operation_by_path_then_method_with_what_it_takes_and_gives() {
    let (api, problems) = parse("openapi.json", SPEC);
    assert!(problems.is_empty());
    let api = api.unwrap();
    assert_eq!((api.title.as_str(), api.version.as_str(), api.file.as_str()), ("STS", "1.4", "openapi.json"));
    assert_eq!(api.endpoints.iter().map(|e| (e.id.as_str(), e.method.as_str(), e.summary.as_str())).collect::<Vec<_>>(), [
        ("api/delete/api/v1/domains/{domain}", "DELETE", "Take off sale"),
        ("api/get/api/v1/leads", "GET", "List leads"),
        ("api/post/api/v1/leads", "POST", "Create a lead"),
    ]);
    let create = &api.endpoints[2];
    assert_eq!((create.tags.as_slice(), create.auth.as_slice()), (["Leads".to_string()].as_slice(), ["bearerAuth".to_string()].as_slice()));
    assert_eq!(create.request_body["content"]["application/json"]["schema"]["$ref"], "#/components/schemas/Lead");
    assert_eq!(create.responses.iter().map(|r| (r.status.as_str(), r.description.as_str())).collect::<Vec<_>>(),
               [("201", "Lead created"), ("401", "Missing or wrong token")]);
    assert_eq!((create.responses[0].schema["type"].as_str(), create.responses[1].schema.is_null()), (Some("object"), true));
    let page = &api.endpoints[1].parameters[0];
    assert_eq!((page.name.as_str(), page.location.as_str(), page.required, page.description.as_str()), ("page", "query", false, "From 1"));
    assert_eq!(api.schemas["Lead"]["required"][0], "domain");
}

#[test]
fn what_isnt_a_json_openapi_document_is_named() {
    assert_eq!(parse("openapi.json", "{ nope").1[0].message.split(':').next(), Some("is not valid JSON"));
    assert_eq!(parse("openapi.json", r#"{ "paths": {} }"#).1[0].message, "has no \"openapi\" version field, so it was skipped");
    assert!(parse("openapi.json", r#"{ "swagger": "2.0", "paths": {} }"#).0.is_some(), "Swagger 2 is read too");

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("openapi.yaml"), "openapi: 3.1.0\n").unwrap();
    let (api, problems) = read(dir.path(), &["openapi.yaml".to_string()]);
    assert!(api.is_none());
    assert_eq!((problems[0].file.as_str(), problems[0].message.as_str()),
               ("openapi.yaml", "is a YAML spec, and Hersir reads JSON only for now. Convert it to JSON to see its endpoints."));
    std::fs::write(dir.path().join("openapi.json"), SPEC).unwrap();
    assert!(read(dir.path(), &["openapi.json".to_string()]).0.is_some());
    assert!(read(Path::new("/nonexistent"), &[]).0.is_none(), "only a file git tracks");
}
