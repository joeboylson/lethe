//! `lethe serve`: the page and the small API behind it, on 127.0.0.1 only.

use crate::{db, notes, paths};
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};

/// The built page (web/dist). `./install.sh` builds it before the binary.
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Page;

type ApiResult = Result<Json<Value>, (StatusCode, String)>;

fn fail(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// Each request opens the database on a blocking thread: it's a small file
/// on this laptop, and the hooks write to it from other processes.
async fn with_db<T: Send + 'static>(
    f: impl FnOnce(&rusqlite::Connection) -> anyhow::Result<T> + Send + 'static,
) -> Result<T, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || f(&db::open()?)).await.map_err(fail)?.map_err(fail)
}

async fn health() -> Json<Value> {
    Json(json!({ "service": "lethe", "version": env!("CARGO_PKG_VERSION") }))
}

async fn projects() -> ApiResult {
    let rows = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT project, project_path, COUNT(*), MAX(created_at), SUM(status = 'waiting')
             FROM notes WHERE archived = 0 GROUP BY project_path ORDER BY MAX(created_at) DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(json!({
                "name": r.get::<_, String>(0)?,
                "path": r.get::<_, String>(1)?,
                "count": r.get::<_, i64>(2)?,
                "latest": r.get::<_, String>(3)?,
                "waiting": r.get::<_, i64>(4)?,
            }))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await?;
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct ListQuery {
    project: Option<String>,
    #[serde(default)]
    archived: bool,
}

async fn list(Query(q): Query<ListQuery>) -> ApiResult {
    let notes = with_db(move |conn| db::notes(conn, q.project.as_deref(), q.archived, 500)).await?;
    Ok(Json(json!(notes)))
}

async fn one(Path(id): Path<i64>) -> ApiResult {
    let found = with_db(move |conn| {
        let Some(note) = db::note(conn, id)? else { return Ok(None) };
        let body = std::fs::read_to_string(&note.path).unwrap_or_else(|e| format!("_Couldn't read {}: {e}_", note.path));
        // The writer's window: what it replaced and what replaced it.
        let session = |id: &Option<String>| -> anyhow::Result<Value> {
            let Some(id) = id else { return Ok(Value::Null) };
            Ok(conn
                .query_row(
                    "SELECT id, cwd, source, started_at, ended_at, end_reason, replaced_by FROM sessions WHERE id = ?1",
                    [id],
                    |r| {
                        Ok(json!({
                            "id": r.get::<_, String>(0)?, "cwd": r.get::<_, Option<String>>(1)?,
                            "source": r.get::<_, Option<String>>(2)?, "started_at": r.get::<_, String>(3)?,
                            "ended_at": r.get::<_, Option<String>>(4)?, "end_reason": r.get::<_, Option<String>>(5)?,
                            "replaced_by": r.get::<_, Option<String>>(6)?,
                        }))
                    },
                )
                .unwrap_or_else(|_| json!({ "id": id })))
        };
        let writer = session(&note.session_id)?;
        let reader = session(&note.picked_up_by)?;
        Ok(Some(json!({ "note": note, "body": body, "writer": writer, "reader": reader })))
    })
    .await?;
    found.map(Json).ok_or((StatusCode::NOT_FOUND, format!("no note {id}")))
}

#[derive(Deserialize)]
struct ArchiveBody {
    archived: bool,
}

async fn archive(Path(id): Path<i64>, Json(b): Json<ArchiveBody>) -> ApiResult {
    let ok = with_db(move |conn| notes::set_archived(conn, id, b.archived)).await?;
    if !ok {
        return Err((StatusCode::NOT_FOUND, format!("no note {id}")));
    }
    Ok(Json(json!({ "id": id, "archived": b.archived })))
}

async fn page(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    // Anything that isn't a built file is the app itself.
    let (file, name) = match Page::get(path) {
        Some(f) if !path.is_empty() => (f, path.to_string()),
        _ => match Page::get("index.html") {
            Some(f) => (f, "index.html".into()),
            None => return (StatusCode::NOT_FOUND, "this build of lethe has no page; build it with ./install.sh").into_response(),
        },
    };
    let mime = match name.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    };
    ([(header::CONTENT_TYPE, mime)], file.data.into_owned()).into_response()
}

pub fn cmd_serve() -> anyhow::Result<i32> {
    db::open()?;
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/projects", get(projects))
        .route("/api/notes", get(list))
        .route("/api/notes/{id}", get(one))
        .route("/api/notes/{id}/archive", post(archive))
        .fallback(page);
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let addr = format!("127.0.0.1:{}", paths::port());
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        println!("{} lethe {} serving {}", db::now(), env!("CARGO_PKG_VERSION"), paths::page_url());
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        Ok::<_, anyhow::Error>(())
    })?;
    Ok(0)
}
