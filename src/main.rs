mod dsl;
mod node;
mod protocol;

use std::fs;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, RwLock};

use anyhow::Result;
use clap::{Parser, Subcommand};
use log::info;
use uuid::Uuid;

use dsl::parse_recipes;
use node::{gossip::start_gossip_service, producer::start_producer_service, NodeState};
use protocol::frame::{recv_message, send_message};
use protocol::production::{GetRecipeMsg, OrderMsg, ProductionProtocol};

#[derive(Parser)]
#[command(name = "pizza_agent", version, about = "Agent de production distribuée de pizzas")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {

    Start {
        #[arg(long, default_value = "127.0.0.1:8001")]
        host: String,
        #[arg(long)]
        capabilities: Option<String>,

        #[arg(long)]
        peer: Option<String>,
        #[arg(long)]
        recipes_file: Option<String>,
        #[arg(long, default_value = "false")]
        debug: bool,
    },

    Client {
        #[arg(long)]
        peer: String,
        #[command(subcommand)]
        subcommand: ClientCommands,
    },

    #[command(name = "list-capabilities")]
    ListCapabilities,
}

#[derive(Subcommand)]
enum ClientCommands {
    #[command(name = "list-recipes")]
    ListRecipes,
    #[command(name = "get-recipe")]
    GetRecipe { recipe_name: String },
    Order { recipe_name: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Start { host, capabilities, peer, recipes_file, debug } => {
            let level = if debug { "debug" } else { "info" };
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();
            cmd_start(host, capabilities, peer, recipes_file)
        }
        Commands::Client { peer, subcommand } => {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
            cmd_client(peer, subcommand)
        }
        Commands::ListCapabilities => { cmd_list_capabilities(); Ok(()) }
    }
}

fn cmd_start(
    host: String,
    capabilities: Option<String>,
    peer: Option<String>,
    recipes_file: Option<String>,
) -> Result<()> {
    let node_id = format!("node-{}", Uuid::new_v4());
    let caps: Vec<String> = capabilities.as_deref().unwrap_or("")
        .split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();

    let state = Arc::new(RwLock::new(NodeState::new(node_id.clone(), host.clone(), caps.clone())));

    if let Some(path) = &recipes_file {
        let content = fs::read_to_string(path)?;
        let recipes = parse_recipes(&content)?;
        let mut s = state.write().unwrap();
        for (_, r) in recipes { s.add_recipe(r); }
        info!("start: {} recettes chargées depuis {}", s.recipes.len(), path);
    }

    let listener = TcpListener::bind(&host)?;
    info!("start: TCP en écoute sur {}", host);
    let _prod = start_producer_service(Arc::clone(&state), listener);

    let udp_socket = UdpSocket::bind(&host)?;
    info!("start: UDP gossip en écoute sur {}", host);

    let bootstrap: Vec<std::net::SocketAddr> = peer.as_deref()
        .into_iter().filter_map(|p| p.parse().ok()).collect();
    if !bootstrap.is_empty() {
        info!("start: bootstrap peers: {:?}", bootstrap);
    }

    let (_send, _recv) = start_gossip_service(Arc::clone(&state), udp_socket, bootstrap);
    info!("start: nœud {} démarré (caps={:?})", node_id, caps);

    _send.join().ok();
    Ok(())
}

fn cmd_client(peer: String, subcommand: ClientCommands) -> Result<()> {
    let addr: std::net::SocketAddr = peer.parse()?;
    let mut stream = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))?;

    match subcommand {
        ClientCommands::ListRecipes => {
            send_message(&mut stream, &ProductionProtocol::ListRecipes)?;
            let resp: ProductionProtocol = recv_message(&mut stream)?;
            if let ProductionProtocol::RecipeListAnswer(r) = resp {
                println!("Recettes disponibles :");
                let mut names: Vec<_> = r.recipes.iter().collect();
                names.sort_by_key(|(n, _)| n.as_str());
                for (name, ma) in names {
                    if ma.is_available() {
                        println!("  ✅ {}", name);
                    } else {
                        println!("  ❌ {} (manque: {:?})", name, ma.missing_list());
                    }
                }
            }
        }
        ClientCommands::GetRecipe { recipe_name } => {
            send_message(&mut stream, &ProductionProtocol::GetRecipe(GetRecipeMsg { recipe_name }))?;
            let resp: ProductionProtocol = recv_message(&mut stream)?;
            if let ProductionProtocol::RecipeAnswer(r) = resp {
                if r.found {
                    println!("Recette {} :\n{}", r.recipe_name, r.recipe.unwrap_or_default());
                } else {
                    println!("Recette '{}' introuvable.", r.recipe_name);
                }
            }
        }
        ClientCommands::Order { recipe_name } => {
            send_message(&mut stream, &ProductionProtocol::Order(OrderMsg { recipe_name }))?;
            loop {
                let resp: ProductionProtocol = recv_message(&mut stream)?;
                match resp {
                    ProductionProtocol::OrderReceipt(r) => {
                        println!("⏳ Commande {} en cours…", r.order_id);
                    }
                    ProductionProtocol::CompletedOrder(c) => {
                        println!("✅ {} terminée !", c.recipe_name);
                        println!("{}", c.result);
                        break;
                    }
                    ProductionProtocol::FailedOrder(f) => {
                        println!("❌ Commande échouée : {}", f.error);
                        break;
                    }
                    ProductionProtocol::OrderDeclined(d) => {
                        println!("❌ Refusée : {}", d.message);
                        break;
                    }
                    other => { println!("⚠ {:?}", other); break; }
                }
            }
        }
    }
    Ok(())
}

fn cmd_list_capabilities() {
    let caps = ["AddBase","AddBasil","AddBBQSauce","AddBellPepper","AddCheese",
                "AddChicken","AddChiliFlakes","AddGarlic","AddHam","AddMushrooms",
                "AddOliveOil","AddOnion","AddOregano","AddPepperoni","AddPineapple",
                "Bake","MakeDough"];
    println!("Actions disponibles :");
    for c in &caps { println!("  - {}", c); }
}
