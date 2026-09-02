#![forbid(unsafe_code)]
//! Lirvena command-line application.

#[cfg(target_os = "linux")]
mod action_runtime;
mod cli;
#[cfg(any(target_os = "linux", test))]
#[cfg_attr(all(test, not(target_os = "linux")), allow(dead_code))]
mod config;
mod notification;
#[cfg(target_os = "linux")]
mod online;
#[cfg(target_os = "linux")]
mod qq;
#[cfg(target_os = "linux")]
mod qr;
#[cfg(target_os = "linux")]
mod support;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match cli::parse(std::env::args().skip(1))? {
        cli::Command::Run => {
            #[cfg(target_os = "linux")]
            {
                run().await
            }
            #[cfg(not(target_os = "linux"))]
            {
                run();
                Ok(())
            }
        }
        cli::Command::NotifyTest(selection) => {
            let state_directory = std::env::var_os("LIRVENA_STATE_DIRECTORY")
                .map_or_else(|| std::path::PathBuf::from(".lirvena-state"), Into::into);
            notification::test(&state_directory, selection).await
        }
    }
}

#[cfg(target_os = "linux")]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("LIRVENA_CEYLITH_ADDRESS").is_some() {
        qr::run(config::ProcessConfig::from_environment()?).await
    } else {
        println!("Lirvena {}", env!("CARGO_PKG_VERSION"));
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
fn run() {
    println!("Lirvena {}", env!("CARGO_PKG_VERSION"));
}
