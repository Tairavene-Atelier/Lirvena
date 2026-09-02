mod ceylith;
mod continuity;
mod credential;
mod daemon;
mod flow;
mod polling;
mod qq;

use crate::config::ProcessConfig;

pub(super) async fn run(config: ProcessConfig) -> Result<(), Box<dyn std::error::Error>> {
    daemon::run(config).await
}
