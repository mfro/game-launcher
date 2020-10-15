use std::{fs::DirEntry, fs::ReadDir, path::Path};

pub struct RecursiveSearch {
    stack: Vec<ReadDir>,
}

impl RecursiveSearch {
    pub fn new<P: AsRef<Path>>(path: P) -> RecursiveSearch {
        let stack = match std::fs::read_dir(path) {
            Ok(iter) => vec![iter],
            Err(_) => vec![],
        };

        RecursiveSearch { stack }
    }
}

impl Iterator for RecursiveSearch {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let iter = match self.stack.last_mut() {
                Some(iter) => iter,
                None => break None,
            };

            let entry = match iter.next() {
                Some(result) => match result {
                    Err(_) => continue,
                    Ok(entry) => entry,
                },
                None => {
                    self.stack.pop();
                    continue;
                }
            };

            let ty = match entry.file_type() {
                Ok(ty) => ty,
                Err(_) => continue,
            };

            if ty.is_file() {
                break Some(entry);
            }

            if ty.is_dir() {
                match std::fs::read_dir(&entry.path()) {
                    Ok(iter) => {
                        self.stack.push(iter);
                        continue;
                    }
                    Err(_) => continue,
                }
            };
        }
    }
}
