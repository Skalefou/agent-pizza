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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::production::{OrderMsg, ProductionProtocol};
    use std::io::Cursor;

    #[test]
    fn test_send_recv_roundtrip() {
        let msg = ProductionProtocol::Order(OrderMsg { recipe_name: "Pepperoni".to_string() });
        let mut buf = Vec::new();
        send_message(&mut buf, &msg).expect("envoi");
        let len = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        assert_eq!(len, buf.len() - 4);
        let mut cursor = Cursor::new(buf);
        let decoded: ProductionProtocol = recv_message(&mut cursor).expect("réception");
        assert!(matches!(decoded, ProductionProtocol::Order(_)));
    }

    #[test]
    fn test_multiple_messages_in_stream() {
        let mut buf = Vec::new();
        send_message(&mut buf, &ProductionProtocol::ListRecipes).unwrap();
        send_message(&mut buf, &ProductionProtocol::Order(OrderMsg {
            recipe_name: "Funghi".to_string(),
        })).unwrap();
        let mut cursor = Cursor::new(buf);
        let m1: ProductionProtocol = recv_message(&mut cursor).unwrap();
        let m2: ProductionProtocol = recv_message(&mut cursor).unwrap();
        assert!(matches!(m1, ProductionProtocol::ListRecipes));
        assert!(matches!(m2, ProductionProtocol::Order(_)));
    }
}
