//! Revim: a terminal text editor with Vim motions. Sets up logging, then runs the editor
//! with the modules from `conf.rs`.

use std::{path::PathBuf, process::ExitCode};
mod conf;
mod logger;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let _logs = logger::init();
    let path = std::env::args().nth(1).map(PathBuf::from);
    match revim_runtime::run(path, conf::modules()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            ExitCode::FAILURE
        }
    }
}
