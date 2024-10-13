use super::handle::ControllerCommand;
/// [Server] builder.
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub struct ControllerBuilder {
    pub(crate) cmd_tx: UnboundedSender<ControllerCommand>,
    pub(crate) cmd_rx: UnboundedReceiver<ControllerCommand>,
}

impl Default for ControllerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerBuilder {
    /// Create new Server builder instance
    pub fn new() -> ControllerBuilder {
        let (cmd_tx, cmd_rx) = unbounded_channel();

        ControllerBuilder { cmd_tx, cmd_rx }
    }
}
