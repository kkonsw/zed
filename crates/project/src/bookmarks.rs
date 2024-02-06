use crate::ProjectPath;
use std::path::PathBuf;
use text::Point;

#[derive(Clone)]
pub struct Bookmark {
    label: String,
    project_path: ProjectPath,
    abs_path: PathBuf,
    point: Point,
}

impl Bookmark {
    pub fn new(label: &str, project_path: ProjectPath, abs_path: PathBuf, point: Point) -> Self {
        Self {
            label: String::from(label),
            project_path,
            abs_path,
            point,
        }
    }

    pub fn label(&self) -> &String {
        &self.label
    }

    pub fn abs_path(&self) -> &PathBuf {
        &self.abs_path
    }

    pub fn project_path(&self) -> &ProjectPath {
        &self.project_path
    }

    pub fn point(&self) -> Point {
        self.point
    }
}
