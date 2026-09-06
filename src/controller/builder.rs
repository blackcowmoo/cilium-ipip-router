use super::handle::ControllerCommand;
/// [Server] builder.
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub struct ControllerBuilder {
    pub cmd_tx: UnboundedSender<ControllerCommand>,
    pub cmd_rx: UnboundedReceiver<ControllerCommand>,
}

impl ControllerBuilder {
    /// Create new Server builder instance
    pub fn new() -> ControllerBuilder {
        let (cmd_tx, cmd_rx) = unbounded_channel();
        ControllerBuilder { cmd_tx, cmd_rx }
    }
}

impl Default for ControllerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_builder_default() {
        let builder = ControllerBuilder::default();
        assert!(!builder.cmd_tx.is_closed());
        assert_eq!(builder.cmd_rx.len(), 0);
    }

    #[test]
    fn test_controller_builder_new() {
        let builder = ControllerBuilder::new();
        assert!(!builder.cmd_tx.is_closed());
        assert_eq!(builder.cmd_rx.len(), 0);
    }

    #[test]
    fn test_controller_builder_clone_channel() {
        let builder = ControllerBuilder::new();
        let tx = builder.cmd_tx.clone();
        let _rx = builder.cmd_rx;
        assert!(!tx.is_closed());
    }

    #[tokio::test]
    async fn test_controller_builder_multiple_commands() {
        let builder = ControllerBuilder::new();

        let cmd1 = ControllerCommand::Stop { graceful: true };
        let cmd2 = ControllerCommand::Stop { graceful: false };

        assert!(builder.cmd_tx.send(cmd1).is_ok());
        assert!(builder.cmd_tx.send(cmd2).is_ok());

        assert_eq!(builder.cmd_rx.len(), 2);
    }
}
