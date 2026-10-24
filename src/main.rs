use std::path::PathBuf;
mod conf;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).map(PathBuf::from);
    revim_runtime::run(path).await
}
