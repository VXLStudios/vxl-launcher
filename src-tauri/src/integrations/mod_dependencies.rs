use crate::integrations::mod_lookup::project_titles;
use crate::integrations::unified_mod::{
    get_mod_versions_unified, ModPlatform, UnifiedDependency, UnifiedDependencyType,
    UnifiedModVersionsParams, UnifiedVersion,
};
use futures::future::join_all;
use log::{error, info, warn};
use std::collections::{HashSet, VecDeque};

pub const DEPENDENCY_DEPTH: u8 = 20;

pub struct DependencyTarget {
    pub loader: String,
    pub game_version: String,
}

pub struct ResolvedDependency {
    pub project_id: String,
    pub version: UnifiedVersion,
    pub title: Option<String>,
}

impl ResolvedDependency {
    pub fn display_name(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| self.version.name.clone())
    }
}

pub fn pick_version<'a>(
    versions: &'a [UnifiedVersion],
    pinned_version_id: Option<&String>,
    game_version: &str,
    parent_date: &str,
) -> Option<&'a UnifiedVersion> {
    pinned_version_id
        .and_then(|id| versions.iter().find(|version| &version.id == id))
        .or_else(|| {
            versions
                .iter()
                .filter(|version| {
                    version.game_versions.iter().any(|v| v == game_version)
                        && version.date_published.as_str() <= parent_date
                })
                .max_by(|a, b| a.date_published.cmp(&b.date_published))
        })
        .or_else(|| {
            versions
                .iter()
                .filter(|version| version.game_versions.iter().any(|v| v == game_version))
                .max_by(|a, b| a.date_published.cmp(&b.date_published))
        })
        .or_else(|| {
            versions
                .iter()
                .max_by(|a, b| a.date_published.cmp(&b.date_published))
        })
}

pub async fn resolve_required_dependencies_seen(
    platform: &ModPlatform,
    dependencies: &[UnifiedDependency],
    parent_date: &str,
    target: &DependencyTarget,
    max_depth: u8,
    seen: &mut HashSet<String>,
) -> Vec<ResolvedDependency> {
    let mut resolved = Vec::new();
    let mut queue: VecDeque<(Vec<UnifiedDependency>, String, u8)> = VecDeque::new();
    queue.push_back((dependencies.to_vec(), parent_date.to_string(), max_depth));

    while let Some((batch, batch_parent_date, depth)) = queue.pop_front() {
        if depth == 0 {
            warn!("Dependency chain too deep, stopping here");
            continue;
        }

        let mut pending = Vec::new();
        for dependency in batch {
            if dependency.dependency_type != UnifiedDependencyType::Required {
                continue;
            }

            let Some(project_id) = dependency.project_id.clone() else {
                continue;
            };

            if !seen.insert(format!("{:?}:{}", platform, project_id)) {
                continue;
            }

            pending.push(dependency);
        }

        let fetch_jobs = pending.iter().map(|dependency| {
            let platform = platform.clone();
            let project_id = dependency.project_id.clone().unwrap();
            let loader = target.loader.clone();
            let game_version = target.game_version.clone();
            async move {
                let params = UnifiedModVersionsParams {
                    source: platform,
                    project_id: project_id.clone(),
                    loaders: Some(vec![loader]),
                    game_versions: Some(vec![game_version]),
                    limit: None,
                    offset: None,
                };
                let result = get_mod_versions_unified(params).await;
                (project_id, result)
            }
        });

        let fetch_results = join_all(fetch_jobs).await;

        for (dependency, (project_id, versions_result)) in pending.into_iter().zip(fetch_results) {
            let versions = match versions_result {
                Ok(response) => response.versions,
                Err(e) => {
                    error!("Failed to get versions for dependency '{}': {}", project_id, e);
                    continue;
                }
            };

            let Some(chosen) = pick_version(
                &versions,
                dependency.version_id.as_ref(),
                &target.game_version,
                &batch_parent_date,
            ) else {
                warn!("No compatible version found for dependency '{}'", project_id);
                continue;
            };

            info!(
                "[dep-resolve] {} pin={:?} parent_date={} candidates={} -> chosen={:?}",
                project_id,
                dependency.version_id,
                batch_parent_date,
                versions.len(),
                chosen.version_number
            );

            if !chosen.dependencies.is_empty() {
                queue.push_back((
                    chosen.dependencies.clone(),
                    chosen.date_published.clone(),
                    depth - 1,
                ));
            }

            resolved.push(ResolvedDependency {
                project_id,
                version: chosen.clone(),
                title: None,
            });
        }
    }

    let ids: Vec<String> = resolved.iter().map(|d| d.project_id.clone()).collect();
    let titles = match platform {
        ModPlatform::Modrinth => project_titles(ids, Vec::new()).await,
        ModPlatform::CurseForge => {
            project_titles(Vec::new(), ids.iter().filter_map(|id| id.parse().ok()).collect())
                .await
        }
    };
    for dependency in &mut resolved {
        dependency.title = titles.get(&dependency.project_id).cloned();
    }

    resolved
}
