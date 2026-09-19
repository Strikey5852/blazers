use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use tokio::fs;

#[derive(Debug)]
pub struct DownloadTasks {
    pub provider: String,
    pub label: String,
    pub identifier: String,
    pub target_dir: String,
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
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
                        process_modrinth(&client, &task.label, &task.identifier, &task.target_dir).await
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

async fn process_modrinth(client: &Client, label: &str, identifier: &str, dir: &str) {
    // Hardcoded for testing
    let mc_version = "1.21.1";
    let loader = "fabric";

    let api_url = match label {
        "mods" => format!(
            "https://api.modrinth.com/v2/project/{}/version?game_versions=[\"{}\"]&loaders=[\"{}\"]",
            identifier, mc_version, loader
        ),
        _ => format!(
            "https://api.modrinth.com/v2/project/{}/version?game_versions=[\"{}\"]",
            identifier, mc_version
        )
    };

    let response = match client.get(&api_url).send().await {
        Ok(res) if res.status().is_success() => res,
        Ok(res) => {
            eprintln!("[Modrinth] Failed {}: {}", identifier, res.status());
            return;
        }
        Err(e) => {
            eprintln!("[Modrinth] Network error for {}: {}", identifier, e);
            return;
        }
    };

    let versions: Vec<ModrinthVersion> = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[Modrinth] JSON parse error for {}: {}]", identifier, e);
            return;
        }
    };

    if let Some(latest) = versions.first() {
        let file = latest.files.iter().find(|f| f.primary).or_else(|| latest.files.first());

        if let Some(f) = file {
            let dest_path = Path::new(dir).join(&f.filename);

            if dest_path.exists() {
                println!("[Modrinth] {} already exists", f.filename);
                return;
            }

            println!("[Modrinth] Downloading {}", f.filename);

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
    } else {
        eprintln!("[Modrinth] No versions found for {}", identifier);
    }
}

async fn process_curseforge(_client: &Client, _label: &str, _identifier: &str, _dir: &str) {}

fn create_client() -> Client {
    Client::builder()
        .user_agent("Strikey5852/blazers/0.1.0 (https://github.com/Strikey5852/blazers)")
        .build().unwrap_or_default()
}