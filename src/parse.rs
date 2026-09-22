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
    loader: String,
    minecraft_version: String,
    mods: Vec<Item>,
    resourcepacks: Vec<Item>,
    shaders: Vec<Item>,
}

/// A list entry: slug/URL or override table.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Item {
    Slug(String),
    Override(ItemOverrides),
}

/// Overrides for a single item.
#[derive(Debug, Deserialize)]
struct ItemOverrides {
    #[serde(alias = "slug", alias = "url")]
    id: String,
    version: Option<String>,
    minecraft_version: Option<String>,
    loader: Option<String>,
    default_provider: Option<String>,
}

impl Item {
    fn id(&self) -> &str {
        match self {
            Item::Slug(slug) => slug,
            Item::Override(overrides) => &overrides.id,
        }
    }

    fn overrides(&self) -> Option<&ItemOverrides> {
        match self {
            Item::Slug(_) => None,
            Item::Override(overrides) => Some(overrides),
        }
    }

    fn version(&self) -> Option<&str> {
        self.overrides().and_then(|o| o.version.as_deref())
    }

    fn minecraft_version(&self) -> Option<&str> {
        self.overrides().and_then(|o| o.minecraft_version.as_deref())
    }

    fn loader(&self) -> Option<&str> {
        self.overrides().and_then(|o| o.loader.as_deref())
    }

    fn default_provider(&self) -> Option<&str> {
        self.overrides().and_then(|o| o.default_provider.as_deref())
    }

    /// Settings this entry overrides, for the summary.
    fn overridden(&self) -> Vec<&'static str> {
        let Some(o) = self.overrides() else {
            return Vec::new();
        };

        let mut names = Vec::new();

        if o.version.is_some() {
            names.push("version");
        }
        if o.minecraft_version.is_some() {
            names.push("minecraft_version");
        }
        if o.loader.is_some() {
            names.push("loader");
        }
        if o.default_provider.is_some() {
            names.push("default_provider");
        }

        names
    }
}

pub fn parse() -> Vec<DownloadTasks> {
    let toml_text = fs::read_to_string("config.toml").expect("Failed to read config");
    parse_str(&toml_text)
}

fn parse_str(toml_text: &str) -> Vec<DownloadTasks> {
    let config: Config = toml::from_str(toml_text).expect("Failed to parse config");

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
            let default_provider = item.default_provider().unwrap_or(&s.default_provider);
            let (provider, identifier) = pair(default_provider, item.id());

            if seen.insert((provider.clone(), label.to_string(), identifier.clone())) {
                tasks.push(DownloadTasks {
                    provider,
                    loader: item.loader().unwrap_or(&s.loader).to_string(),
                    mc_ver: item.minecraft_version().unwrap_or(&s.minecraft_version).to_string(),
                    version: item.version().map(str::to_string),
                    overrides: item.overridden(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config(mods: &str) -> String {
        format!(
            r#"
[settings]
mods_dir = "mods"
resourcepacks_dir = "resourcepacks"
shaderpacks_dir = "shaderpacks"
default_provider = "modrinth"
loader = "fabric"
minecraft_version = "1.21.1"

mods = [{}]

resourcepacks = ["fresh-animations"]
shaders = []
"#,
            mods
        )
    }

    fn mods(items: &str) -> Vec<DownloadTasks> {
        parse_str(&config(items))
            .into_iter()
            .filter(|task| task.label == "mods")
            .collect()
    }

    #[test]
    fn plain_entry_uses_settings() {
        let tasks = mods(r#""sodium""#);

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].provider, "modrinth");
        assert_eq!(tasks[0].identifier, "sodium");
        assert_eq!(tasks[0].loader, "fabric");
        assert_eq!(tasks[0].mc_ver, "1.21.1");
        assert_eq!(tasks[0].version, None);
        assert!(tasks[0].overrides.is_empty());
    }

    #[test]
    fn table_entry_overrides_settings() {
        let tasks = mods(
            r#"{ id = "iris", version = "1.8.8", minecraft_version = "1.20.1", loader = "neoforge" }"#,
        );

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].identifier, "iris");
        assert_eq!(tasks[0].version.as_deref(), Some("1.8.8"));
        assert_eq!(tasks[0].mc_ver, "1.20.1");
        assert_eq!(tasks[0].loader, "neoforge");
        assert_eq!(tasks[0].overrides, vec!["version", "minecraft_version", "loader"]);
    }

    #[test]
    fn partial_table_keeps_remaining_settings() {
        let tasks = mods(r#"{ id = "jei", loader = "neoforge" }"#);

        assert_eq!(tasks[0].loader, "neoforge");
        assert_eq!(tasks[0].mc_ver, "1.21.1");
        assert_eq!(tasks[0].version, None);
        assert_eq!(tasks[0].overrides, vec!["loader"]);
    }

    #[test]
    fn id_accepts_slug_and_url_aliases() {
        let tasks = mods(
            r#"{ slug = "Curseforge:jei" }, { url = "https://modrinth.com/mod/sodium" }"#,
        );

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].provider, "curseforge");
        assert_eq!(tasks[0].identifier, "jei");
        assert_eq!(tasks[1].provider, "modrinth");
        assert_eq!(tasks[1].identifier, "sodium");
    }

    #[test]
    fn default_provider_can_be_overridden() {
        let tasks = mods(r#"{ id = "jei", default_provider = "curseforge" }"#);

        assert_eq!(tasks[0].provider, "curseforge");
        assert_eq!(tasks[0].overrides, vec!["default_provider"]);
    }

    #[test]
    fn duplicate_entries_keep_the_first() {
        let tasks = mods(r#""sodium", { id = "sodium", loader = "neoforge" }"#);

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].loader, "fabric");
    }

    #[test]
    fn resourcepacks_get_no_loader() {
        let tasks = parse_str(&config(r#""sodium""#));

        let resourcepack = tasks.iter().find(|task| task.label == "resourcepacks").unwrap();

        assert_eq!(resourcepack.identifier, "fresh-animations");
        assert_eq!(resourcepack.target_dir, "resourcepacks");
    }

    #[test]
    fn multi_line_table_entries_are_supported() {
        let tasks = mods(
            r#"
    { id = "iris",
      minecraft_version = "1.20.1",
      loader = "neoforge", },
"#,
        );

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].identifier, "iris");
        assert_eq!(tasks[0].mc_ver, "1.20.1");
        assert_eq!(tasks[0].loader, "neoforge");
        assert_eq!(tasks[0].overrides, vec!["minecraft_version", "loader"]);
    }
}



