use std::collections::HashMap;
use std::time::Instant;
use crate::dsl::Recipe;
use rand;

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub host: String,
    pub capabilities: Vec<String>,
    #[allow(dead_code)]
    pub recipes: Vec<String>,
    pub generation: u64,
    pub last_seen: Instant,
}

#[derive(Debug)]
pub struct NodeState {
    #[allow(dead_code)]
    pub node_id: String,
    pub host: String,
    pub capabilities: Vec<String>,
    pub recipes: HashMap<String, Recipe>,
    pub generation: u64,
    pub node_generation_id: u64,
    pub peers: HashMap<String, PeerInfo>,
}

impl NodeState {
    pub fn new(node_id: String, host: String, capabilities: Vec<String>) -> Self {
        let node_generation_id = rand::random::<u32>() as u64;
        NodeState {
            node_id,
            host,
            capabilities,
            recipes: HashMap::new(),
            generation: 1,
            node_generation_id,
            peers: HashMap::new(),
        }
    }

    pub fn upsert_peer(&mut self, key: String, info: PeerInfo) {
        self.peers.insert(key, info);
    }

    pub fn evict_stale_peers(&mut self, timeout: std::time::Duration) {
        let now = Instant::now();
        self.peers.retain(|_, p| now.duration_since(p.last_seen) < timeout);
    }

    pub fn peer_hosts(&self) -> Vec<String> {
        self.peers.values().map(|p| p.host.clone()).collect()
    }

    pub fn all_capabilities(&self) -> Vec<String> {
        let mut caps: std::collections::HashSet<String> =
            self.capabilities.iter().cloned().collect();
        for peer in self.peers.values() {
            caps.extend(peer.capabilities.iter().cloned());
        }
        caps.into_iter().collect()
    }

    pub fn add_recipe(&mut self, recipe: Recipe) {
        self.recipes.insert(recipe.name.clone(), recipe);
    }

    pub fn recipe_names(&self) -> Vec<String> {
        self.recipes.keys().cloned().collect()
    }

    pub fn find_peer_for_action(&self, action: &str) -> Option<String> {
        self.peers
            .values()
            .find(|p| p.capabilities.contains(&action.to_string()))
            .map(|p| p.host.clone())
    }
}
