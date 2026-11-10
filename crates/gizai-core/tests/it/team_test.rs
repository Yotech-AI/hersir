use gizai_core::{db::Db, seed::ensure_seed, team};

#[test]
fn team_has_members_states_labels_and_no_agents_on_its_columns() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Jeffrey").unwrap();
    let t = team::get(&db, &s.team_id).unwrap();
    assert_eq!(t.members.len(), 1, "only the user");
    assert_eq!(t.members[0].kind, "person");
    assert_eq!(t.states.len(), 7);
    assert_eq!(t.labels.len(), 4);
    assert!(t.states.iter().all(|s| s.agent_ids.is_empty()), "no agents on the columns until Jeffrey puts them there");
    assert_eq!(team::list(&db).unwrap().len(), 1);
}

#[test]
fn an_agent_takes_one_card_at_a_time_unless_told_more() {
    let db = gizai_core::db::Db::open_in_memory().unwrap();
    let s = gizai_core::seed::ensure_seed(&db, "Jeffrey").unwrap();
    let input = gizai_core::model::AgentInput { name: "Backend Agent".into(), role_key: "backend".into(), ..Default::default() };
    let id = gizai_core::team::add_agent(&db, &s.you_id, &s.team_id, input.clone()).unwrap();
    assert_eq!(gizai_core::team::agent(&db, &id).unwrap().max_runs, 1);
    gizai_core::team::update_agent(&db, &s.you_id, &id, gizai_core::model::AgentInput { max_runs: Some(6), ..input.clone() }).unwrap();
    assert_eq!(gizai_core::team::agent(&db, &id).unwrap().max_runs, 6);
    gizai_core::team::update_agent(&db, &s.you_id, &id, input.clone()).unwrap();
    assert_eq!(gizai_core::team::agent(&db, &id).unwrap().max_runs, 6, "unchanged when not given");
    for bad in [0, 11] {
        let e = gizai_core::team::update_agent(&db, &s.you_id, &id, gizai_core::model::AgentInput { max_runs: Some(bad), ..input.clone() }).unwrap_err();
        assert!(e.to_string().contains("1 and 10"), "{e}");
    }
}

// ---- GA-262: one rule per allowed command ----

const NOT_ONE: [&str; 6] = ["Bash(ls:*) Bash(*)", "Bash(a) b", "Bash(a,b)", "Bash(a(b))", "Bash(ls:*),Bash(*)", "Read Bash"];

#[test]
fn an_allowed_command_is_one_rule_a_tool_name_with_at_most_one_parenthesis_and_nothing_after_it() {
    for ok in ["Bash(git status:*)", "Bash(npm test:*)", "Bash(./vendor/bin/*)", "Read", "mcp__otus", "mcp__otus__get_card", "Edit",
               "Read(//srv/docs/**)", "WebFetch(domain:example.test)", " Bash(npm test:*) "] {
        assert!(team::one_rule(ok), "{ok} is one rule");
    }
    for bad in NOT_ONE.into_iter().chain(["Bash(a)b", "Bash(a))", "Bash((a)", "Bash(a", "(a)", "Bash a", "Bash:(a)", "Bäsh(a)"]) {
        assert!(!team::one_rule(bad), "{bad} isn't one rule");
    }
    assert_eq!(team::not_one_rule(" Bash(ls:*) Bash(*) "),
               "Bash(ls:*) Bash(*) isn't one rule: write each command as its own entry, like Bash(npm test:*).");
}

#[test]
fn saving_an_allowed_command_that_isnt_one_rule_is_refused_with_the_sentence_and_one_rule_entries_save() {
    use gizai_core::model::AgentInput;
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Jeffrey").unwrap();
    let input = |tools: &[&str]| AgentInput { name: "Backend Agent".into(), role_key: "backend".into(),
                                              allowed_tools: tools.iter().map(|t| t.to_string()).collect(), ..Default::default() };
    // a new agent (the agent form's Add) is refused, and nothing is made
    for bad in NOT_ONE {
        let e = team::add_agent(&db, &s.you_id, &s.team_id, input(&["Bash(git status:*)", bad])).unwrap_err().to_string();
        assert_eq!(e, team::not_one_rule(bad), "{bad}");
    }
    assert!(team::all_agents(&db).unwrap().is_empty(), "a refused agent isn't made");
    let id = team::add_agent(&db, &s.you_id, &s.team_id, input(&["Bash(git status:*)", "Bash(npm test:*)"])).unwrap();
    assert_eq!(team::agent(&db, &id).unwrap().allowed_tools, ["Bash(git status:*)", "Bash(npm test:*)"]);
    // an update (the agent form's Save) is refused and changes nothing
    for bad in NOT_ONE {
        let e = team::update_agent(&db, &s.you_id, &id, input(&[bad, "Bash(npm test:*)"])).unwrap_err().to_string();
        assert!(e.contains(&format!("{bad} isn't one rule: write each command as its own entry, like Bash(npm test:*).")), "{e}");
    }
    assert_eq!(team::agent(&db, &id).unwrap().allowed_tools, ["Bash(git status:*)", "Bash(npm test:*)"], "nothing changed");
    // every entry of each role's default list is one rule, so every role's list still saves
    for role in ["lead", "backend", "frontend", "qa", "devops", "design"] {
        let list = gizai_core::seed::role_tools(role);
        assert!(list.iter().all(|t| team::one_rule(t)), "{role}: {list:?}");
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        team::update_agent(&db, &s.you_id, &id, input(&refs)).unwrap_or_else(|e| panic!("{role}'s list: {e}"));
    }
}
