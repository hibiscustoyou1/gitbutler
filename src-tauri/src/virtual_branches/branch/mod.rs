mod hunk;
mod ownership;
mod reader;
mod writer;

pub use reader::BranchReader as Reader;
pub use writer::BranchWriter as Writer;

use serde::{Deserialize, Serialize};

use anyhow::Result;

pub use ownership::Ownership;

#[derive(Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub applied: bool,
    pub upstream: String,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
    pub tree: git2::Oid, // last git tree written to a session, or merge base tree if this is new. use this for delta calculation from the session data
    pub head: git2::Oid,
    pub ownership: Vec<Ownership>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchUpdateRequest {
    pub id: String,
    pub name: Option<String>,
}

impl Branch {
    pub fn put(&mut self, ownership: &Ownership) {
        let target = self
            .ownership
            .iter()
            .cloned()
            .find(|o| o.file_path == ownership.file_path);

        self.ownership
            .retain(|o| o.file_path != ownership.file_path);

        if let Some(target) = target {
            self.ownership.push(target.plus(ownership));
        } else {
            self.ownership.push(ownership.clone());
        }

        self.ownership.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        self.ownership.dedup();
    }

    pub fn take(&mut self, ownership: &Ownership) {
        let target = self
            .ownership
            .iter()
            .cloned()
            .find(|o| o.file_path == ownership.file_path);

        self.ownership
            .retain(|o| o.file_path != ownership.file_path);

        if let Some(target) = target.as_ref() {
            if let Some(remaining) = target.minus(ownership) {
                self.ownership.push(remaining);
            }
        }

        self.ownership.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        self.ownership.dedup();
    }

    pub fn contains(&self, ownership: &Ownership) -> bool {
        self.ownership.iter().any(|o| o.contains(ownership))
    }
}

impl TryFrom<&dyn crate::reader::Reader> for Branch {
    type Error = crate::reader::Error;

    fn try_from(reader: &dyn crate::reader::Reader) -> Result<Self, Self::Error> {
        let id = reader.read_string("id").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("id: {}", e),
            ))
        })?;
        let name = reader.read_string("meta/name").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/name: {}", e),
            ))
        })?;
        let applied = reader.read_bool("meta/applied").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/applied: {}", e),
            ))
        })?;
        let upstream = reader.read_string("meta/upstream").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/upstream: {}", e),
            ))
        })?;
        let tree = reader.read_string("meta/tree").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/tree: {}", e),
            ))
        })?;
        let head = reader.read_string("meta/head").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/head: {}", e),
            ))
        })?;
        let created_timestamp_ms = reader.read_u128("meta/created_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/created_timestamp_ms: {}", e),
            ))
        })?;
        let updated_timestamp_ms = reader.read_u128("meta/updated_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/updated_timestamp_ms: {}", e),
            ))
        })?;

        let ownership_string = reader.read_string("meta/ownership").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/ownership: {}", e),
            ))
        })?;
        let ownership = ownership_string
            .lines()
            .map(Ownership::parse_string)
            .collect::<Result<Vec<Ownership>>>()
            .map_err(|e| {
                crate::reader::Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("meta/ownership: {}", e),
                ))
            })?;

        Ok(Self {
            id,
            name,
            applied,
            upstream,
            tree: git2::Oid::from_str(&tree).unwrap(),
            head: git2::Oid::from_str(&head).unwrap(),
            created_timestamp_ms,
            updated_timestamp_ms,
            ownership,
        })
    }
}
