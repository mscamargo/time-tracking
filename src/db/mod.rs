use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, Result, params};
use std::fs;
use std::path::PathBuf;

/// Represents a project in the time tracking system
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

/// Represents a time entry in the time tracking system
#[derive(Debug, Clone, PartialEq)]
pub struct TimeEntry {
    pub id: i64,
    pub project_id: Option<i64>,
    pub description: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

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

/// Creates a new project with the given name and color
pub fn create_project(conn: &Connection, name: &str, color: &str) -> Result<Project> {
    conn.execute(
        "INSERT INTO projects (name, color) VALUES (?1, ?2)",
        params![name, color],
    )?;

    let id = conn.last_insert_rowid();

    conn.query_row(
        "SELECT id, name, color, created_at FROM projects WHERE id = ?1",
        params![id],
        |row| {
            let created_at_str: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&format!("{}Z", created_at_str.replace(' ', "T")))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at,
            })
        },
    )
}

/// Retrieves all projects from the database
pub fn get_all_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at FROM projects ORDER BY name"
    )?;

    let projects = stmt.query_map([], |row| {
        let created_at_str: String = row.get(3)?;
        let created_at = DateTime::parse_from_rfc3339(&format!("{}Z", created_at_str.replace(' ', "T")))
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at,
        })
    })?;

    projects.collect()
}

/// Deletes a project by ID
pub fn delete_project(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    Ok(())
}

/// Helper function to parse SQLite datetime strings to DateTime<Utc>
fn parse_datetime(datetime_str: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&format!("{}Z", datetime_str.replace(' ', "T")))
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// Creates a new time entry with the given project_id, description, and start_time
pub fn create_entry(
    conn: &Connection,
    project_id: Option<i64>,
    description: &str,
    start_time: DateTime<Utc>,
) -> Result<TimeEntry> {
    let start_time_str = start_time.format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO time_entries (project_id, description, start_time) VALUES (?1, ?2, ?3)",
        params![project_id, description, start_time_str],
    )?;

    let id = conn.last_insert_rowid();

    conn.query_row(
        "SELECT id, project_id, description, start_time, end_time, created_at FROM time_entries WHERE id = ?1",
        params![id],
        |row| {
            let start_time_str: String = row.get(3)?;
            let end_time_str: Option<String> = row.get(4)?;
            let created_at_str: String = row.get(5)?;

            Ok(TimeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                description: row.get(2)?,
                start_time: parse_datetime(&start_time_str),
                end_time: end_time_str.map(|s| parse_datetime(&s)),
                created_at: parse_datetime(&created_at_str),
            })
        },
    )
}

/// Stops a time entry by setting its end_time
pub fn stop_entry(conn: &Connection, id: i64, end_time: DateTime<Utc>) -> Result<()> {
    let end_time_str = end_time.format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "UPDATE time_entries SET end_time = ?1 WHERE id = ?2",
        params![end_time_str, id],
    )?;

    Ok(())
}

/// Gets the currently running time entry (entry with null end_time)
pub fn get_running_entry(conn: &Connection) -> Result<Option<TimeEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, description, start_time, end_time, created_at
         FROM time_entries
         WHERE end_time IS NULL
         ORDER BY start_time DESC
         LIMIT 1"
    )?;

    let mut rows = stmt.query([])?;

    match rows.next()? {
        Some(row) => {
            let start_time_str: String = row.get(3)?;
            let end_time_str: Option<String> = row.get(4)?;
            let created_at_str: String = row.get(5)?;

            Ok(Some(TimeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                description: row.get(2)?,
                start_time: parse_datetime(&start_time_str),
                end_time: end_time_str.map(|s| parse_datetime(&s)),
                created_at: parse_datetime(&created_at_str),
            }))
        }
        None => Ok(None),
    }
}

/// Gets all time entries for a specific date
pub fn get_entries_for_date(conn: &Connection, date: NaiveDate) -> Result<Vec<TimeEntry>> {
    let date_str = date.format("%Y-%m-%d").to_string();

    let mut stmt = conn.prepare(
        "SELECT id, project_id, description, start_time, end_time, created_at
         FROM time_entries
         WHERE date(start_time) = ?1
         ORDER BY start_time DESC"
    )?;

    let entries = stmt.query_map(params![date_str], |row| {
        let start_time_str: String = row.get(3)?;
        let end_time_str: Option<String> = row.get(4)?;
        let created_at_str: String = row.get(5)?;

        Ok(TimeEntry {
            id: row.get(0)?,
            project_id: row.get(1)?,
            description: row.get(2)?,
            start_time: parse_datetime(&start_time_str),
            end_time: end_time_str.map(|s| parse_datetime(&s)),
            created_at: parse_datetime(&created_at_str),
        })
    })?;

    entries.collect()
}

/// Deletes a time entry by ID
pub fn delete_entry(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM time_entries WHERE id = ?1", params![id])?;
    Ok(())
}

/// Gets a project by ID
pub fn get_project_by_id(conn: &Connection, id: i64) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at FROM projects WHERE id = ?1"
    )?;

    let mut rows = stmt.query(params![id])?;

    match rows.next()? {
        Some(row) => {
            let created_at_str: String = row.get(3)?;
            Ok(Some(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at: parse_datetime(&created_at_str),
            }))
        }
        None => Ok(None),
    }
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

    #[test]
    fn test_create_project() {
        let conn = create_test_db();

        let project = create_project(&conn, "Work", "#3498db").unwrap();

        assert_eq!(project.id, 1);
        assert_eq!(project.name, "Work");
        assert_eq!(project.color, "#3498db");
    }

    #[test]
    fn test_get_all_projects_empty() {
        let conn = create_test_db();

        let projects = get_all_projects(&conn).unwrap();

        assert!(projects.is_empty());
    }

    #[test]
    fn test_get_all_projects() {
        let conn = create_test_db();

        create_project(&conn, "Work", "#3498db").unwrap();
        create_project(&conn, "Personal", "#e74c3c").unwrap();
        create_project(&conn, "Learning", "#2ecc71").unwrap();

        let projects = get_all_projects(&conn).unwrap();

        assert_eq!(projects.len(), 3);
        // Projects should be ordered by name
        assert_eq!(projects[0].name, "Learning");
        assert_eq!(projects[1].name, "Personal");
        assert_eq!(projects[2].name, "Work");
    }

    #[test]
    fn test_delete_project() {
        let conn = create_test_db();

        let project = create_project(&conn, "Work", "#3498db").unwrap();
        assert_eq!(get_all_projects(&conn).unwrap().len(), 1);

        delete_project(&conn, project.id).unwrap();

        let projects = get_all_projects(&conn).unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_project() {
        let conn = create_test_db();

        // Deleting a non-existent project should not error
        let result = delete_project(&conn, 999);
        assert!(result.is_ok());
    }

    // Time Entry CRUD Tests

    #[test]
    fn test_create_entry() {
        let conn = create_test_db();
        let start_time = Utc::now();

        let entry = create_entry(&conn, None, "Working on task", start_time).unwrap();

        assert_eq!(entry.id, 1);
        assert_eq!(entry.project_id, None);
        assert_eq!(entry.description, "Working on task");
        assert!(entry.end_time.is_none());
    }

    #[test]
    fn test_create_entry_with_project() {
        let conn = create_test_db();
        let project = create_project(&conn, "Work", "#3498db").unwrap();
        let start_time = Utc::now();

        let entry = create_entry(&conn, Some(project.id), "Project task", start_time).unwrap();

        assert_eq!(entry.project_id, Some(project.id));
        assert_eq!(entry.description, "Project task");
    }

    #[test]
    fn test_stop_entry() {
        let conn = create_test_db();
        let start_time = Utc::now();
        let entry = create_entry(&conn, None, "Task to stop", start_time).unwrap();

        let end_time = Utc::now();
        stop_entry(&conn, entry.id, end_time).unwrap();

        // Verify the entry was stopped
        let running = get_running_entry(&conn).unwrap();
        assert!(running.is_none());
    }

    #[test]
    fn test_get_running_entry_none() {
        let conn = create_test_db();

        let running = get_running_entry(&conn).unwrap();

        assert!(running.is_none());
    }

    #[test]
    fn test_get_running_entry_found() {
        let conn = create_test_db();
        let start_time = Utc::now();
        let created = create_entry(&conn, None, "Running task", start_time).unwrap();

        let running = get_running_entry(&conn).unwrap();

        assert!(running.is_some());
        let running_entry = running.unwrap();
        assert_eq!(running_entry.id, created.id);
        assert_eq!(running_entry.description, "Running task");
        assert!(running_entry.end_time.is_none());
    }

    #[test]
    fn test_get_running_entry_returns_most_recent() {
        let conn = create_test_db();

        // Create multiple running entries (edge case)
        let start1 = Utc::now();
        create_entry(&conn, None, "First task", start1).unwrap();

        let start2 = Utc::now();
        let second = create_entry(&conn, None, "Second task", start2).unwrap();

        let running = get_running_entry(&conn).unwrap();

        assert!(running.is_some());
        // Should return the most recent by start_time
        assert_eq!(running.unwrap().id, second.id);
    }

    #[test]
    fn test_get_entries_for_date_empty() {
        let conn = create_test_db();
        let today = Utc::now().date_naive();

        let entries = get_entries_for_date(&conn, today).unwrap();

        assert!(entries.is_empty());
    }

    #[test]
    fn test_get_entries_for_date() {
        let conn = create_test_db();

        // Create entries for today
        let now = Utc::now();
        create_entry(&conn, None, "Task 1", now).unwrap();
        create_entry(&conn, None, "Task 2", now).unwrap();

        let today = now.date_naive();
        let entries = get_entries_for_date(&conn, today).unwrap();

        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_get_entries_for_date_filters_by_date() {
        let conn = create_test_db();

        // Create an entry for today
        let now = Utc::now();
        create_entry(&conn, None, "Today's task", now).unwrap();

        // Manually insert an entry for a different date
        conn.execute(
            "INSERT INTO time_entries (project_id, description, start_time) VALUES (NULL, 'Old task', '2020-01-15 10:00:00')",
            [],
        ).unwrap();

        let today = now.date_naive();
        let entries = get_entries_for_date(&conn, today).unwrap();

        // Should only get today's entry
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].description, "Today's task");
    }

    #[test]
    fn test_delete_entry() {
        let conn = create_test_db();
        let start_time = Utc::now();
        let entry = create_entry(&conn, None, "Task to delete", start_time).unwrap();

        delete_entry(&conn, entry.id).unwrap();

        let today = start_time.date_naive();
        let entries = get_entries_for_date(&conn, today).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_entry() {
        let conn = create_test_db();

        // Deleting a non-existent entry should not error
        let result = delete_entry(&conn, 999);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_project_by_id() {
        let conn = create_test_db();
        let project = create_project(&conn, "Work", "#3498db").unwrap();

        let found = get_project_by_id(&conn, project.id).unwrap();

        assert!(found.is_some());
        let found_project = found.unwrap();
        assert_eq!(found_project.id, project.id);
        assert_eq!(found_project.name, "Work");
        assert_eq!(found_project.color, "#3498db");
    }

    #[test]
    fn test_get_project_by_id_not_found() {
        let conn = create_test_db();

        let found = get_project_by_id(&conn, 999).unwrap();

        assert!(found.is_none());
    }
}
