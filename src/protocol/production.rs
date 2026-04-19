use std::collections::HashMap;
use ciborium::value::Value;
use serde::{Deserialize, Deserializer, Serialize};

fn deser_cbor_uuid<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = Value::deserialize(d)?;
    match v {
        Value::Tag(37, inner) => match *inner {
            Value::Text(s) => Ok(s),
            _ => Err(serde::de::Error::custom("tag 37 : attendu text")),
        },
        Value::Text(s) => Ok(s),
        _ => Err(serde::de::Error::custom("UUID : type CBOR inattendu")),
    }
}

fn ser_cbor_uuid<S: serde::Serializer>(uuid: &str, s: S) -> Result<S::Ok, S::Error> {
    Value::Tag(37, Box::new(Value::Text(uuid.to_string()))).serialize(s)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionProtocol {
    ListRecipes,
    GetRecipe(GetRecipeMsg),
    Order(OrderMsg),
    RecipeListAnswer(RecipeListAnswerMsg),
    RecipeAnswer(RecipeAnswerMsg),
    OrderReceipt(OrderReceiptMsg),
    CompletedOrder(CompletedOrderMsg),
    FailedOrder(FailedOrderMsg),
    OrderDeclined(OrderDeclinedMsg),
    ProcessPayload(ProcessPayloadMsg),
    Deliver(DeliverMsg),
    ProductionError(ProductionErrorMsg),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRecipeMsg {
    pub recipe_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderMsg {
    pub recipe_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeListAnswerMsg {
    pub recipes: HashMap<String, MissingActions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingActions {
    Local { missing_actions: Vec<String> },
    Remote {
        host: String,
        #[serde(default)]
        missing: Vec<String>,
    },
}

impl MissingActions {
    pub fn is_available(&self) -> bool {
        matches!(self, MissingActions::Local { missing_actions } if missing_actions.is_empty())
    }

    pub fn missing_list(&self) -> Vec<String> {
        match self {
            MissingActions::Local { missing_actions } => missing_actions.clone(),
            MissingActions::Remote { missing, .. } => missing.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeAnswerMsg {
    pub recipe_name: String,
    pub recipe: Option<String>,
    pub found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderReceiptMsg {
    #[serde(deserialize_with = "deser_cbor_uuid", serialize_with = "ser_cbor_uuid")]
    pub order_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedOrderMsg {
    pub recipe_name: String,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedOrderMsg {
    pub order_id: String,
    pub recipe_name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDeclinedMsg {
    pub order_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPayloadMsg {
    #[serde(deserialize_with = "deser_cbor_uuid", serialize_with = "ser_cbor_uuid")]
    pub order_id: String,
    pub order_timestamp: i64,
    pub delivery_host: String,
    pub action_index: u32,
    pub action_sequence: Vec<ActionStep>,
    pub content: String,
    pub updates: Vec<String>,
    pub name: String,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    pub name: String,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverMsg {
    #[serde(deserialize_with = "deser_cbor_uuid", serialize_with = "ser_cbor_uuid")]
    pub order_id: String,
    pub content: String,
    pub updates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionErrorMsg {
    pub error: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i+2], 16).unwrap())
            .collect()
    }

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de>>(v: &T) -> T {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(v, &mut buf).unwrap();
        ciborium::de::from_reader(buf.as_slice()).unwrap()
    }

    #[test]
    fn test_list_recipes_est_bare_string() {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&ProductionProtocol::ListRecipes, &mut buf).unwrap();
        let s: String = ciborium::de::from_reader(buf.as_slice()).unwrap();
        assert_eq!(s, "list_recipes");
    }

    #[test]
    fn test_decode_list_recipes_capture() {

        let bytes = hex_decode("6c6c6973745f72656369706573");
        let msg: ProductionProtocol = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        assert!(matches!(msg, ProductionProtocol::ListRecipes));
    }

    #[test]
    fn test_decode_order_capture() {

        let bytes = hex_decode("a1656f72646572a16b7265636970655f6e616d65695065707065726f6e69");
        let msg: ProductionProtocol = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        if let ProductionProtocol::Order(o) = msg {
            assert_eq!(o.recipe_name, "Pepperoni");
        } else { panic!("attendu Order"); }
    }

    #[test]
    fn test_decode_order_receipt_capture() {

        let bytes = hex_decode("a16d6f726465725f72656365697074a1686f726465725f6964d825782437373466643365652d623238342d343063622d613361362d626638393239643762313333");
        let msg: ProductionProtocol = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        if let ProductionProtocol::OrderReceipt(r) = msg {
            assert_eq!(r.order_id, "774fd3ee-b284-40cb-a3a6-bf8929d7b133");
        } else { panic!("attendu OrderReceipt"); }
    }

    #[test]
    fn test_order_roundtrip() {
        let msg = ProductionProtocol::Order(OrderMsg { recipe_name: "Margherita".to_string() });
        let decoded = roundtrip(&msg);
        if let ProductionProtocol::Order(o) = decoded {
            assert_eq!(o.recipe_name, "Margherita");
        } else { panic!("mauvais variant"); }
    }
}
