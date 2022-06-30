use super::schema::*;
use super::tonic_sqlite;
use diesel::{prelude::*, sqlite::SqliteConnection, Insertable, Queryable};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Debug, QueryableByName, Queryable, Insertable)]
#[table_name = "grsql"]
pub struct Data {
    #[serde(default = "Default::default", skip_serializing_if = "is_zero")]
    pub id: i32,
    pub name: String,
    pub mime: String,
    #[serde(default = "now")]
    pub created: i32,
    #[serde(default = "now")]
    pub updated: i32,
    #[serde(default = "Default::default")]
    pub content: Vec<u8>,
}

impl Data {
    pub fn new() -> Self {
        let now = now();
        Self {
            id: 0,
            name: String::new(),
            mime: "text/plain".into(),
            created: now,
            updated: now,
            content: Vec::new(),
        }
    }

    pub fn validate(&mut self) -> bool {
        if self.created == 0 {
            self.created = now();
        }
        if self.updated == 0 {
            self.updated = now();
        }
        self.id != 0 && !self.name.is_empty() && !self.mime.is_empty() && !self.content.is_empty()
    }
}

impl From<tonic_sqlite::Data> for Data {
    fn from(d: tonic_sqlite::Data) -> Self {
        Self {
            id: d.id.unwrap_or(0) as _,
            name: d.name,
            mime: d.mime,
            created: d.created.unwrap_or(now()),
            updated: d.updated.unwrap_or(now()),
            content: d.content,
        }
    }
}

impl Into<tonic_sqlite::Data> for Data {
    fn into(self) -> tonic_sqlite::Data {
        let id = match self.id {
            0 => None,
            _ => Some(self.id),
        };
        let created = match self.created {
            0 => None,
            _ => Some(self.created),
        };
        let updated = match self.updated {
            0 => None,
            _ => Some(self.updated),
        };
        tonic_sqlite::Data {
            id: id,
            name: self.name,
            mime: self.mime,
            created,
            updated,
            content: self.content,
        }
    }
}

pub fn now() -> i32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as _
}

pub fn is_zero(en: &i32) -> bool {
    en == &0
}

#[allow(dead_code)]
pub fn establish_connection() -> SqliteConnection {
    dotenv::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("database_url must not be null");
    SqliteConnection::establish(&database_url)
        .expect(&format!("fail to connect to {}", database_url))
}
