use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, Database, Statement};

use crate::db::{DBUrl, SQLitePathURL, schema};

const DB_DIR: &str = ".quantiles";
const DB_FILE: &str = "quantiles.sqlite";

/// Look for `.quantiles` directory under the current working directory, then
/// if it's not found, walk up to parents looking for a `.quantiles` directory.
///
/// When the first such `.quantiles` directory is found, return its containing
/// dir.
///
/// If `create` is `true`, ensure the discovered `.quantiles` workspace is initialized.
/// Otherwise, initializes a new workspace in `cwd` and returns `cwd`.
///
/// # Errors
///
/// Returns an error if the current directory cannot be determined or
/// workspace initialization fails.
pub async fn resolve_workspace_root(cwd: impl AsRef<Path>, create: bool) -> Result<PathBuf> {
    let cwd = cwd.as_ref();

    for ancestor in cwd.ancestors() {
        if ancestor.join(DB_DIR).is_dir() {
            if create {
                init_workspace(ancestor).await?;
            }
            return Ok(ancestor.to_path_buf());
        }
    }

    // if cwd or and ancestor wasn't found, then check if we need to create
    if create {
        init_workspace(cwd).await?;
    }

    Ok(cwd.to_path_buf())
}

/// Initialize a quantiles workspace under `root`.
///
/// # Errors
///
/// Returns an error if the `.quantiles` directory cannot be created, the sqlite
/// database cannot be opened, or the schema cannot be applied.
pub async fn init_workspace(root: &Path) -> Result<PathBuf> {
    let qt_dir = root.join(DB_DIR);
    fs::create_dir_all(&qt_dir)
        .with_context(|| format!("failed to create {}", qt_dir.display()))?;
    fs::create_dir_all(metrics_dir(root)).context("failed to create metrics dir")?;

    let db_path = workspace_db_path(root);
    let db_url = DBUrl::SQLitePath(SQLitePathURL {
        path: db_path.clone(),
        create: true,
    });
    let db = open_database(db_url)
        .await
        .with_context(|| format!("failed to open {}", db_path.display()))?;

    schema::apply_schema(&db)
        .await
        .context("failed to initialize database schema")?;

    Ok(db_path)
}

/// Open the qt workspace database under `root`.
///
/// # Errors
///
/// Returns an error if the workspace database cannot be opened.
pub async fn open_workspace(root: &Path) -> Result<sea_orm::DatabaseConnection> {
    let db_path = workspace_db_path(root);
    let db_url = DBUrl::SQLitePath(SQLitePathURL {
        path: db_path.clone(),
        create: false,
    });

    open_database(db_url)
        .await
        .with_context(|| format!("failed to open {}; run `qt init` first", db_path.display()))
}

/// Open a sqlite database directly by URL.
///
/// # Errors
///
/// Returns an error if the database cannot be opened.
pub async fn open_database(db_url: DBUrl) -> Result<sea_orm::DatabaseConnection> {
    let url_str = match &db_url {
        DBUrl::SQLitePath(SQLitePathURL { path, create }) => {
            if *create {
                format!("sqlite://{}?mode=rwc", path.display())
            } else {
                format!("sqlite://{}?mode=rw", path.display())
            }
        }
    };

    let conn = Database::connect(url_str)
        .await
        .with_context(|| format!("failed to open {db_url:?}"))?;
    // Enable WAL mode so readers and writers do not block each other.
    // This is essential for concurrent step requests from the SDK.
    let backend = conn.get_database_backend();
    conn.execute(Statement::from_string(backend, "PRAGMA journal_mode = WAL"))
        .await?;
    conn.execute(Statement::from_string(
        backend,
        "PRAGMA busy_timeout = 5000",
    ))
    .await?;
    Ok(conn)
}

#[must_use]
pub fn workspace_db_path(root: &Path) -> PathBuf {
    root.join(DB_DIR).join(DB_FILE)
}

/// Path to the Parquet metrics directory under `root`.
#[must_use]
pub fn metrics_dir(root: &Path) -> PathBuf {
    root.join(DB_DIR).join("metrics")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_finds_workspace_in_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join(DB_DIR)).unwrap();

        let resolved = resolve_workspace_root(&root, false).await.unwrap();
        assert_eq!(resolved, root);
    }

    #[tokio::test]
    async fn resolve_finds_workspace_in_ancestor() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        let sub = root.join("a").join("b").join("c");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir(root.join(DB_DIR)).unwrap();

        let resolved = resolve_workspace_root(&sub, false).await.unwrap();
        assert_eq!(resolved, root);
    }

    #[tokio::test]
    async fn resolve_creates_workspace_when_not_found_and_create_true() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().join("fresh");
        fs::create_dir(&cwd).unwrap();

        assert!(!cwd.join(DB_DIR).exists());
        let resolved = resolve_workspace_root(&cwd, true).await.unwrap();
        assert_eq!(resolved, cwd);
        assert!(cwd.join(DB_DIR).join(DB_FILE).exists());
    }

    #[tokio::test]
    async fn resolve_initializes_existing_workspace_dir_when_create_true() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().join("fresh");
        fs::create_dir(&cwd).unwrap();
        fs::create_dir(cwd.join(DB_DIR)).unwrap();

        assert!(!cwd.join(DB_DIR).join(DB_FILE).exists());
        let resolved = resolve_workspace_root(&cwd, true).await.unwrap();
        assert_eq!(resolved, cwd);
        assert!(cwd.join(DB_DIR).join(DB_FILE).exists());
        assert!(metrics_dir(&cwd).is_dir());
    }

    #[tokio::test]
    async fn resolve_returns_cwd_without_creating_when_not_found_and_create_false() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().join("readonly");
        fs::create_dir(&cwd).unwrap();

        let resolved = resolve_workspace_root(&cwd, false).await.unwrap();
        assert_eq!(resolved, cwd);
        assert!(!cwd.join(DB_DIR).exists());
    }

    #[tokio::test]
    async fn resolve_prefers_closest_ancestor() {
        let tmp = tempfile::tempdir().unwrap();
        let outer = tmp.path().join("outer");
        let inner = outer.join("inner");
        let deep = inner.join("deep");
        fs::create_dir_all(&deep).unwrap();

        // create workspaces at both levels
        fs::create_dir(outer.join(DB_DIR)).unwrap();
        fs::create_dir(inner.join(DB_DIR)).unwrap();

        let resolved = resolve_workspace_root(&deep, false).await.unwrap();
        assert_eq!(resolved, inner);
    }
}
