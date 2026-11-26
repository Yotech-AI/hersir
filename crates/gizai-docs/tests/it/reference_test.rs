//! Docs → Reference: the declarations read from a project's files. TSX (components, hooks, functions, types, constants,
//! classes; members and imports left out), Vue (the file's component and its script's declarations, at the file's own
//! lines), PHP (Laravel's models, controllers, migrations, middleware and exceptions), the comment above a declaration,
//! the folders and files left out, and a file the grammar can't read in full.
use std::path::Path;

use gizai_docs::{reference, Entry, MAX_BYTES};

fn write(dir: &Path, path: &str, text: &str) -> String {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
    path.to_string()
}

/// (name, kind, line) of each entry of `path`.
fn of<'a>(entries: &'a [Entry], path: &str) -> Vec<(&'a str, &'a str, usize)> {
    entries.iter().filter(|e| e.path == path).map(|e| (e.name.as_str(), e.kind.as_str(), e.line)).collect()
}

const TSX: &str = r#"import { useState } from "react";

/** The page's header, with the site's name. */
export function Header({ title }: { title: string }) {
  const [open, setOpen] = useState(false);
  const toggle = () => setOpen(!open);
  return <header onClick={toggle}>{title}</header>;
}

// Whether the menu is open, kept in the URL.
export function useMenu() {
  return useState(false);
}

export const ROUTES = ["/", "/about"];

export interface Props { title: string }
export type Theme = "dark" | "light";
export enum Size { Small, Large }

export class Store {
  count = 0;
  add() { this.count += 1; }
}

export function slug(s: string) { return s.toLowerCase(); }
"#;

#[test]
fn tsx_gives_components_hooks_functions_types_and_constants_without_members_or_imports() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "src/Header.tsx", TSX);
    let r = reference(dir.path(), std::slice::from_ref(&file));
    assert_eq!(r.files, 1);
    assert!(r.partial.is_empty(), "{:?}", r.partial);
    assert_eq!(of(&r.entries, &file), [
        ("Header", "component", 4), ("toggle", "function", 6), ("useMenu", "hook", 11), ("ROUTES", "constant", 15),
        ("Props", "interface", 17), ("Theme", "type", 18), ("Size", "enum", 19), ("Store", "class", 21), ("slug", "function", 26),
    ]);
    let header = &r.entries[0];
    assert_eq!(header.doc, "The page's header, with the site's name.");
    assert_eq!(header.end_line, 8);
    assert!(header.text.starts_with("export function Header") || header.text.starts_with("function Header"), "{}", header.text);
    assert_eq!(r.entries[2].doc, "Whether the menu is open, kept in the URL.");
}

#[test]
fn a_vue_file_is_a_component_named_after_it_with_its_scripts_declarations_at_the_files_lines() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "resources/js/UserCard.vue",
        "<template>\n  <div>{{ name }}</div>\n</template>\n\n<script setup lang=\"ts\">\nimport { computed } from 'vue';\n\nfunction initials(name: string) {\n  return name.slice(0, 2);\n}\n</script>\n");
    let r = reference(dir.path(), std::slice::from_ref(&file));
    assert_eq!(of(&r.entries, &file), [("UserCard", "component", 1), ("initials", "function", 8)]);
}

const PHP: &str = r#"<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

/**
 * An invoice, sent to a client once a month.
 */
class Invoice extends Model
{
    protected $fillable = ['total'];
}

class InvoiceController extends Controller
{
    public function index() { return []; }
}

class CreateInvoicesTable extends Migration {}

class EnsureClient
{
    public function handle($request, Closure $next) { return $next($request); }
}

class PaymentFailed extends \RuntimeException {}

interface Billable {}
"#;

#[test]
fn php_gives_laravels_kinds_by_what_a_class_extends_and_its_doc_block() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "app/Models/Invoice.php", PHP);
    let r = reference(dir.path(), std::slice::from_ref(&file));
    assert_eq!(of(&r.entries, &file), [
        ("Invoice", "model", 10), ("InvoiceController", "controller", 15), ("CreateInvoicesTable", "migration", 20),
        ("EnsureClient", "middleware", 22), ("PaymentFailed", "exception", 27), ("Billable", "interface", 29),
    ]);
    assert_eq!(r.entries[0].doc, "An invoice, sent to a client once a month.");
}

#[test]
fn installed_built_minified_declaration_big_and_unreadable_files_are_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "src/keep.ts", "export function keep() {}\n"),
        write(d, "vendor/laravel/x.php", "<?php class X {}\n"),
        write(d, "node_modules/x/index.js", "function x() {}\n"),
        write(d, "dist/app.js", "function built() {}\n"),
        write(d, "public/js/app.js", "function bundled() {}\n"),
        write(d, "src/types.d.ts", "export interface Declared {}\n"),
        write(d, "src/lib.min.js", "function m(){}\n"),
        write(d, "src/big.ts", &format!("export function big() {{}}\n{}", "// padding\n".repeat((MAX_BYTES as usize) / 10))),
        write(d, "README.md", "# Not code\n"),
        "src/gone.ts".to_string(),
    ];
    std::fs::write(d.join("src/latin1.ts"), b"export const caf\xe9 = 1;\n").unwrap();
    let mut all = files.clone();
    all.push("src/latin1.ts".into());
    let r = reference(d, &all);
    assert_eq!(r.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["keep"]);
    assert_eq!(r.files, 1);
}

#[test]
fn a_file_the_grammar_cant_read_in_full_still_gives_its_declarations_and_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "src/Broken.tsx", "export function Fine() { return 1; }\n\nexport function Broken( {\n");
    let r = reference(dir.path(), std::slice::from_ref(&file));
    assert!(of(&r.entries, &file).iter().any(|(n, ..)| *n == "Fine"), "{:?}", r.entries);
    assert_eq!(r.partial, [file]);
}

const DART: &str = r#"import 'package:flutter/material.dart';

/// The app's first screen.
class HomePage extends StatelessWidget {
  const HomePage({super.key});
  @override
  Widget build(BuildContext context) => const Text('Home');
}

class _CounterState extends State<Counter> {
  int count = 0;
}

mixin Logging {}

extension Pretty on String {}

enum Status { draft, sent }

typedef Json = Map<String, dynamic>;

const appName = 'Ekpoma Watch';

void main() {
  runApp(const HomePage());
}
"#;

#[test]
fn dart_gives_widgets_classes_mixins_extensions_enums_types_constants_and_functions() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "lib/main.dart", DART);
    let r = reference(dir.path(), std::slice::from_ref(&file));
    assert!(r.partial.is_empty(), "{:?}", r.partial);
    assert_eq!(of(&r.entries, &file), [
        ("HomePage", "widget", 4), ("_CounterState", "widget", 10), ("Logging", "mixin", 14), ("Pretty", "extension", 16),
        ("Status", "enum", 18), ("Json", "type", 20), ("appName", "constant", 22), ("main", "function", 24),
    ]);
    assert_eq!(r.entries[0].doc, "The app's first screen.");
}

const KOTLIN: &str = r#"package app.lawschool

/** A case the student is reading. */
data class Case(val name: String)

interface Repository {
    fun all(): List<Case>
}

enum class Court { HIGH, APPEAL }

object Cache {
    val size = 10
}

typealias Cases = List<Case>

val VERSION = "1.0"

fun main() {
    println(VERSION)
}
"#;

#[test]
fn kotlin_gives_classes_interfaces_enums_objects_types_constants_and_functions() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "app/src/main/java/App.kt", KOTLIN);
    let r = reference(dir.path(), std::slice::from_ref(&file));
    assert!(r.partial.is_empty(), "{:?}", r.partial);
    assert_eq!(of(&r.entries, &file), [
        ("Case", "class", 4), ("Repository", "interface", 6), ("Court", "enum", 10), ("Cache", "object", 12),
        ("Cases", "type", 16), ("VERSION", "constant", 18), ("main", "function", 20),
    ]);
    assert_eq!(r.entries[0].doc, "A case the student is reading.");
}

#[test]
fn php_gives_laravels_mcp_tools_resources_prompts_and_servers_by_what_they_import_and_extend() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "app/Mcp/Tools/SearchBlueprints.php", "<?php\nnamespace App\\Mcp\\Tools;\n\nuse Laravel\\Mcp\\Server\\Tool;\n\nclass SearchBlueprints extends Tool\n{\n}\n"),
        write(d, "app/Mcp/Resources/Plans.php", "<?php\nuse Laravel\\Mcp\\Server\\Resource;\nclass Plans extends Resource {}\n"),
        write(d, "app/Mcp/Prompts/Brief.php", "<?php\nuse Laravel\\Mcp\\Server\\Prompt;\nclass Brief extends Prompt {}\n"),
        write(d, "app/Mcp/Servers/Factory.php", "<?php\nuse Laravel\\Mcp\\Server;\nclass Factory extends Server {}\n"),
        write(d, "app/Support/Tool.php", "<?php\nclass Hammer extends Tool {}\n"),
    ];
    let r = reference(d, &files);
    let kinds: Vec<(&str, &str, &str)> = r.entries.iter().map(|e| (e.name.as_str(), e.kind.as_str(), e.syntactic.as_str())).collect();
    assert_eq!(kinds, [("SearchBlueprints", "mcp tool", "class"), ("Plans", "mcp resource", "class"), ("Brief", "mcp prompt", "class"),
        ("Factory", "mcp server", "class"), ("Hammer", "class", "class")]);
}
