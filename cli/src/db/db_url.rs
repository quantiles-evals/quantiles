use std::path::PathBuf;

#[derive(Debug)]
pub enum DBUrl {
    SQLitePath(SQLitePathURL),
}

#[derive(Debug)]
pub struct SQLitePathURL {
    pub path: PathBuf,
    pub create: bool,
}
