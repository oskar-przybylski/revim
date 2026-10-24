mod conf;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    revim_runtime::run().await
}
