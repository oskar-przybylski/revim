use std::{path::PathBuf, process::ExitCode};
mod conf;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let path = std::env::args().nth(1).map(PathBuf::from);
    match revim_runtime::run(path, conf::modules()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Display, not Debug: returning `Err` from `main` would print `Custom { .. }`
            // with escaped newlines instead of the message.
            eprintln!("revim: {e}");
            ExitCode::FAILURE
        }
    }
}
