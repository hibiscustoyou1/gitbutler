use crate::reader::{self, Reader, SubReader};

use super::Branch;

pub struct BranchReader<'reader> {
    reader: &'reader dyn reader::Reader,
}

impl<'reader> BranchReader<'reader> {
    pub fn new(reader: &'reader dyn Reader) -> Self {
        Self { reader }
    }

    pub fn read_selected(&self) -> Result<Option<String>, reader::Error> {
        match self.reader.read_string("branches/selected") {
            Ok(selected) => Ok(Some(selected)),
            Err(reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn read(&self, id: &str) -> Result<Branch, reader::Error> {
        if !self.reader.exists(&format!("branches/{}", id)) {
            return Err(reader::Error::NotFound);
        }

        let single_reader: &dyn crate::reader::Reader =
            &SubReader::new(self.reader, &format!("branches/{}", id));
        Branch::try_from(single_reader)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{
        gb_repository, projects, sessions, storage, users, virtual_branches::branch::Ownership,
    };

    use super::{super::Writer, *};

    static mut TEST_INDEX: usize = 0;

    fn test_branch() -> Branch {
        unsafe {
            TEST_INDEX += 1;
        }
        Branch {
            id: format!("branch_{}", unsafe { TEST_INDEX }),
            name: format!("branch_name_{}", unsafe { TEST_INDEX }),
            applied: true,
            upstream: format!("upstream_{}", unsafe { TEST_INDEX }),
            created_timestamp_ms: unsafe { TEST_INDEX } as u128,
            updated_timestamp_ms: unsafe { TEST_INDEX + 100 } as u128,
            head: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                unsafe { TEST_INDEX }
            ))
            .unwrap(),
            tree: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef012345{}",
                unsafe { TEST_INDEX + 10 }
            ))
            .unwrap(),
            ownership: vec![Ownership {
                file_path: format!("file/{}", unsafe { TEST_INDEX }).into(),
                hunks: vec![],
            }],
        }
    }

    fn test_repository() -> Result<git2::Repository> {
        let path = tempdir()?.path().to_str().unwrap().to_string();
        let repository = git2::Repository::init(path)?;
        let mut index = repository.index()?;
        let oid = index.write_tree()?;
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &repository.find_tree(oid)?,
            &[],
        )?;
        Ok(repository)
    }

    #[test]
    fn test_read_not_found() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;

        let reader = BranchReader::new(&session_reader);
        let result = reader.read("not found");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "file not found");

        Ok(())
    }

    #[test]
    fn test_read_override() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let branch = test_branch();

        let writer = Writer::new(&gb_repo);
        writer.write(&branch)?;

        let session = gb_repo.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;

        let reader = BranchReader::new(&session_reader);

        assert_eq!(branch, reader.read(&branch.id).unwrap());

        Ok(())
    }

    #[test]
    fn test_read_selected() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
        let reader = BranchReader::new(&session_reader);
        let writer = Writer::new(&gb_repo);

        assert_eq!(None, reader.read_selected()?);

        writer.write_selected(&Some("test".to_string()))?;
        assert_eq!(Some("test".to_string()), reader.read_selected()?);

        writer.write_selected(&Some("updated".to_string()))?;
        assert_eq!(Some("updated".to_string()), reader.read_selected()?);

        Ok(())
    }
}
