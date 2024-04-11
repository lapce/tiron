use std::io::{BufRead, Write};

use anyhow::Result;
use crossbeam_channel::Receiver;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

pub fn stdio_transport<W, R, RpcMessage1, RpcMessage2>(
    mut writer: W,
    writer_receiver: Receiver<RpcMessage1>,
    mut reader: R,
    reader_sender: crossbeam_channel::Sender<RpcMessage2>,
) where
    W: 'static + Write + Send,
    R: 'static + BufRead + Send,
    RpcMessage1: 'static + Serialize + DeserializeOwned + Send + Sync,
    RpcMessage2: 'static + Serialize + DeserializeOwned + Send + Sync,
{
    std::thread::spawn(move || {
        for value in writer_receiver {
            if write_msg(&mut writer, value).is_err() {
                return;
            };
        }
    });
    std::thread::spawn(move || -> Result<()> {
        loop {
            if let Some(msg) = read_msg(&mut reader)? {
                reader_sender.send(msg)?;
            }
        }
    });
}

pub fn write_msg<W, RpcMessage>(out: &mut W, msg: RpcMessage) -> Result<()>
where
    W: Write,
    RpcMessage: Serialize,
{
    let msg = format!("{}\n", serde_json::to_string(&msg)?);
    out.write_all(msg.as_bytes())?;
    out.flush()?;
    Ok(())
}

pub fn read_msg<R, RpcMessage>(inp: &mut R) -> Result<Option<RpcMessage>>
where
    R: BufRead,
    RpcMessage: DeserializeOwned,
{
    let mut buf = String::new();
    let _ = inp.read_line(&mut buf)?;
    let value: Value = serde_json::from_str(&buf)?;

    let msg = match serde_json::from_value::<RpcMessage>(value) {
        Ok(msg) => Some(msg),
        Err(_) => None,
    };
    Ok(msg)
}
