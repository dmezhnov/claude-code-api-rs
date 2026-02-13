use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};
use std::str::FromStr;

/// Initialize the SQLite connection pool and run migrations.
pub async fn init_db(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT DEFAULT '',
            path TEXT UNIQUE,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            is_active INTEGER NOT NULL DEFAULT 1
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            project_id TEXT REFERENCES projects(id),
            title TEXT DEFAULT '',
            model TEXT NOT NULL,
            system_prompt TEXT DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            is_active INTEGER NOT NULL DEFAULT 1,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost REAL NOT NULL DEFAULT 0.0,
            message_count INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES sessions(id),
            role TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            message_metadata TEXT DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cost REAL NOT NULL DEFAULT 0.0
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS api_keys (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key_hash TEXT UNIQUE NOT NULL,
            name TEXT DEFAULT '',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_used_at TEXT,
            total_requests INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost REAL NOT NULL DEFAULT 0.0
        )",
    )
    .execute(pool)
    .await?;

    tracing::info!("Database migrations completed");
    Ok(())
}

// -- Row types --

#[derive(Debug, FromRow, Serialize)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub is_active: i32,
}

#[derive(Debug, FromRow, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub model: String,
    pub system_prompt: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_active: i32,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub message_count: i64,
}

// -- Project CRUD --

pub async fn create_project(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    description: &str,
    path: Option<&str>,
) -> Result<ProjectRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO projects (id, name, description, path) VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(path)
    .execute(pool)
    .await?;

    get_project(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn list_projects(pool: &SqlitePool) -> Result<Vec<ProjectRow>, sqlx::Error> {
    sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, path, created_at, updated_at, is_active
         FROM projects WHERE is_active = 1 ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_project(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<ProjectRow>, sqlx::Error> {
    sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, path, created_at, updated_at, is_active
         FROM projects WHERE id = ? AND is_active = 1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn delete_project(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE projects SET is_active = 0 WHERE id = ? AND is_active = 1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// -- Session CRUD --

pub async fn create_session(
    pool: &SqlitePool,
    id: &str,
    project_id: Option<&str>,
    model: &str,
    system_prompt: Option<&str>,
    title: Option<&str>,
) -> Result<SessionRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO sessions (id, project_id, model, system_prompt, title)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(model)
    .bind(system_prompt.unwrap_or(""))
    .bind(title.unwrap_or(""))
    .execute(pool)
    .await?;

    get_session(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<SessionRow>, sqlx::Error> {
    sqlx::query_as::<_, SessionRow>(
        "SELECT id, project_id, title, model, system_prompt, created_at, updated_at,
                is_active, total_tokens, total_cost, message_count
         FROM sessions WHERE is_active = 1 ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_session(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<SessionRow>, sqlx::Error> {
    sqlx::query_as::<_, SessionRow>(
        "SELECT id, project_id, title, model, system_prompt, created_at, updated_at,
                is_active, total_tokens, total_cost, message_count
         FROM sessions WHERE id = ? AND is_active = 1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn delete_session(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("UPDATE sessions SET is_active = 0 WHERE id = ? AND is_active = 1")
            .bind(id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_session_metrics(
    pool: &SqlitePool,
    id: &str,
    tokens: i64,
    cost: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE sessions
         SET total_tokens = total_tokens + ?,
             total_cost = total_cost + ?,
             message_count = message_count + 1,
             updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(tokens)
    .bind(cost)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

// -- Message CRUD --

pub async fn add_message(
    pool: &SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO messages (session_id, role, content, input_tokens, output_tokens, cost)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(cost)
    .execute(pool)
    .await?;
    Ok(())
}
