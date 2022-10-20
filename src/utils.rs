use std::{env, path::PathBuf};
use walkdir::WalkDir;

pub fn find_file(file: String) -> Option<PathBuf> {
    let current_dir = env::current_dir().unwrap();
    for entry in WalkDir::new(current_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let f_name = entry.file_name().to_string_lossy();

        if f_name == file {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}
