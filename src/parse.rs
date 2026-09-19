use crate::download::DownloadTasks;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct Config {
    settings: Settings,
}

#[derive(Debug, Deserialize)]
struct Settings {
    mods_dir: String,
    resourcepacks_dir: String,
    shaderpacks_dir: String,
    default_provider: String,
    mods: Vec<String>,
    resourcepacks: Vec<String>,
    shaders: Vec<String>,
}

pub fn parse() -> Vec<DownloadTasks> {
    let toml_text = fs::read_to_string("config.toml").expect("Failed to read config");
    let config: Config = toml::from_str(&toml_text).expect("Failed to parse config");

    let s = config.settings;
    let categories = [
        ("mods", &s.mods_dir, &s.mods),
        ("resourcepacks", &s.resourcepacks_dir, &s.resourcepacks),
        ("shaders", &s.shaderpacks_dir, &s.shaders)
    ];

    let mut tasks = Vec::new();
    let mut seen = HashSet::new();

    for (label, dir, list) in categories {
        for item in list {
            let (provider, identifier) = pair(&s.default_provider, item);

            if seen.insert((provider.clone(), label.to_string(), identifier.clone())) {
                tasks.push(DownloadTasks {
                    provider,
                    label: label.to_string(),
                    identifier,
                    target_dir: dir.to_string(),
                });
            }
        }
    }
    tasks
}

fn pair(default_provider: &str, item: &str) -> (String, String) {
    let item = item.trim_end_matches('/').to_lowercase();

    if item.contains("modrinth.com") {
        let identifier = item.split('/').last().unwrap_or("").to_string();
        ("modrinth".to_string(), identifier)
    } else if item.contains("curseforge.com") {
        let identifier = item.split('/').last().unwrap_or("").to_string();
        ("curseforge".to_string(), identifier)
    } else if let Some((provider, identifier)) = item.split_once(':') {
        (provider.to_string(), identifier.to_string())
    } else {
        (default_provider.to_lowercase(), item.to_string())
    }
}



