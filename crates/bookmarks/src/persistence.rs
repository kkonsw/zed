use std::path::PathBuf;

use db::{define_connection, query, sqlez_macros::sql};
use workspace::{WorkspaceDb, WorkspaceId};

define_connection! {
    // Current schema shape using pseudo-rust syntax:
    // bookmarks (
    //   bookmark_id: usize, primary key
    //   workspace_id: usize,
    //   label: String,
    //   project_path: PathBuf,
    // )
    pub static ref BOOKMARKS_DB: BookmarksDb<WorkspaceDb> =
        &[sql! (
            CREATE TABLE bookmarks(
                bookmark_id INTEGER PRIMARY KEY,
                workspace_id INTEGER NOT NULL,
                project_path BLOB NOT NULL,
                label TEXT,
                FOREIGN KEY(workspace_id) REFERENCES workspaces(workspace_id)
                ON DELETE CASCADE
                ON UPDATE CASCADE
            ) STRICT;
        ),
        ];
}

impl BookmarksDb {
    query! {
        fn bookmarks(id: WorkspaceId) -> Result<Vec<(String, PathBuf)>> {
            SELECT label, project_path
            FROM bookmarks
            WHERE workspace_id IS ?
        }
    }

    query! {
        pub async fn save_bookmark(workspace_id: WorkspaceId, label: String, path: PathBuf) -> Result<()> {
            INSERT INTO bookmarks
                (workspace_id, label, path)
            VALUES
                (?1, ?2, ?3)
        }
    }
}
