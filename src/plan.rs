use crate::download::DownloadTasks;
use std::io::{self, Write};

/// Print the resolved config.
pub fn print_plan(tasks: &[DownloadTasks]) {
    println!("Resolved {} items from config.toml\n", tasks.len());

    let targets: Vec<String> = tasks
        .iter()
        .map(|task| format!("{}/{}", task.target_dir, task.identifier))
        .collect();

    let versions: Vec<&str> = tasks
        .iter()
        .map(|task| task.version.as_deref().unwrap_or("latest"))
        .collect();

    let target_width = targets.iter().map(String::len).max().unwrap_or(0);
    let version_width = versions.iter().map(|version| version.len()).max().unwrap_or(0);

    for ((task, target), version) in tasks.iter().zip(&targets).zip(&versions) {
        println!("{}", row(task, target, version, target_width, version_width));
    }
}

/// One line per item: target, provider, loader, MC version, version, overrides.
fn row(task: &DownloadTasks, target: &str, version: &str, target_width: usize, version_width: usize) -> String {
    // Only mods are filtered by loader.
    let loader = if task.label == "mods" {
        task.loader.as_str()
    } else {
        "-"
    };

    let mut line = format!(
        "  {:<target_width$}  {:<10}  {:<9}  mc {:<8}  {:<version_width$}",
        target, task.provider, loader, task.mc_ver, version,
    );

    if !task.overrides.is_empty() {
        line.push_str(&format!("  overrides: {}", task.overrides.join(", ")));
    }

    line.trim_end().to_string()
}

/// y/N prompt.
pub fn confirm() -> bool {
    print!("\nProceed with download? [y/N] ");
    let _ = io::stdout().flush();

    let mut answer = String::new();

    match io::stdin().read_line(&mut answer) {
        // Empty stdin counts as "no".
        Ok(0) => false,
        Ok(_) => is_yes(&answer),
        Err(e) => {
            eprintln!("Could not read confirmation: {}", e);
            false
        }
    }
}

fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(label: &str, version: Option<&str>, overrides: Vec<&'static str>) -> DownloadTasks {
        DownloadTasks {
            provider: "modrinth".to_string(),
            loader: "fabric".to_string(),
            mc_ver: "1.21.1".to_string(),
            version: version.map(str::to_string),
            overrides,
            label: label.to_string(),
            identifier: "iris".to_string(),
            target_dir: "mods".to_string(),
        }
    }

    #[test]
    fn only_y_and_yes_confirm() {
        assert!(is_yes("y"));
        assert!(is_yes("Y"));
        assert!(is_yes(" yes\n"));
        assert!(!is_yes(""));
        assert!(!is_yes("\n"));
        assert!(!is_yes("n"));
        assert!(!is_yes("yep"));
    }

    #[test]
    fn row_notes_overridden_settings() {
        let overridden = task("mods", Some("1.8.8"), vec!["version", "loader"]);
        let plain = task("mods", None, vec![]);

        assert_eq!(
            row(&overridden, "mods/iris", "1.8.8", 9, 6),
            "  mods/iris  modrinth    fabric     mc 1.21.1    1.8.8   overrides: version, loader"
        );
        assert_eq!(
            row(&plain, "mods/iris", "latest", 9, 6),
            "  mods/iris  modrinth    fabric     mc 1.21.1    latest"
        );
    }

    #[test]
    fn row_hides_loader_for_packs() {
        let pack = task("resourcepacks", None, vec![]);
        let line = row(&pack, "resourcepacks/iris", "latest", 18, 6);

        assert!(line.starts_with("  resourcepacks/iris  modrinth    -"), "actual: {line:?}");
    }
}
