use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use lapon_common::{action::ActionMessage, node::NodeMessage};
use lapon_node::node;

pub fn start_local() -> (Sender<NodeMessage>, Receiver<ActionMessage>) {
    let (writer_tx, writer_rx) = crossbeam_channel::unbounded::<NodeMessage>();
    let (reader_tx, reader_rx) = crossbeam_channel::unbounded::<ActionMessage>();

    std::thread::spawn(move || -> Result<()> {
        node::mainloop(writer_rx, reader_tx)?;
        Ok(())
    });

    (writer_tx, reader_rx)
}
