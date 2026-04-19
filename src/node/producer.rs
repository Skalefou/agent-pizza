use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{debug, info, warn};
use uuid::Uuid;

use crate::node::state::NodeState;
use crate::protocol::frame::{recv_message, send_message};
use crate::protocol::gossip::CborAddr;
use crate::protocol::production::{
    ActionStep, CompletedOrderMsg, FailedOrderMsg, GetRecipeMsg,
    OrderMsg, OrderReceiptMsg, ProcessPayloadMsg,
    ProductionProtocol, RecipeAnswerMsg, RecipeAvailability, RecipeListAnswerMsg, RecipeStatus, Update,
};

pub fn start_producer_service(
    state: Arc<RwLock<NodeState>>,
    listener: TcpListener,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        info!("producer: TCP en écoute sur {}", listener.local_addr().unwrap());
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    let st = Arc::clone(&state);
                    std::thread::spawn(move || handle_connection(st, s));
                }
                Err(e) => warn!("producer: accept: {}", e),
            }
        }
    })
}

fn handle_connection(state: Arc<RwLock<NodeState>>, mut stream: TcpStream) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    debug!("producer: connexion de {}", peer);
    loop {
        let msg: ProductionProtocol = match recv_message(&mut stream) {
            Ok(m) => m,
            Err(e) => { debug!("producer: {}: {}", peer, e); return; }
        };
        match msg {
            ProductionProtocol::ListRecipes       => { handle_list_recipes(&state, &mut stream); }
            ProductionProtocol::GetRecipe(r)      => { handle_get_recipe(&state, &mut stream, r); }
            ProductionProtocol::Order(r)          => { handle_order(&state, &mut stream, r); return; }
            ProductionProtocol::ProcessPayload(p) => { handle_process_payload(&state, &mut stream, p); return; }
            other => warn!("producer: message inattendu: {:?}", other),
        }
    }
}

fn handle_list_recipes(state: &Arc<RwLock<NodeState>>, stream: &mut TcpStream) {
    let s = state.read().unwrap();
    let local_caps = &s.capabilities;

    let mut recipes: HashMap<String, RecipeAvailability> = s.recipes.values().map(|r| {
        let missing = r.missing_actions(local_caps);
        let avail = RecipeAvailability {
            local: RecipeStatus { missing_actions: missing },
            remote_peers: vec![],
        };
        (r.name.clone(), avail)
    }).collect();

    for (peer_addr, peer_info) in &s.peers {
        for recipe_name in &peer_info.recipes {
            recipes.entry(recipe_name.clone())
                .or_insert_with(|| RecipeAvailability {
                    local: RecipeStatus { missing_actions: vec![] },
                    remote_peers: vec![],
                })
                .remote_peers
                .push(peer_addr.clone());
        }
    }

    let _ = send_message(stream, &ProductionProtocol::RecipeListAnswer(
        RecipeListAnswerMsg { recipes }
    ));
}

fn handle_get_recipe(state: &Arc<RwLock<NodeState>>, stream: &mut TcpStream, req: GetRecipeMsg) {
    let s = state.read().unwrap();
    let (found, recipe_dsl) = match s.recipes.get(&req.recipe_name) {
        Some(r) => (true, Some(recipe_to_dsl(r))),
        None => (false, None),
    };
    let _ = send_message(stream, &ProductionProtocol::RecipeAnswer(RecipeAnswerMsg {
        recipe_name: req.recipe_name, recipe: recipe_dsl, found,
    }));
}

fn handle_order(state: &Arc<RwLock<NodeState>>, stream: &mut TcpStream, req: OrderMsg) {
    info!("producer: commande pour {}", req.recipe_name);
    let order_id = Uuid::new_v4().to_string();
    let order_timestamp = now_micros();
    let delivery_host = CborAddr(state.read().unwrap().host.clone());

    let _ = send_message(stream, &ProductionProtocol::OrderReceipt(OrderReceiptMsg {
        order_id: order_id.clone(),
    }));

    let recipe = state.read().unwrap().recipes.get(&req.recipe_name).cloned();
    let recipe = match recipe {
        Some(r) => r,
        None => {
            let _ = send_message(stream, &ProductionProtocol::FailedOrder(FailedOrderMsg {
                order_id, recipe_name: req.recipe_name,
                error: "recette introuvable".to_string(),
            }));
            return;
        }
    };

    let actions: Vec<ActionStep> = recipe.all_actions().into_iter().map(|a| ActionStep {
        name: a.name, params: a.params,
    }).collect();

    if actions.is_empty() {
        let _ = send_message(stream, &ProductionProtocol::CompletedOrder(CompletedOrderMsg {
            recipe_name: recipe.name, result: String::new(),
        }));
        return;
    }

    let payload = ProcessPayloadMsg {
        order_id,
        order_timestamp,
        delivery_host,
        action_index: 0,
        action_sequence: actions,
        content: String::new(),
        updates: vec![],
    };

    process_pipeline(state, stream, payload, recipe.name);
}

fn handle_process_payload(
    state: &Arc<RwLock<NodeState>>,
    stream: &mut TcpStream,
    payload: ProcessPayloadMsg,
) {
    let recipe_name = payload.action_sequence
        .first().map(|a| a.name.clone()).unwrap_or_default();
    process_pipeline(state, stream, payload, recipe_name);
}

fn process_pipeline(
    state: &Arc<RwLock<NodeState>>,
    stream: &mut TcpStream,
    mut payload: ProcessPayloadMsg,
    recipe_name: String,
) {
    loop {
        let idx = payload.action_index as usize;
        if idx >= payload.action_sequence.len() {
            let _ = send_message(stream, &ProductionProtocol::CompletedOrder(CompletedOrderMsg {
                recipe_name, result: payload.content.clone(),
            }));
            return;
        }

        let action_name = payload.action_sequence[idx].name.clone();
        let can_do = state.read().unwrap().capabilities.contains(&action_name);

        if can_do {
            let ts = now_micros();
            let new_content = execute_action(&action_name, &payload.action_sequence[idx].params, &payload.content);
            payload.updates.push(Update::Action {
                action: payload.action_sequence[idx].clone(),
                timestamp: ts,
            });
            payload.content = new_content;
            payload.action_index += 1;
            info!("producer: {} effectuée (commande {})", action_name, payload.order_id);
        } else {
            let peer_host = state.read().unwrap().find_peer_for_action(&action_name);
            match peer_host {
                Some(host) => {
                    let ts = now_micros();
                    payload.updates.push(Update::Forward {
                        to: CborAddr(host.clone()),
                        timestamp: ts,
                    });
                    info!("producer: transfer {} → {} pour {}", payload.order_id, host, action_name);
                    match forward_payload(&host, &payload) {
                        Ok(ProductionProtocol::CompletedOrder(c)) => {
                            let _ = send_message(stream, &ProductionProtocol::CompletedOrder(c));
                            return;
                        }
                        Ok(ProductionProtocol::FailedOrder(f)) => {
                            let _ = send_message(stream, &ProductionProtocol::FailedOrder(f));
                            return;
                        }
                        Ok(other) => {
                            warn!("producer: réponse inattendue du pair: {:?}", other);
                            let _ = send_message(stream, &ProductionProtocol::FailedOrder(FailedOrderMsg {
                                order_id: payload.order_id, recipe_name,
                                error: "réponse inattendue".to_string(),
                            }));
                            return;
                        }
                        Err(e) => {
                            warn!("producer: erreur transfert: {}", e);
                            let _ = send_message(stream, &ProductionProtocol::FailedOrder(FailedOrderMsg {
                                order_id: payload.order_id, recipe_name,
                                error: format!("transfert échoué: {}", e),
                            }));
                            return;
                        }
                    }
                }
                None => {
                    warn!("producer: aucun pair pour {}", action_name);
                    let _ = send_message(stream, &ProductionProtocol::FailedOrder(FailedOrderMsg {
                        order_id: payload.order_id, recipe_name,
                        error: format!("{} introuvable dans le réseau", action_name),
                    }));
                    return;
                }
            }
        }
    }
}

fn forward_payload(host: &str, payload: &ProcessPayloadMsg) -> anyhow::Result<ProductionProtocol> {
    let addr: std::net::SocketAddr = host.parse()?;
    let mut s = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))?;
    send_message(&mut s, &ProductionProtocol::ProcessPayload(payload.clone()))?;
    let resp: ProductionProtocol = recv_message(&mut s)?;
    Ok(resp)
}

fn execute_action(name: &str, params: &HashMap<String, String>, current: &str) -> String {
    let ps: Vec<String> = params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
    if ps.is_empty() {
        if current.is_empty() { name.to_string() } else { format!("{} | {}", current, name) }
    } else {
        let pstr = ps.join(", ");
        if current.is_empty() { format!("{}({})", name, pstr) } else { format!("{} | {}({})", current, name, pstr) }
    }
}

fn recipe_to_dsl(recipe: &crate::dsl::Recipe) -> String {
    let mut lines = vec![format!("{} =", recipe.name)];
    for (i, step) in recipe.steps.iter().enumerate() {
        let prefix = if i == 0 { "    " } else { "    -> " };
        let actions: Vec<String> = step.group.actions.iter().map(|a| a.to_string_repr()).collect();
        let group = if actions.len() == 1 { actions[0].clone() } else { format!("[{}]", actions.join(", ")) };
        let rep = if step.repeat > 1 { format!("{}^{}", group, step.repeat) } else { group };
        lines.push(format!("{}{}", prefix, rep));
    }
    lines.join("\n")
}

fn now_micros() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros() as u64
}
