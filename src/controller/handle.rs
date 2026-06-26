use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum ControllerCommand {
    /// Stop accepting connections and begin shutdown procedure.
    Stop {
        /// True if shut down should be graceful.
        graceful: bool,
    },
}

/// Server handle.
#[derive(Debug, Clone)]
pub struct ControllerHandle {
    pub cmd_tx: UnboundedSender<ControllerCommand>,
}

impl ControllerHandle {
    pub fn new(cmd_tx: UnboundedSender<ControllerCommand>) -> Self {
        ControllerHandle { cmd_tx }
    }

    /// Stop incoming connection processing, stop all workers and exit.
    pub async fn stop(&self, graceful: bool) {
        let _ = self.cmd_tx.send(ControllerCommand::Stop { graceful });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_controller_handle_new() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);
        let _debug_str = format!("{:?}", handle);
    }

    #[tokio::test]
    async fn test_controller_handle_stop_graceful() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);

        handle.stop(true).await;

        let cmd = rx.recv().await.expect("Should receive command");
        match cmd {
            ControllerCommand::Stop { graceful } => assert!(graceful),
        }
    }

    #[tokio::test]
    async fn test_controller_handle_stop_non_graceful() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);

        handle.stop(false).await;

        let cmd = rx.recv().await.expect("Should receive command");
        match cmd {
            ControllerCommand::Stop { graceful } => assert!(!graceful),
        }
    }

    #[test]
    fn test_controller_command_stop_enum() {
        let stop_graceful = ControllerCommand::Stop { graceful: true };
        let stop_non_graceful = ControllerCommand::Stop { graceful: false };

        matches!(stop_graceful, ControllerCommand::Stop { graceful: true });
        matches!(
            stop_non_graceful,
            ControllerCommand::Stop { graceful: false }
        );
    }

    #[test]
    fn test_controller_handle_debug_impl() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);
        let _debug_str = format!("{:?}", handle);
    }
}
