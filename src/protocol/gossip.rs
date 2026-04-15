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
