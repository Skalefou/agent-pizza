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
