// GA-271: After the Chief uses it, Ask for my OK (the default) or Allow, per MCP server and the browser (`trusted` on its
// switch) and for Web (`web_trusted`, JSON `webTrusted`). Saved only through the agent form (`set_agent_tools`,
// `set_cli_tools`), next to the other switches; data saved before has neither field, so it reads as Ask everywhere.
use gizai_core::db::Db;
use gizai_core::mcp_servers::{self as m, AgentServer, AgentTools, CliTools, McpServer};
use gizai_core::model::AgentInput;
use gizai_core::{seed, team};

/// The Chief (Chat on), its Otus server and you: (db, you, chief, otus's id).
fn setup() -> (Db, String, String, String) {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let id = team::add_agent(&db, &s.you_id, &s.team_id, AgentInput { name: "Chief".into(), role_key: "lead".into(), chat_enabled: Some(true),
        ..Default::default() }).unwrap();
    let otus = m::save(&db, McpServer { name: "otus".into(), transport: "stdio".into(), command: "/opt/otus/otus-mcp".into(), ..Default::default() }).unwrap();
    (db, s.you_id, id, otus.id)
}

fn raw_extra(db: &Db, id: &str, json: &str) {
    db.write(None, |w| {
        w.conn().execute("UPDATE agent_configs SET mcp_extra_json=?2 WHERE actor_id=?1", rusqlite::params![id, json])?;
        Ok(())
    }).unwrap();
}

fn extra(db: &Db, id: &str) -> serde_json::Value {
    let raw: String = db.read(|c| Ok(c.query_row("SELECT mcp_extra_json FROM agent_configs WHERE actor_id=?1", [id], |r| r.get(0))?)).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn server(id: &str, on: bool, trusted: bool) -> AgentServer {
    AgentServer { server_id: id.into(), on, tools_off: vec![], trusted }
}

#[test]
fn a_new_agent_and_one_saved_before_this_choice_read_as_ask_for_every_source() {
    let (db, _, id, otus) = setup();
    assert!(!CliTools::default().web_trusted && !AgentServer::default().trusted, "Ask for my OK is the default");
    let m0 = team::agent(&db, &id).unwrap();
    assert!(m0.tools.mcp.is_empty() && !m0.cli_tools.web_trusted);
    assert_eq!(m0.tools.trusted_on().count(), 0);

    // saved before GA-271: neither `trusted` nor `webTrusted` in the JSON, Otus, the browser and Web all on
    let before = format!(r#"{{"mcp":[{{"serverId":"{otus}","on":true,"toolsOff":["delete_note"]}},{{"serverId":"chrome-devtools","on":true,"toolsOff":[]}}],
        "cli":{{"webSearch":true,"webFetch":true,"fetchDomains":["docs.rs"],"insecureCerts":false,"builtin":[],"slashCommands":false}},"screenshots":false}}"#);
    raw_extra(&db, &id, &before);
    let a = team::agent(&db, &id).unwrap();
    assert_eq!(a.tools.mcp, [AgentServer { server_id: otus.clone(), on: true, tools_off: vec!["delete_note".into()], trusted: false },
                             server(m::BROWSER, true, false)], "what was on stays on, on Ask");
    assert_eq!(a.tools.trusted_on().count(), 0);
    assert!(a.cli_tools.web_search && a.cli_tools.web_fetch && !a.cli_tools.web_trusted, "{:?}", a.cli_tools);
    assert!(!m::parse_cli_tools(Some(&before)).web_trusted);
    assert!(m::parse_tools(Some(&before)).mcp.iter().all(|s| !s.trusted));
}

#[test]
fn allow_saves_per_server_the_browser_and_web_reads_back_and_ask_again_removes_it() {
    let (db, you, id, otus) = setup();
    let linear = m::save(&db, McpServer { name: "linear".into(), transport: "http".into(), url: "https://mcp.linear.example/mcp".into(), ..Default::default() })
        .unwrap().id;
    let saved = m::set_agent_tools(&db, &you, &id, AgentTools { mcp: vec![server(&otus, true, true), server(m::BROWSER, true, true), server(&linear, true, false)] })
        .unwrap();
    assert_eq!(saved.mcp.iter().map(|s| (s.server_id.as_str(), s.trusted)).collect::<Vec<_>>(),
               [(otus.as_str(), true), (m::BROWSER, true), (linear.as_str(), false)]);
    let web = m::set_cli_tools(&db, &you, &id, CliTools { web_search: true, web_trusted: true, ..Default::default() }).unwrap();
    assert!(web.web_trusted);

    // read back as the app reads it (team::agent, after a restart too), with the JSON names the UI uses
    let a = team::agent(&db, &id).unwrap();
    assert_eq!(a.tools.trusted_on().collect::<Vec<_>>(), [otus.as_str(), m::BROWSER]);
    assert!(a.cli_tools.web_trusted && a.cli_tools.web_search);
    let x = extra(&db, &id);
    assert_eq!(x["mcp"][0]["trusted"], true, "{x}");
    assert_eq!(x["mcp"][2]["trusted"], false, "{x}");
    assert_eq!(x["cli"]["webTrusted"], true, "{x}");
    assert!(x["cli"].get("web_trusted").is_none(), "{x}");

    // each switch leaves the other alone: saving the servers again keeps Web's choice, and the other way round
    m::set_agent_tools(&db, &you, &id, AgentTools { mcp: vec![server(&otus, true, true)] }).unwrap();
    assert!(team::agent(&db, &id).unwrap().cli_tools.web_trusted);
    m::set_cli_tools(&db, &you, &id, CliTools { web_search: true, ..Default::default() }).unwrap();
    let a = team::agent(&db, &id).unwrap();
    assert!(!a.cli_tools.web_trusted, "Web is on Ask again");
    assert_eq!(a.tools.trusted_on().collect::<Vec<_>>(), [otus.as_str()], "Otus stays on Allow");

    // Ask again for Otus: nothing is on Allow
    m::set_agent_tools(&db, &you, &id, AgentTools { mcp: vec![server(&otus, true, false)] }).unwrap();
    assert_eq!(team::agent(&db, &id).unwrap().tools.trusted_on().count(), 0);
}

#[test]
fn a_server_on_allow_that_is_switched_off_keeps_its_choice_but_trusts_nothing_until_it_is_on() {
    let (db, you, id, otus) = setup();
    m::set_agent_tools(&db, &you, &id, AgentTools { mcp: vec![server(&otus, false, true)] }).unwrap();
    let a = team::agent(&db, &id).unwrap();
    assert_eq!(a.tools.mcp, [server(&otus, false, true)], "the choice is kept");
    assert_eq!(a.tools.trusted_on().count(), 0, "an off server is in no run");
    m::set_agent_tools(&db, &you, &id, AgentTools { mcp: vec![server(&otus, true, true)] }).unwrap();
    assert_eq!(team::agent(&db, &id).unwrap().tools.trusted_on().collect::<Vec<_>>(), [otus.as_str()]);
}
