//! The MCP section, as yodocs read it: the servers in .mcp.json by name, each with what docs/mcp.md says about it, and
//! never a value (a URL's credentials and an argument's secret as placeholders, env and headers by name only); and the
//! tools, resources and prompts the project's own Laravel MCP server declares, read from their classes.
use std::collections::HashMap;

use gizai_docs::index::entries;
use gizai_docs::mcp::{parse, tools};
use gizai_docs::{Entry, Reference};

const MCP_JSON: &str = r#"{
  "mcpServers": {
    "sts": { "command": "php", "args": ["artisan", "mcp:start", "sts", "--api-key=abc123"], "env": { "STS_TOKEN": "s3cret", "APP_ENV": "local" } },
    "Figma": { "type": "http", "url": "https://user:pass@mcp.figma.com/mcp?token=xyz&team=42#top", "headers": { "Authorization": "Bearer abc" } },
    "broken": "not an object"
  }
}"#;

#[test]
fn the_servers_by_name_with_their_notes_and_never_a_value() {
    let notes = HashMap::from([("sts".to_string(), "<p>Our own server.</p>\n".to_string())]);
    let (servers, problems) = parse(".mcp.json", MCP_JSON, &notes);
    assert_eq!(servers.iter().map(|s| (s.name.as_str(), s.transport.as_str(), s.target.as_str())).collect::<Vec<_>>(), [
        ("Figma", "http", "https://mcp.figma.com/mcp?token=${TOKEN}&team=${TEAM}#top"),
        ("sts", "stdio", "php"),
    ]);
    let sts = &servers[1];
    assert_eq!(sts.args, ["artisan", "mcp:start", "sts", "--api-key=${VALUE}"]);
    assert_eq!((sts.env_names.as_slice(), sts.purpose.as_str()), (["APP_ENV", "STS_TOKEN"].map(String::from).as_slice(), "<p>Our own server.</p>\n"));
    assert_eq!(servers[0].header_names, ["Authorization"]);
    let json = serde_json::to_string(&servers).unwrap();
    for secret in ["abc123", "s3cret", "pass", "xyz", "Bearer"] {
        assert!(!json.contains(secret), "{secret} leaked: {json}");
    }
    assert_eq!(problems.iter().map(|p| p.message.as_str()).collect::<Vec<_>>(), [
        "the URL for \"Figma\" carried credentials in it, which have been replaced with placeholders",
        "an argument to \"sts\" carried a value for --api-key, which has been replaced with a placeholder",
    ]);
    let (none, problems) = parse(".mcp.json", "{ not json", &HashMap::new());
    assert!(none.is_empty() && problems[0].message.starts_with("is not valid JSON: "), "{problems:?}");
}

const TOOL: &str = r#"#[Title('Search blueprints')]
#[Description('Find the right starting stack for a project\'s brief.')]
#[IsReadOnly]
#[Override]
class SearchBlueprints extends FactoryTool
{
    protected string $name = 'search_blueprints';

    public function schema(JsonSchema $schema): array
    {
        return [
            'brief' => $schema->string()
                ->description('What the client asked for')
                ->required(),
            'limit' => $schema->integer()->description("At most this many"),
        ];
    }
}"#;

fn entry(name: &str, kind: &str, text: &str) -> Entry {
    Entry { name: name.into(), kind: kind.into(), syntactic: "class".into(), path: format!("app/Mcp/{name}.php"), line: 1, end_line: 20,
            doc: "The doc block.".into(), text: text.into() }
}

#[test]
fn the_tools_the_projects_own_server_declares() {
    let r = Reference { entries: vec![
        entry("FactoryTool", "mcp tool", "abstract class FactoryTool extends Tool {"),
        entry("SearchBlueprints", "class", TOOL),
        entry("GetCustomerHTMLReport", "mcp tool", "class GetCustomerHTMLReport extends Tool {}"),
        entry("Plans", "mcp resource", "class Plans extends Resource {}"),
        entry("Invoice", "model", "class Invoice extends Model {}"),
    ], ..Reference::default() };
    let got = tools(&r, &entries(&r));
    assert_eq!(got.iter().map(|t| (t.id.as_str(), t.name.as_str(), t.sort.as_str(), t.title.as_str())).collect::<Vec<_>>(), [
        ("mcp/resource/plans", "plans", "resource", "Plans"),
        ("mcp/tool/get_customer_html_report", "get_customer_html_report", "tool", "GetCustomerHTMLReport"),
        ("mcp/tool/search_blueprints", "search_blueprints", "tool", "Search blueprints"),
    ], "not the abstract base, which a subclass made a tool");
    let search = &got[2];
    assert_eq!(search.description, "Find the right starting stack for a project's brief.");
    assert_eq!(search.annotations.iter().map(|a| (a.name.as_str(), a.value.as_str())).collect::<Vec<_>>(), [("IsReadOnly", "")]);
    assert_eq!(search.inputs.iter().map(|i| (i.name.as_str(), i.kind.as_str(), i.description.as_str(), i.required)).collect::<Vec<_>>(),
               [("brief", "string", "What the client asked for", true), ("limit", "integer", "At most this many", false)]);
    assert_eq!((search.file.as_str(), search.entry.as_str()), ("app/Mcp/SearchBlueprints.php", "app/Mcp/SearchBlueprints.php#SearchBlueprints"));
    assert_eq!(got[1].description, "The doc block.", "no Description attribute: the doc block");
}
