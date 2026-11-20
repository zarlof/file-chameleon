//! Folders and tags: the second `impl` block of [`ClipStore`].

use super::history::{ClipStore, fold};
use super::{Folder, FolderInfo, TagInfo};
use crate::{Error, Result};

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Failed("empty name".into()));
    }
    Ok(name.chars().take(80).collect())
}

impl ClipStore {
    fn has_folder(&self, id: u64) -> bool {
        self.folders.iter().any(|f| f.id == id)
    }

    /// All folders (by name) with the number of live items directly inside.
    pub fn folders(&self) -> Vec<FolderInfo> {
        let mut out: Vec<FolderInfo> = self
            .folders
            .iter()
            .map(|f| FolderInfo {
                id: f.id,
                name: f.name.clone(),
                parent: f.parent,
                count: self
                    .items
                    .iter()
                    .filter(|i| i.trashed_at.is_none() && i.folder == Some(f.id))
                    .count(),
            })
            .collect();
        out.sort_by_cached_key(|f| fold(&f.name));
        out
    }

    pub fn folder_create(&mut self, name: &str, parent: Option<u64>) -> Result<Folder> {
        let name = clean_name(name)?;
        if parent.is_some_and(|p| !self.has_folder(p)) {
            return Err(Error::Failed("no such folder".into()));
        }
        let id = self.folders.iter().map(|f| f.id).max().unwrap_or(0) + 1;
        let folder = Folder { id, name, parent };
        self.folders.push(folder.clone());
        self.save()?;
        Ok(folder)
    }

    pub fn folder_rename(&mut self, id: u64, name: &str) -> Result<()> {
        let name = clean_name(name)?;
        let f = self
            .folders
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| Error::Failed("no such folder".into()))?;
        f.name = name;
        self.save()
    }

    /// Items in the folder become unfiled; its sub-folders move up to its parent.
    pub fn folder_delete(&mut self, id: u64) -> Result<()> {
        let Some(pos) = self.folders.iter().position(|f| f.id == id) else {
            return Err(Error::Failed("no such folder".into()));
        };
        let gone = self.folders.remove(pos);
        for f in self.folders.iter_mut().filter(|f| f.parent == Some(id)) {
            f.parent = gone.parent;
        }
        for i in self.items.iter_mut().filter(|i| i.folder == Some(id)) {
            i.folder = None;
        }
        self.save()
    }

    /// Put items into a folder (`None` = unfiled). Unknown ids are skipped.
    pub fn move_to(&mut self, ids: &[u64], folder: Option<u64>) -> Result<usize> {
        if folder.is_some_and(|f| !self.has_folder(f)) {
            return Err(Error::Failed("no such folder".into()));
        }
        self.update_many(ids, |i| {
            i.folder = folder;
            true
        })
    }

    /// Tags of live items with their counts: most used first, then by name.
    pub fn tags(&self) -> Vec<TagInfo> {
        let mut out: Vec<(String, TagInfo)> = Vec::new();
        for i in self.items.iter().filter(|i| i.trashed_at.is_none()) {
            for t in &i.tags {
                let key = fold(t);
                match out.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, info)) => info.count += 1,
                    None => out.push((
                        key,
                        TagInfo {
                            tag: t.clone(),
                            count: 1,
                        },
                    )),
                }
            }
        }
        out.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));
        out.into_iter().map(|(_, t)| t).collect()
    }

    /// Remove then add tags on items. Tags are trimmed, keep their case, and are unique
    /// ignoring case. Returns how many items changed.
    pub fn tag(&mut self, ids: &[u64], add: &[String], remove: &[String]) -> Result<usize> {
        let remove: Vec<String> = remove.iter().map(|t| fold(t.trim())).collect();
        let mut adds: Vec<String> = Vec::new();
        for t in add.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
            if !adds.iter().any(|a| fold(a) == fold(t)) {
                adds.push(t.chars().take(40).collect());
            }
        }
        self.update_many(ids, |i| {
            let before = i.tags.clone();
            i.tags.retain(|t| !remove.contains(&fold(t)));
            for a in &adds {
                if !i.tags.iter().any(|t| fold(t) == fold(a)) {
                    i.tags.push(a.clone());
                }
            }
            i.tags != before
        })
    }
}
