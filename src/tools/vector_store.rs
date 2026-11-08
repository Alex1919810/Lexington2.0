//! Embedded, namespace-isolated document storage. Embeddings are replaceable indices.
use rusqlite::{Connection, params};
use std::{path::Path, sync::OnceLock, time::Duration};

pub struct VectorStore(Connection);

pub struct Document {
    pub id: String,
    pub text: String,
    pub created_at: i64,
}

pub struct SearchHit {
    pub document: Document,
    pub distance: f64,
}

fn storage_error(_: rusqlite::Error) -> String {
    "本地向量数据库操作失败，原有数据未被重置。".into()
}

impl VectorStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        static REGISTERED: OnceLock<bool> = OnceLock::new();
        if !REGISTERED.get_or_init(|| unsafe {
            // sqlite-vec is statically linked; register only this compiled-in extension.
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut std::ffi::c_char,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> std::ffi::c_int,
            >(
                sqlite_vec::sqlite3_vec_init as *const ()
            ))) == rusqlite::ffi::SQLITE_OK
        }) {
            return Err("无法初始化本地向量扩展。".into());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| "无法创建向量数据库目录。")?;
        }
        // Create with private permissions before SQLite opens the database.
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(_) => return Err("无法创建向量数据库。".into()),
        }
        let connection = Connection::open(path).map_err(storage_error)?;
        connection
            .busy_timeout(Duration::from_millis(250))
            .map_err(storage_error)?;
        connection
            .execute_batch("PRAGMA secure_delete=ON;")
            .map_err(storage_error)?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(storage_error)?;
        if version == 0 {
            let tables: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                    [],
                    |row| row.get(0),
                )
                .map_err(storage_error)?;
            if tables != 0 {
                return Err("向量数据库格式未知，未覆盖。".into());
            }
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                CREATE TABLE documents (
                    namespace TEXT NOT NULL, id TEXT NOT NULL, text TEXT NOT NULL,
                    created_at INTEGER NOT NULL, space TEXT, dimensions INTEGER, embedding BLOB,
                    PRIMARY KEY(namespace, id)
                );
                CREATE INDEX documents_namespace ON documents(namespace, created_at);
                PRAGMA user_version=1;
                COMMIT;",
                )
                .map_err(storage_error)?;
        } else if version != 1 {
            return Err("向量数据库版本不受支持，未覆盖。".into());
        }
        Ok(Self(connection))
    }

    pub fn put(&self, namespace: &str, document: &Document) -> Result<(), String> {
        if namespace.is_empty()
            || namespace.len() > 200
            || document.id.is_empty()
            || document.id.len() > 200
            || document.text.trim().is_empty()
            || document.text.chars().count() > 8192
        {
            return Err("向量文档标识或正文无效。".into());
        }
        self.0.execute("INSERT INTO documents(namespace,id,text,created_at) VALUES (?1,?2,?3,?4)
            ON CONFLICT(namespace,id) DO UPDATE SET text=excluded.text, created_at=excluded.created_at,
                embedding=NULL,space=NULL,dimensions=NULL WHERE documents.text<>excluded.text",
            params![namespace,document.id,document.text,document.created_at]).map_err(storage_error)?;
        Ok(())
    }

    pub fn pending(
        &self,
        namespace: &str,
        space: &str,
        limit: usize,
    ) -> Result<Vec<Document>, String> {
        let mut statement = self.0.prepare("SELECT id,text,created_at FROM documents WHERE namespace=?1 AND (space IS NULL OR space<>?2) ORDER BY created_at DESC,rowid DESC LIMIT ?3").map_err(storage_error)?;
        statement
            .query_map(
                params![namespace, space, limit.min(64) as i64],
                read_document,
            )
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)
    }

    pub fn set_embedding(
        &self,
        namespace: &str,
        document: &Document,
        space: &str,
        vector: &[f32],
    ) -> Result<(), String> {
        let vector = encode_vector(vector)?;
        self.0.execute("UPDATE documents SET space=?1,dimensions=?2,embedding=?3 WHERE namespace=?4 AND id=?5 AND text=?6",
            params![space,vector.len()/4,vector,namespace,document.id,document.text]).map_err(storage_error)?;
        Ok(())
    }

    pub fn search(
        &self,
        namespace: &str,
        space: &str,
        query: &[f32],
        limit: usize,
        max_distance: f64,
    ) -> Result<Vec<SearchHit>, String> {
        let vector = encode_vector(query)?;
        if !max_distance.is_finite() || !(0.0..=2.0).contains(&max_distance) {
            return Err("向量检索阈值无效。".into());
        }
        // Scalar cosine search lets each namespace/model use its own dimensions. All
        // filters apply before ranking, so another namespace cannot crowd out results.
        let mut statement = self.0.prepare("SELECT id,text,created_at,vec_distance_cosine(embedding,?1) AS distance
            FROM documents WHERE namespace=?2 AND space=?3 AND dimensions=?4 AND embedding IS NOT NULL
            AND distance<=?5 ORDER BY distance,created_at DESC,rowid DESC LIMIT ?6").map_err(storage_error)?;
        statement
            .query_map(
                params![
                    vector,
                    namespace,
                    space,
                    query.len(),
                    max_distance,
                    limit.min(32) as i64
                ],
                |row| {
                    Ok(SearchHit {
                        document: read_document(row)?,
                        distance: row.get(3)?,
                    })
                },
            )
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)
    }

    pub fn delete(&self, namespace: &str, id: Option<&str>) -> Result<(), String> {
        self.0
            .execute(
                "DELETE FROM documents WHERE namespace=?1 AND (?2 IS NULL OR id=?2)",
                params![namespace, id],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn prune(&self, namespace: &str, keep: usize) -> Result<(), String> {
        self.0.execute("DELETE FROM documents WHERE namespace=?1 AND id NOT IN (SELECT id FROM documents WHERE namespace=?1 ORDER BY created_at DESC,rowid DESC LIMIT ?2)", params![namespace,keep.min(i64::MAX as usize) as i64]).map_err(storage_error)?;
        Ok(())
    }
}

fn read_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        text: row.get(1)?,
        created_at: row.get(2)?,
    })
}

fn encode_vector(vector: &[f32]) -> Result<Vec<u8>, String> {
    if vector.is_empty()
        || vector.len() > 4096
        || vector.iter().any(|value| !value.is_finite())
        || vector.iter().all(|value| *value == 0.0)
    {
        return Err("向量必须为 1～4096 维的有限非零数值。".into());
    }
    Ok(vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistence_namespace_model_edit_delete_and_validation() {
        let root = std::env::temp_dir().join(format!("lexington-vector-{}", rand::random::<u64>()));
        let path = root.join("memory.sqlite3");
        let store = VectorStore::open(&path).unwrap();
        for (namespace, id, vector, time) in [
            ("a", "tea", [1., 0.], 1),
            ("b", "other", [1., 0.], 2),
            ("a", "coffee", [0., 1.], 3),
        ] {
            let document = Document {
                id: id.into(),
                text: format!("fact {id}"),
                created_at: time,
            };
            store.put(namespace, &document).unwrap();
            store
                .set_embedding(namespace, &document, "model1", &vector)
                .unwrap();
        }
        drop(store);
        let store = VectorStore::open(&path).unwrap();
        let hits = store.search("a", "model1", &[1., 0.], 3, 0.2).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document.id, "tea");
        assert!(hits[0].distance < 0.001);
        assert!(
            store
                .search("a", "model2", &[1., 0.], 3, 2.)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .search("a", "model1", &[1., 0., 0.], 3, 2.)
                .unwrap()
                .is_empty()
        );
        let mut document = hits.into_iter().next().unwrap().document;
        store.put("a", &document).unwrap();
        assert!(store.pending("a", "model1", 8).unwrap().is_empty());
        let stale = Document {
            id: document.id.clone(),
            text: document.text.clone(),
            created_at: 1,
        };
        document.text = "edited fact".into();
        store.put("a", &document).unwrap();
        store
            .set_embedding("a", &stale, "model1", &[1., 0.])
            .unwrap();
        assert_eq!(store.pending("a", "model1", 8).unwrap().len(), 1);
        store
            .set_embedding("a", &document, "model2", &[1., 0.])
            .unwrap();
        store.delete("a", Some("tea")).unwrap();
        assert!(
            store
                .search("a", "model2", &[1., 0.], 3, 2.)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .set_embedding("a", &document, "model2", &[f32::NAN])
                .is_err()
        );
        assert!(store.search("a", "model1", &[0., 0.], 3, 2.).is_err());
        store.prune("a", 0).unwrap();
        assert!(store.pending("a", "new", 8).unwrap().is_empty());
        assert_eq!(
            store.search("b", "model1", &[1., 0.], 3, 2.).unwrap().len(),
            1
        );
        store.delete("b", None).unwrap();
        for id in ["z-earlier", "a-later"] {
            store
                .put(
                    "c",
                    &Document {
                        id: id.into(),
                        text: id.into(),
                        created_at: 4,
                    },
                )
                .unwrap();
        }
        store.prune("c", 1).unwrap();
        assert_eq!(store.pending("c", "", 8).unwrap()[0].id, "a-later");
        drop(store);
        std::fs::write(&path, b"fixture corrupt database").unwrap();
        assert!(VectorStore::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"fixture corrupt database");
        std::fs::remove_dir_all(root).unwrap();
    }
}
