# Spécification du Protocole Pizza Factory

## Vue générale

Le système Pizza Factory est un réseau distribué d'agents qui collaborent pour produire des pizzas selon des recettes. Le protocole repose sur deux couches de communication :

1. **Couche Gossip (UDP)** : Découverte mutuelle des agents, broadcast de capacités et recettes
2. **Couche Production (TCP)** : Commandes, acheminement des pizzas, exécution des étapes

## Hypothèses (à confirmer par capture réseau)

### Sérialisation
- Format : **CBOR** (Concise Binary Object Representation, RFC 7049)
- Justification : léger, binaire, auto-décrivant, standard pour IoT/agents

### Réseau
- **UDP** : Port dynamique ou configuré, messages de découverte (gossip)
- **TCP** : Port configuré pour le même agent, production et commandes
- Port unique par agent (ex : `127.0.0.1:8001`)
- Framing TCP : probablement avec préfixe de longueur (4 ou 8 bytes) ou délimiteur

## Types de Messages

### Messages UDP (Gossip Layer)

#### Announce
Lors du lancement de deux serveurs, l'un annonce sa présence à l'autre.
Ce message, capturé avec Wireshark, est relayé par chaque agent à l'ensemble de ses contacts. 
Ce mécanisme de propagation permet à tous les agents du réseau de connaître l'existence et les capacités de leurs pairs.
````text
0000   02 00 00 00 45 00 00 b9 29 e0 00 00 80 11 12 52   ....E...)......R
0010   7f 00 00 01 7f 00 00 01 1f 41 1f 40 00 a5 ec 5f   .........A.@..._
0020   a1 68 41 6e 6e 6f 75 6e 63 65 a5 69 6e 6f 64 65   .hAnnounce.inode
0030   5f 61 64 64 72 d9 01 04 6e 31 32 37 2e 30 2e 30   _addr...n127.0.0
0040   2e 31 3a 38 30 30 31 6c 63 61 70 61 62 69 6c 69   .1:8001lcapabili
0050   74 69 65 73 84 67 41 64 64 42 61 73 65 69 41 64   ties.gAddBaseiAd
0060   64 43 68 65 65 73 65 6c 41 64 64 50 65 70 70 65   dCheeselAddPeppe
0070   72 6f 6e 69 64 42 61 6b 65 67 72 65 63 69 70 65   ronidBakegrecipe
0080   73 80 65 70 65 65 72 73 81 d9 01 04 6e 31 32 37   s.epeers....n127
0090   2e 30 2e 30 2e 31 3a 38 30 30 30 67 76 65 72 73   .0.0.1:8000gvers
00a0   69 6f 6e a2 67 63 6f 75 6e 74 65 72 01 6a 67 65   ion.gcounter.jge
00b0   6e 65 72 61 74 69 6f 6e 1a 69 cc 0b a8            neration.i...
````
En analysant ce message, nous pouvons en déduire la structure suivante :

```
- node_id: String (UUID ou string unique)
- host_addr: String (ex: "127.0.0.1")
- port: u16 (ex: 8001)
- capabilities: Vec<String> (ex: ["MakeDough", "AddBase"])
- version: u32 (version du protocole)
- timestamp: i64 (last announcement time, epoch Unix)
```

#### Ping
Après l'annonce, un nœud envoi régulièrement le message suivant :
```text
0000   02 00 00 00 45 00 00 5f 29 fe 00 00 80 11 12 8e   ....E.._).......
0010   7f 00 00 01 7f 00 00 01 1f 41 1f 40 00 4b 6d 80   .........A.@.Km.
0020   a1 64 50 69 6e 67 a2 69 6c 61 73 74 5f 73 65 65   .dPing.ilast_see
0030   6e d9 03 e9 a2 25 1a 00 03 b2 c8 01 1a 69 cc 0b   n....%.......i..
0040   a8 67 76 65 72 73 69 6f 6e a2 67 63 6f 75 6e 74   .gversion.gcount
0050   65 72 01 6a 67 65 6e 65 72 61 74 69 6f 6e 1a 69   er.jgeneration.i
0060   cc 0b a5                                          ...
```


Il a pour but de vérifier qu'un pair est toujours vivant. Il se fait d'un nœud A vers un nœud B. 
Il contient la structure suivante :
```
- sender_id: String
- sender_addr: String
- version: u32
```

#### Pong
En réponse au Ping, le nœud B envoie un message Pong au nœud A :
```text
0000   02 00 00 00 45 00 00 5f 29 ff 00 00 80 11 12 8d   ....E.._).......
0010   7f 00 00 01 7f 00 00 01 1f 40 1f 41 00 4b 6d 7a   .........@.A.Kmz
0020   a1 64 50 6f 6e 67 a2 69 6c 61 73 74 5f 73 65 65   .dPong.ilast_see
0030   6e d9 03 e9 a2 25 1a 00 03 b2 c8 01 1a 69 cc 0b   n....%.......i..
0040   a8 67 76 65 72 73 69 6f 6e a2 67 63 6f 75 6e 74   .gversion.gcount
0050   65 72 01 6a 67 65 6e 65 72 61 74 69 6f 6e 1a 69   er.jgeneration.i
0060   cc 0b a5                                          ...
```

Il a pour but de répondre au Ping, et de confirmer que le nœud est toujours vivant. 
Il se fait d'un noeud B vers un noeud A.
Ainsi il contient les memes informations que le Ping, mais avec des champs de sender et responder inversés :

```text
- responder_id: String
- responder_addr: String
- version: u32
```
#### Diagramme de séquence - Gossip

<img alt="diagramme de séquance - gossip" src="gossip.svg" />

### Messages TCP (Production Layer)

#### ListRecipes
**Rôle** : Client demande la liste des recettes connues d'un agent  
**Direction** : Client TCP → Nœud (TCP connect + send)  

**Champs estimés** :
```
- (Empty ou juste un marker type)
```

#### ListRecipesResponse
**Rôle** : Réponse avec recettes disponibles et manquantes  
**Direction** : Nœud → Client (TCP send)  

**Champs estimés** :
```
- recipes: Vec<RecipeInfo>
  - name: String
  - missing_actions: Vec<String> (ex: ["AddMushrooms", "AddBasil"])
  - available: bool
```

#### GetRecipe
**Rôle** : Récupérer la définition DSL d'une recette  
**Direction** : Client → Nœud (TCP)  

**Champs estimés** :
```
- recipe_name: String
```

#### GetRecipeResponse
**Rôle** : Recette en format texte DSL  
**Direction** : Nœud → Client (TCP)  

**Champs estimés** :
```
- recipe_name: String
- dsl_content: String (ex: "MakeDough -> AddBase(...) -> Bake(...)")
- found: bool
```

#### Order
**Rôle** : Commande pour produire une pizza  
**Direction** : Client → Nœud (TCP)  

**Champs estimés** :
```
- order_id: String (UUID)
- recipe_name: String (ex: "Margherita")
- timestamp: i64 (Unix epoch when order was created)
- requester_addr: String (client address, may be ignored)
```

#### OrderStatus / OrderUpdate
**Rôle** : Mise à jour de la progression d'une commande  
**Direction** : Nœud courant → Nœud qui a lancé la commande, ou vers client  

**Champs estimés** :
```
- order_id: String
- status: String (ex: "in_progress", "completed", "failed")
- completed_steps: Vec<String> (ex: ["MakeDough", "AddBase"])
- next_action: String (optional)
- current_host: String (nœud exécutant l'action)
```

#### DeliverOrder
**Rôle** : Livraison finale d'une pizza  
**Direction** : Dernier nœud → Nœud demandeur ou client  

**Champs estimés** :
```
- order_id: String
- recipe_name: String
- status: String ("completed" ou "failed")
- final_pizza_state: String (description de la pizza)
- timestamp: i64
```

#### OrderRejected / OrderFailed
**Rôle** : Rejet ou échec d'une commande  
**Direction** : Nœud → Client ou nœud précédent  

**Champs estimés** :
```
- order_id: String
- reason: String (ex: "Action AddMushrooms not available on any peer")
- recipe_name: String
```

#### Diagramme de séquence - Production
<img alt="diagramme de séquance - com client serveur" src="commande-pizza.svg">