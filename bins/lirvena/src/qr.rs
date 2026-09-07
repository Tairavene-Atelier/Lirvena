mod ceylith;
mod continuity;
mod credential;
mod daemon;
mod flow;
mod polling;
mod qq;

use crate::config::ProcessConfig;

pub(super) async fn run(config: ProcessConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut next = config;
    loop {
        match daemon::run(next).await? {
            daemon::DaemonOutcome::Stopped => return Ok(()),
            daemon::DaemonOutcome::Restart => {
                eprintln!("Lirvena completed a graceful restart cycle");
                next = ProcessConfig::from_environment()?;
            }
        }
    }
}
