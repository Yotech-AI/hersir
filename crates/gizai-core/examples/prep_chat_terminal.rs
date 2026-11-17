//! Adds a chat for the UI test of Open in terminal (GA-279) to data prep_chats made: "How do I test GA-279?", the newest
//! chat, where the Chief's answers ask you to run commands (or don't): the card's Why ("Before you test, in
//! `~/Herd/doq-studio`: Run … Run …"), a tool call, a message that only mentions a command and a file, a ```sh block with $
//! prompts in a subfolder with a URL after it, the folder's absolute path, commands without a folder, two folders, four
//! folders and plain text, each its own answer (GA-298: one row of buttons under a whole answer, so a question of yours
//! comes between them). Last, GA-298's answer: chips of four cards, a move_task call, a file and line in doq-studio
//! (page.html:30 at its top, a file of 40 lines written here: a file in a folder there would be "Ignored by git", as the
//! UI test's scratch home is in .devdata) and a command. Prints the chat's id.
//! usage: cargo run -p gizai-core --example prep_chat_terminal -- <data dir> <home folder the app gets>
use gizai_core::{chat, db::Db, seed::ensure_seed, team};
use serde_json::json;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (dir, home) = (&a[1], a[2].trim_end_matches('/'));
    let db = Db::open(&std::path::Path::new(dir).join("gizai.db")).unwrap();
    let you = ensure_seed(&db, "Jeffrey").unwrap().you_id;
    let lead = team::chat_agent(&db).unwrap().expect("prep_chats' Chief").actor_id;
    let th = chat::create_thread(&db, &you, &lead, "How do I test GA-279?").unwrap();
    let say = |role: &str, body: &str| {
        let author = if role == "user" { you.clone() } else { lead.clone() };
        let tool = (role == "tool").then(|| json!({"id": "toolu_1", "input": {"query": body}, "result": "{}", "isError": false}));
        chat::add_message(&db, chat::NewMessage { thread_id: th.clone(), role: role.into(), author_id: Some(author), body_md: Some(body.into()),
                                                  tool_name: tool.as_ref().map(|_| "mcp__gizai__list_tasks".to_string()), tool, ..Default::default() })
            .unwrap();
    };
    say("user", "How do I test GA-279?");
    say("agent", "Before you test, in `~/Herd/doq-studio`:\n\n1. Run `php artisan migrate`.\n2. Run `npm run build`.");
    say("tool", "Run `npm test` in ~/Herd/doq-studio");
    say("agent", "QA ran `npm test` on GA-279, and the change is in `src/api.ts`.");
    say("user", "And the embed test?");
    say("agent", "In `~/Herd/doq-studio/resources/embed-test`:\n\n```sh\n$ php -S 127.0.0.1:8081\n```\n\nThen open http://127.0.0.1:8081/page.html.");
    say("user", "And from its full path?");
    say("agent", &format!("In `{home}/Herd/doq-studio`: run `npm test`."));
    say("user", "And composer?");
    say("agent", "Run `composer install` in your checkout.");
    say("user", "And the shop?");
    say("agent", "In `~/Herd/shop`: run `npm run build`. Then in `~/Herd/doq-studio`: run `php artisan migrate`.");
    say("user", "And all of them?");
    say("agent", "Run `npm ci` in ~/Herd/doq-studio, ~/Herd/shop, ~/Herd/kade-portal and ~/Herd/invoice-api.");
    say("user", "Is it done?");
    say("agent", "All done: GA-279 is in Review.");
    // GA-298: an answer that links four cards, moved one (a move_task of Hersir's that ended without an error), names a file
    // and line in doq-studio and asks you to run a command
    let page = format!("{home}/Herd/doq-studio/page.html");
    std::fs::write(&page, (1..=40).map(|n| format!("line {n}\n")).collect::<String>()).unwrap();
    say("user", "Move KADE-1 along?");
    chat::add_message(&db, chat::NewMessage { thread_id: th.clone(), role: "tool".into(), author_id: Some(lead.clone()), body_md: Some("Moved KADE-1".into()),
                                              tool_name: Some("mcp__gizai__move_task".into()),
                                              tool: Some(json!({"id": "toolu_2", "input": {"identifier": "KADE-1", "state": "Testing"}, "result": "{}", "isError": false})),
                                              ..Default::default() }).unwrap();
    say("agent", "I moved [KADE-1 - Export invoices](gizai:task/KADE-1) to Testing; [KADE-2](gizai:task/KADE-2), [KADE-3](gizai:task/KADE-3) and [GFW-1](gizai:task/GFW-1) are next. The page is `page.html:30`. In `~/Herd/doq-studio`: run `npm test`.");
    println!("{th}");
}
