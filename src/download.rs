use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use tokio::fs;

#[derive(Debug)]
pub struct DownloadTasks {
    pub provider: String,
    pub loader: String,
    pub mc_ver: String,
    /// Pinned version, when the config sets one.
    pub version: Option<String>,
    /// Overridden settings, for the summary.
    pub overrides: Vec<&'static str>,
    pub label: String,
    pub identifier: String,
    pub target_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthVersion {
    id: String,
    version_number: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    primary: bool,
}

pub async fn run(tasks: &[DownloadTasks]) {
    let client = create_client();

    let handles: Vec<_> = tasks
        .iter()
        .map(|task| {
            let client = client.clone();
            async move {
                let _ = fs::create_dir_all(&task.target_dir).await;
                match task.provider.as_str() {
                    "modrinth" => {
                        process_modrinth(&client, &task).await
                    }
                    "curseforge" => {
                        process_curseforge(&client, &task.label, &task.identifier, &task.target_dir).await
                    }
                    _ => eprintln!("Unknown provider: {}", task.provider),
                }
            }
        })
        .collect();

    futures::future::join_all(handles).await;
}

async fn process_modrinth(client: &Client, task: &DownloadTasks) {
    let loader = &task.loader;
    let mc_ver = &task.mc_ver;
    let label = &task.label;
    let identifier = &task.identifier;
    let dir = &task.target_dir;

    let api_url = match label.as_str() {
        "mods" => format!(
            "https://api.modrinth.com/v2/project/{}/version?game_versions=[\"{}\"]&loaders=[\"{}\"]",
            identifier, mc_ver, loader
        ),
        _ => format!(
            "https://api.modrinth.com/v2/project/{}/version?game_versions=[\"{}\"]",
            identifier, mc_ver
        )
    };

    let Some(versions) = fetch_versions(client, &api_url, identifier).await else {
        return;
    };

    // Pinned version, otherwise the newest match.
    let version = match task.version.as_deref() {
        Some(wanted) => match pick(&versions, wanted) {
            Some(found) => Some(found.clone()),
            // Pins can sit outside the filters, so look the version up directly.
            None => match fetch_version_number(client, identifier, wanted).await {
                Some(found) => {
                    warn_about_filters(task, &found);
                    Some(found)
                }
                None => {
                    eprintln!("[Modrinth] {} has no version matching {}", identifier, wanted);
                    return;
                }
            },
        },
        None => versions.first().cloned(),
    };

    let Some(version) = version else {
        eprintln!("[Modrinth] No versions found for {}", identifier);
        return;
    };

    let file = version.files.iter().find(|f| f.primary).or_else(|| version.files.first());

    let Some(f) = file else {
        eprintln!("[Modrinth] {} {} has no files", identifier, version.version_number);
        return;
    };

    let dest_path = Path::new(dir).join(&f.filename);

    if dest_path.exists() {
        println!("[Modrinth] {} already exists", f.filename);
        return;
    }

    println!("[Modrinth] Downloading {} ({})", f.filename, version.version_number);

    if let Ok(download_res) = client.get(&f.url).send().await {
        if download_res.status().is_success() {
            if let Ok(bytes) = download_res.bytes().await {
                if fs::write(&dest_path, bytes).await.is_ok() {
                    println!("[Modrinth] saved to {}", dest_path.display());
                }
            }
        }
    }
}

async fn fetch_versions(client: &Client, api_url: &str, identifier: &str) -> Option<Vec<ModrinthVersion>> {
    let response = match client.get(api_url).send().await {
        Ok(res) if res.status().is_success() => res,
        Ok(res) => {
            eprintln!("[Modrinth] Failed {}: {}", identifier, res.status());
            return None;
        }
        Err(e) => {
            eprintln!("[Modrinth] Network error for {}: {}", identifier, e);
            return None;
        }
    };

    match response.json().await {
        Ok(versions) => Some(versions),
        Err(e) => {
            eprintln!("[Modrinth] JSON parse error for {}: {}", identifier, e);
            None
        }
    }
}

/// Fetch one version by number, without the filter query.
async fn fetch_version_number(client: &Client, identifier: &str, wanted: &str) -> Option<ModrinthVersion> {
    let url = format!("https://api.modrinth.com/v2/project/{}/version/{}", identifier, wanted);

    let response = match client.get(&url).send().await {
        Ok(res) if res.status().is_success() => res,
        // No such pin; the caller reports it.
        Ok(_) => return None,
        Err(e) => {
            eprintln!("[Modrinth] Network error for {} {}: {}", identifier, wanted, e);
            return None;
        }
    };

    response.json().await.ok()
}

/// Match a pin against version id, version number, or file name.
/// Versions come back newest first, so the first match wins.
fn pick<'a>(versions: &'a [ModrinthVersion], wanted: &str) -> Option<&'a ModrinthVersion> {
    let wanted = wanted.to_lowercase();

    let exact = |version: &ModrinthVersion| {
        version.id.to_lowercase() == wanted
            || version.version_number.to_lowercase() == wanted
            || version.files.iter().any(|f| f.filename.to_lowercase() == wanted)
    };

    let partial = |version: &ModrinthVersion| {
        version.version_number.to_lowercase().contains(&wanted)
            || version.files.iter().any(|f| f.filename.to_lowercase().contains(&wanted))
    };

    versions.iter().find(|v| exact(v)).or_else(|| versions.iter().find(|v| partial(v)))
}

/// Warn when a pinned version targets another loader or game version.
fn warn_about_filters(task: &DownloadTasks, version: &ModrinthVersion) {
    if task.label == "mods" && !version.loaders.iter().any(|l| l.eq_ignore_ascii_case(&task.loader)) {
        eprintln!(
            "[Modrinth] Warning: {} {} is for loaders {:?}, not {}",
            task.identifier, version.version_number, version.loaders, task.loader
        );
    }

    if !version.game_versions.iter().any(|g| g == &task.mc_ver) {
        eprintln!(
            "[Modrinth] Warning: {} {} targets game versions {:?}, not {}",
            task.identifier, version.version_number, version.game_versions, task.mc_ver
        );
    }
}

async fn process_curseforge(_client: &Client, _label: &str, _identifier: &str, _dir: &str) {}

fn create_client() -> Client {
    Client::builder()
        .user_agent("Strikey5852/blazers/0.1.0 (https://github.com/Strikey5852/blazers)")
        .build().unwrap_or_default()
}