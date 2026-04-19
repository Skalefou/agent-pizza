use ciborium::value::Value;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub struct CborAddr(pub String);

impl Serialize for CborAddr {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Value::Tag(260, Box::new(Value::Text(self.0.clone()))).serialize(s)
    }
}

impl<'de> Deserialize<'de> for CborAddr {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        match v {
            Value::Tag(260, inner) => match *inner {
                Value::Text(s) => Ok(CborAddr(s)),
                _ => Err(serde::de::Error::custom("tag 260 : contenu attendu text")),
            },
            Value::Text(s) => Ok(CborAddr(s)),
            _ => Err(serde::de::Error::custom("CborAddr : type CBOR inattendu")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerVersion {
    pub counter: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    Announce(AnnouncePayload),
    Ping(PingPayload),
    Pong(PongPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncePayload {
    pub node_addr: CborAddr,
    pub capabilities: Vec<String>,
    pub recipes: Vec<String>,
    pub peers: Vec<CborAddr>,
    pub version: PeerVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingPayload {
    pub node_addr: CborAddr,
    pub version: PeerVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PongPayload {
    pub node_addr: CborAddr,
    pub version: PeerVersion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbor_addr_roundtrip() {
        let addr = CborAddr("127.0.0.1:8001".to_string());
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&addr, &mut buf).unwrap();

        assert_eq!(&buf[..3], &[0xd9, 0x01, 0x04]);
        let decoded: CborAddr = ciborium::de::from_reader(buf.as_slice()).unwrap();
        assert_eq!(decoded.0, "127.0.0.1:8001");
    }

    #[test]
    fn test_announce_roundtrip() {
        let msg = GossipMessage::Announce(AnnouncePayload {
            node_addr: CborAddr("127.0.0.1:8001".to_string()),
            capabilities: vec!["MakeDough".to_string()],
            recipes: vec![],
            peers: vec![CborAddr("127.0.0.1:8002".to_string())],
            version: PeerVersion { counter: 1, generation: 42 },
        });
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&msg, &mut buf).unwrap();
        let decoded: GossipMessage = ciborium::de::from_reader(buf.as_slice()).unwrap();
        if let GossipMessage::Announce(p) = decoded {
            assert_eq!(p.node_addr.0, "127.0.0.1:8001");
            assert_eq!(p.version.counter, 1);
        } else { panic!("mauvais variant"); }
    }

    #[test]
    fn test_decode_announce_capture() {

        let hex = "a168416e6e6f756e6365a5696e6f64655f61646472d901046e3132372e302e302e313a383035306c6361706162696c697469657381675465737443617067726563697065738065706565727381d901046e3132372e302e302e313a383039396776657273696f6ea267636f756e746572016a67656e65726174696f6e1a69d54b4c";
        let bytes: Vec<u8> = (0..hex.len()).step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i+2], 16).unwrap())
            .collect();
        let msg: GossipMessage = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        if let GossipMessage::Announce(p) = msg {
            assert_eq!(p.node_addr.0, "127.0.0.1:8050");
            assert_eq!(p.capabilities, vec!["TestCap"]);
            assert_eq!(p.peers[0].0, "127.0.0.1:8099");
            assert_eq!(p.version.counter, 1);
        } else { panic!("attendu Announce"); }
    }
}
