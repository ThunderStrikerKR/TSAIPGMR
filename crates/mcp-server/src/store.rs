use std::{error::Error, fs, path::PathBuf, sync::Arc};

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone)]
pub struct Store(Arc<PathBuf>);

#[derive(Serialize)]
pub struct Receipt {
    pub project_id: String,
    pub request_id: String,
    pub receipt_id: String,
    pub result: String,
}

pub enum WriteOutcome {
    Stored(Receipt),
    Replayed(Receipt),
    Conflict,
}

impl Store {
    pub fn open(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        if path.as_os_str().is_empty() || path.as_os_str() == ":memory:" {
            return Err("MCP_DB_PATH must name a persistent file".into());
        }
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let store = Self(Arc::new(path));
        store.connect()?.execute_batch(
            "CREATE TABLE IF NOT EXISTS synthetic_results (
                project_id TEXT NOT NULL,
                request_id TEXT NOT NULL,
                receipt_id TEXT NOT NULL UNIQUE,
                result TEXT NOT NULL,
                PRIMARY KEY (project_id, request_id)
            );",
        )?;
        Ok(store)
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        Connection::open_with_flags(
            self.0.as_ref(),
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
    }

    pub fn record(
        &self,
        project_id: String,
        request_id: String,
        result: String,
    ) -> rusqlite::Result<WriteOutcome> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(receipt) = read(&transaction, &project_id, &request_id)? {
            return Ok(if receipt.result == result {
                WriteOutcome::Replayed(receipt)
            } else {
                WriteOutcome::Conflict
            });
        }
        let receipt = Receipt {
            project_id,
            request_id,
            receipt_id: Uuid::new_v4().to_string(),
            result,
        };
        transaction.execute(
            "INSERT INTO synthetic_results (project_id, request_id, receipt_id, result) VALUES (?1, ?2, ?3, ?4)",
            params![receipt.project_id, receipt.request_id, receipt.receipt_id, receipt.result],
        )?;
        transaction.commit()?;
        Ok(WriteOutcome::Stored(receipt))
    }

    pub fn get(&self, project_id: &str, request_id: &str) -> rusqlite::Result<Option<Receipt>> {
        read(&self.connect()?, project_id, request_id)
    }
}

fn read(
    connection: &Connection,
    project_id: &str,
    request_id: &str,
) -> rusqlite::Result<Option<Receipt>> {
    connection.query_row(
        "SELECT receipt_id, result FROM synthetic_results WHERE project_id = ?1 AND request_id = ?2",
        params![project_id, request_id],
        |row| Ok(Receipt {
            project_id: project_id.into(),
            request_id: request_id.into(),
            receipt_id: row.get(0)?,
            result: row.get(1)?,
        }),
    ).optional()
}
