use crate::integrations::{curseforge, modrinth};
use log::debug;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub const MODRINTH_HASH_BATCH: usize = 200;
pub const CURSEFORGE_FILE_BATCH: usize = 200;

pub fn chunk_unique<T: Eq + Hash + Clone>(values: impl IntoIterator<Item = T>, size: usize) -> Vec<Vec<T>> {
    let mut seen = HashSet::new();
    let unique: Vec<T> = values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect();

    unique
        .chunks(size.max(1))
        .map(|chunk| chunk.to_vec())
        .collect()
}

pub async fn project_titles(
    modrinth_ids: Vec<String>,
    curseforge_ids: Vec<u32>,
) -> HashMap<String, String> {
    let mut titles: HashMap<String, String> = HashMap::new();

    for chunk in chunk_unique(modrinth_ids, MODRINTH_HASH_BATCH) {
        match modrinth::get_multiple_projects(chunk).await {
            Ok(projects) => titles.extend(
                projects
                    .into_iter()
                    .map(|project| (project.id, project.title)),
            ),
            Err(e) => debug!("Could not fetch Modrinth project titles: {}", e),
        }
    }

    for chunk in chunk_unique(curseforge_ids, CURSEFORGE_FILE_BATCH) {
        match curseforge::get_mods_by_ids(chunk, Some(true)).await {
            Ok(response) => titles.extend(
                response
                    .data
                    .into_iter()
                    .map(|entry| (entry.id.to_string(), entry.name)),
            ),
            Err(e) => debug!("Could not fetch CurseForge project titles: {}", e),
        }
    }

    titles
}
