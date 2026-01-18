use rusqlite::{Connection, Result};
use std::fs;
use std::path::PathBuf;

/// Returns the path to the database file in XDG data directory
pub fn get_db_path() -> PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("time-tracking");

    fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    data_dir.join("time-tracking.db")
}

/// Initialize the database connection and create tables if they don't exist
pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path();
    let conn = Connection::open(&db_path)?;

    create_tables(&conn)?;

    Ok(conn)
}

/// Create database tables if they don't exist
fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            color TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS time_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER,
            description TEXT NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL
        )",
        [],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::collections::HashSet;

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_tables_exist() {
        let conn = create_test_db();

        // Query sqlite_master to get all table names
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
            .unwrap();

        let tables: HashSet<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains("projects"), "projects table should exist");
        assert!(tables.contains("time_entries"), "time_entries table should exist");
    }

    #[test]
    fn test_projects_table_schema() {
        let conn = create_test_db();

        // Verify we can insert into projects table with expected columns
        conn.execute(
            "INSERT INTO projects (name, color) VALUES (?1, ?2)",
            ["Test Project", "#FF0000"],
        ).unwrap();

        let mut stmt = conn
            .prepare("SELECT id, name, color, created_at FROM projects")
            .unwrap();

        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();

        let id: i64 = row.get(0).unwrap();
        let name: String = row.get(1).unwrap();
        let color: String = row.get(2).unwrap();
        let created_at: String = row.get(3).unwrap();

        assert_eq!(id, 1);
        assert_eq!(name, "Test Project");
        assert_eq!(color, "#FF0000");
        assert!(!created_at.is_empty());
    }

    #[test]
    fn test_time_entries_table_schema() {
        let conn = create_test_db();

        // Insert a project first
        conn.execute(
            "INSERT INTO projects (name, color) VALUES (?1, ?2)",
            ["Test Project", "#FF0000"],
        ).unwrap();

        // Insert a time entry
        conn.execute(
            "INSERT INTO time_entries (project_id, description, start_time) VALUES (?1, ?2, ?3)",
            [Some("1"), Some("Working on feature"), Some("2024-01-15T10:00:00")],
        ).unwrap();

        let mut stmt = conn
            .prepare("SELECT id, project_id, description, start_time, end_time, created_at FROM time_entries")
            .unwrap();

        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();

        let id: i64 = row.get(0).unwrap();
        let project_id: Option<i64> = row.get(1).unwrap();
        let description: String = row.get(2).unwrap();
        let start_time: String = row.get(3).unwrap();
        let end_time: Option<String> = row.get(4).unwrap();
        let created_at: String = row.get(5).unwrap();

        assert_eq!(id, 1);
        assert_eq!(project_id, Some(1));
        assert_eq!(description, "Working on feature");
        assert_eq!(start_time, "2024-01-15T10:00:00");
        assert!(end_time.is_none());
        assert!(!created_at.is_empty());
    }

    #[test]
    fn test_time_entry_without_project() {
        let conn = create_test_db();

        // Insert a time entry without a project
        conn.execute(
            "INSERT INTO time_entries (project_id, description, start_time) VALUES (?1, ?2, ?3)",
            [None::<&str>, Some("No project task"), Some("2024-01-15T10:00:00")],
        ).unwrap();

        let project_id: Option<i64> = conn
            .query_row(
                "SELECT project_id FROM time_entries WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(project_id.is_none());
    }
}
