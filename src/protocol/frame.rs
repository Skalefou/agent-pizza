use std::io::{Read, Write};
use serde::{Deserialize, Serialize};

pub fn send_message<W, T>(writer: &mut W, msg: &T) -> anyhow::Result<()>
where
    W: Write,
    T: Serialize,
{
    let mut payload = Vec::new();
    ciborium::ser::into_writer(msg, &mut payload)?;
    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

pub fn recv_message<R, T>(reader: &mut R) -> anyhow::Result<T>
where
    R: Read,
    T: for<'de> Deserialize<'de>,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    let msg: T = ciborium::de::from_reader(payload.as_slice())?;
    Ok(msg)
}
