use std::path::Path;

use rusqlite::{params, Connection};

use crate::model::Snapshot;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS activity (
   server TEXT PRIMARY KEY,
   name TEXT,
   "0" INTEGER DEFAULT 0,
   "1" INTEGER DEFAULT 0,
   "2" INTEGER DEFAULT 0,
   "3" INTEGER DEFAULT 0,
   "4" INTEGER DEFAULT 0,
   "5" INTEGER DEFAULT 0,
   "6" INTEGER DEFAULT 0,
   "7" INTEGER DEFAULT 0,
   "8" INTEGER DEFAULT 0,
   "9" INTEGER DEFAULT 0,
   "10" INTEGER DEFAULT 0,
   "11" INTEGER DEFAULT 0,
   "12" INTEGER DEFAULT 0,
   "13" INTEGER DEFAULT 0,
   "14" INTEGER DEFAULT 0,
   "15" INTEGER DEFAULT 0,
   "16" INTEGER DEFAULT 0,
   "17" INTEGER DEFAULT 0,
   "18" INTEGER DEFAULT 0,
   "19" INTEGER DEFAULT 0,
   "20" INTEGER DEFAULT 0,
   "21" INTEGER DEFAULT 0,
   "22" INTEGER DEFAULT 0,
   "23" INTEGER DEFAULT 0
);
"#;

pub struct ActivityDb {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct ActivityRow {
    #[allow(dead_code)]
    pub server: String,
    pub name: String,
    pub hours: [i64; 24],
}

impl ActivityRow {
    pub fn total(&self) -> i64 {
        self.hours.iter().sum()
    }
}

impl ActivityDb {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn record(&self, snap: &Snapshot) -> anyhow::Result<()> {
        let hour = chrono::Timelike::hour(&chrono::Utc::now()) as usize;
        let col = hour.to_string();
        let sql = format!(
            r#"INSERT INTO activity (server, name, "{col}") VALUES (?1, ?2, 1)
               ON CONFLICT(server) DO UPDATE SET "{col}" = "{col}" + 1, name = excluded.name"#
        );
        for s in snap.server.values() {
            if s.numplayers + s.numbots <= 0 {
                continue;
            }
            self.conn.execute(&sql, params![s.address, s.realname])?;
        }
        Ok(())
    }

    pub fn all(&self) -> anyhow::Result<Vec<ActivityRow>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT server, name,
                "0","1","2","3","4","5","6","7","8","9","10","11",
                "12","13","14","15","16","17","18","19","20","21","22","23"
               FROM activity"#,
        )?;
        let rows = stmt.query_map([], |row| {
            let mut hours = [0i64; 24];
            for h in 0..24 {
                hours[h] = row.get(h + 2)?;
            }
            Ok(ActivityRow {
                server: row.get(0)?,
                name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                hours,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
