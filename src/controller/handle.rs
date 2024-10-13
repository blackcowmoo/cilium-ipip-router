use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub(crate) enum ControllerCommand {
    /// Stop accepting connections and begin shutdown procedure.
    Stop {
        /// True if shut down should be graceful.
        graceful: bool,
    },
}

/// Server handle.
#[derive(Debug, Clone)]
pub struct ControllerHandle {
    cmd_tx: UnboundedSender<ControllerCommand>,
}

impl ControllerHandle {
    pub(crate) fn new(cmd_tx: UnboundedSender<ControllerCommand>) -> Self {
        ControllerHandle { cmd_tx }
    }

    /// Stop incoming connection processing, stop all workers and exit.
    pub async fn stop(&self, graceful: bool) {
        let _ = self.cmd_tx.send(ControllerCommand::Stop { graceful });
    }
}
