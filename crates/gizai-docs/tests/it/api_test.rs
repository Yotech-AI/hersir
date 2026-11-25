//! Docs → API: the routes read from a project's files. Next.js's app router (route groups, parallel slots and private
//! folders; the three ways a route file exports a method) and pages router; Laravel's routes files (verbs, closures,
//! controller actions, prefix and controller groups, match, view, resources with only, api.php under /api; console.php
//! left out).
use std::path::Path;

use gizai_docs::api::api;

fn write(dir: &Path, path: &str, text: &str) -> String {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
    path.to_string()
}

/// (method, path, handler) of each route.
fn routes(dir: &Path, files: &[String]) -> Vec<(String, String, String)> {
    api(dir, files).into_iter().map(|r| (r.method, r.path, r.handler)).collect()
}

fn r(method: &str, path: &str, handler: &str) -> (String, String, String) {
    (method.into(), path.into(), handler.into())
}

#[test]
fn next_js_app_router_pages_and_route_files_with_their_methods() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "src/app/page.tsx", "export default function Home() { return null; }\n"),
        write(d, "src/app/(marketing)/about/page.tsx", "export default function About() {}\n"),
        write(d, "src/app/_lib/page.tsx", "export default function Hidden() {}\n"),
        write(d, "src/app/@modal/login/page.tsx", "export default function Login() {}\n"),
        write(d, "src/app/api/users/[id]/route.ts", "export async function GET() {}\n\nexport const DELETE = withAuth(handler);\n"),
        write(d, "src/app/api/auth/[...all]/route.ts", "export const { POST, GET: handle } = toNextJsHandler(auth.handler);\n"),
        write(d, "src/app/layout.tsx", "export default function Layout() {}\n"),
    ];
    assert_eq!(routes(d, &files), [
        r("PAGE", "/", ""), r("PAGE", "/about", ""), r("PAGE", "/login", ""),
        r("GET", "/api/users/[id]", ""), r("DELETE", "/api/users/[id]", ""), r("GET", "/api/auth/[...all]", ""), r("POST", "/api/auth/[...all]", ""),
    ]);
    let delete = api(d, &files).into_iter().find(|x| x.method == "DELETE").unwrap();
    assert_eq!((delete.file.as_str(), delete.line), ("src/app/api/users/[id]/route.ts", 3));
}

#[test]
fn next_js_pages_router_pages_and_api_files() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "pages/index.tsx", "export default function Home() {}\n"),
        write(d, "pages/blog/[slug].tsx", "export default function Post() {}\n"),
        write(d, "pages/blog/index.tsx", "export default function Blog() {}\n"),
        write(d, "pages/_app.tsx", "export default function App() {}\n"),
        write(d, "pages/api/hello.ts", "export default function handler() {}\n"),
    ];
    assert_eq!(routes(d, &files), [r("PAGE", "/", ""), r("PAGE", "/blog/[slug]", ""), r("PAGE", "/blog", ""), r("ANY", "/api/hello", "")]);
}

const WEB: &str = r#"<?php

use App\Http\Controllers\InvoiceController;
use Illuminate\Support\Facades\Route;

// The welcome page
Route::get('/', function () {
    return view('welcome', ['x' => '{not a route}']);
});

Route::get('/invoices/{invoice}', [InvoiceController::class, 'show'])->name('invoices.show');

Route::prefix('admin')->middleware('auth')->group(function () {
    Route::resource('photos', PhotoController::class)->only(['index', 'show']);
    Route::post('/sync', 'SyncController@run');
    Route::group(['prefix' => 'reports'], function () {
        Route::get('/monthly', MonthlyReport::class);
    });
});

Route::match(['get', 'post'], '/contact', ContactController::class);
Route::view('/terms', 'pages.terms');
"#;

const API: &str = r#"<?php

use Illuminate\Support\Facades\Route;

Route::apiResource('categories', CategoryController::class);

Route::controller(OrderController::class)->group(function () {
    Route::get('/orders', 'index');
});
"#;

#[test]
fn laravel_routes_files_with_their_groups_resources_and_handlers() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "routes/web.php", WEB),
        write(d, "routes/api.php", API),
        write(d, "routes/console.php", "<?php\nArtisan::command('inspire', function () {});\nRoute::get('/never', fn () => 1);\n"),
    ];
    assert_eq!(routes(d, &files), [
        r("GET", "/", "closure"),
        r("GET", "/invoices/{invoice}", "InvoiceController@show"),
        r("GET", "/admin/photos", "PhotoController@index"),
        r("GET", "/admin/photos/{photo}", "PhotoController@show"),
        r("POST", "/admin/sync", "SyncController@run"),
        r("GET", "/admin/reports/monthly", "MonthlyReport"),
        r("GET", "/contact", "ContactController"),
        r("POST", "/contact", "ContactController"),
        r("GET", "/terms", "view pages.terms"),
        r("GET", "/api/categories", "CategoryController@index"),
        r("POST", "/api/categories", "CategoryController@store"),
        r("GET", "/api/categories/{category}", "CategoryController@show"),
        r("PUT", "/api/categories/{category}", "CategoryController@update"),
        r("DELETE", "/api/categories/{category}", "CategoryController@destroy"),
        r("GET", "/api/orders", "OrderController@index"),
    ]);
    let first = api(d, &files).remove(0);
    assert_eq!((first.file.as_str(), first.line), ("routes/web.php", 7));
}
