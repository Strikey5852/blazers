mod parse;
mod download;

#[tokio::main]
async fn main() {
    let tasks = parse::parse();
    println!("Generated {} tasks", tasks.len());
    download::run(&tasks).await;
}