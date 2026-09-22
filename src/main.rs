mod download;
mod parse;
mod plan;

use std::env;

#[tokio::main]
async fn main() {
    // -y skips the confirmation prompt.
    let assume_yes = env::args().skip(1).any(|arg| arg == "-y" || arg == "--yes");

    let tasks = parse::parse();

    if tasks.is_empty() {
        println!("config.toml lists no items.");
        return;
    }

    plan::print_plan(&tasks);

    if !assume_yes && !plan::confirm() {
        println!("\nAborted, nothing downloaded (pass -y to skip the prompt).");
        return;
    }

    download::run(&tasks).await;
}
