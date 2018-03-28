extern crate actix_web;
extern crate crypto;
extern crate serde_json;
extern crate reqwest;
#[macro_use] extern crate serde_derive;

use std::env;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use crypto::sha2::Sha256;
use crypto::digest::Digest;
use actix_web::*;

const MINER_ADDRESS: &str = "q3nf394hjg-random-miner-address-34nf3i4nflkn3oi";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Block {
    index: i32,
    timestamp: u64,
    data: String,
    previous_hash: String,
    hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BlockData {
    proof_of_work: u64,
    transactions: Vec<Transaction>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct Transaction {
    from: String,
    to: String,
    amount: i32,
}

fn hash_block(index: i32, timestamp: u64, data: &str, previous_hash: &str) -> String {
    let mut sha256 = Sha256::new();
    let source = format!("{}{}{}{}", index, timestamp, data, previous_hash);
    sha256.input_str(source.as_str());
    sha256.result_str()
}

impl Block {
    pub fn new(index: i32, timestamp: u64, data: &str, previous_hash: &str) -> Self {
        Self {
            index: index,
            timestamp: timestamp,
            data: data.to_string(),
            previous_hash: previous_hash.to_string(),
            hash: hash_block(index, timestamp, data, previous_hash),
        }
    }
}

fn create_genesis_block(data: &str) -> Block {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).expect("fail get epoch time");
    Block::new(0, d.as_secs(), data, "0")
}

#[allow(dead_code)]
fn next_block(last_block: &Block) -> Block {
    let next_index = last_block.index + 1;
    let next_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("error-id:72bd26e9826b2b9ec1b7fda394674cfa").as_secs();
    let next_data = format!("Hey! I'm block {}", next_index);
    let next_hash = last_block.hash.clone();
    Block::new(next_index, next_timestamp, next_data.as_str(), next_hash.as_str())
}

fn proof_of_work(last_proof: u64) -> u64 {
    let mut incrementor = last_proof + 1;
    while !(incrementor % 9 == 0 && incrementor % last_proof == 0) {
        incrementor += 1;
    }
    incrementor
}

// for web
struct AppState {
    blockchain: Arc<Mutex<Vec<Block>>>,
    this_node_transactions: Arc<Mutex<Vec<Transaction>>>,
    peer_nodes: Arc<Mutex<Vec<String>>>,
}

// POST /txion?from=FFF&to=TTT&amount=AAA
fn transaction(req: HttpRequest<AppState>) -> String {
    let state = (&req).state();
    let mut this_node_transactions = state.this_node_transactions.lock().expect("error-id:7b7f557ee5a8523cd51230d1c37d1d64");

    let query = req.query();
    let from = query.get("from").expect("error-id:74d3a8555747901c7c1244794d69dd05");
    let to = query.get("to").expect("error-id:e817c5e3f7ad9a6fde1bce8e94c2a265");
    let amount = query.get("amount").expect("error-id:c1a202cbe45ab0ea6fc840c303cac496");
    println!("=== New transaction ===");
    println!("FROM: {}", from);
    println!("TO: {}", to);
    println!("AMOUNT: {}", amount);
    this_node_transactions.push(Transaction{
        from: from.to_string(),
        to: to.to_string(),
        amount: std::str::FromStr::from_str(amount).expect("fail convert amount"),
    });
    "Transaction submission successful\n".to_string()
}

fn find_other_chains(peer_nodes: &Vec<String>) -> Vec<Vec<Block>> {
    let mut ret: Vec<Vec<Block>> = vec![];
    for peer_node in peer_nodes {
        let url = format!("http://{}/blocks", peer_node);
        println!("get access url={}", url);
        // TODO: use actix_web::client
        let text = reqwest::get(url.as_str()).expect("reqwest.get error")
            .text().expect("get text error");
        ret.push(serde_json::from_str(text.as_str()).unwrap());
    }
    ret
}

fn consensus(req: &HttpRequest<AppState>, blockchain: &Vec<Block>) -> Vec<Block> {
    let mut longest_chain = blockchain.clone();
    let peer_nodes = req.state().peer_nodes.lock().expect("peer_nodes.lock() error");
    for chain in find_other_chains(&peer_nodes) {
        if longest_chain.len() < chain.len() {
            longest_chain = chain.clone();
        }
    }
    longest_chain
}

// GET /mine
fn mine(req: HttpRequest<AppState>) -> Result<HttpResponse> {
    let mut blockchain = req.state().blockchain.lock().expect("error-id:54dd798541ec63ee7f7ffefe0e6f9baa");
    let last_block = blockchain[blockchain.len()-1].clone();
    println!("{}", last_block.data);
    let last_proof: BlockData = serde_json::from_str(&last_block.data).expect("json decode error");
    let last_proof = last_proof.proof_of_work;

    let proof = proof_of_work(last_proof);
    let mut this_node_transactions = req.state().this_node_transactions.lock().expect("error-id:b49e7d0179ba50d2e694dba1d9d9323c");
    this_node_transactions.push(Transaction{
        from: "network".to_string(),
        to: MINER_ADDRESS.to_string(),
        amount: 1
    });

    let new_block_data = serde_json::to_string(&BlockData {
        proof_of_work: proof,
        transactions: (*this_node_transactions).clone(),
    }).expect("fail to encode block data");
    let new_block_index = last_block.index + 1;
    let new_block_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH).expect("fail get epoch time")
        .as_secs();
    let last_block_hash = last_block.hash.clone();
    this_node_transactions.clear();

    let mined_block = Block::new(
        new_block_index,
        new_block_timestamp,
        new_block_data.as_str(),
        last_block_hash.as_str());
    blockchain.push(mined_block.clone());

    httpcodes::HTTPOk.build().json(mined_block)
}

// GET /blocks
fn get_blocks(req: HttpRequest<AppState>) -> Result<HttpResponse> {
    let blockchain = req.state().blockchain.lock().expect("blockchain.lock() error");
    let chain_to_send: Vec<Block> = consensus(&req, &blockchain).iter().map(|x| x.clone()).collect();
    httpcodes::HTTPOk.build().json(chain_to_send)
}

// POST /add_peer?host=HHH&port=PPP
fn add_peer(req: HttpRequest<AppState>) -> String {
    let query = req.query();
    let host = query.get("host").unwrap_or("localhost");
    let port = query.get("port").expect("fail get port param from params");
    let address = format!("{}:{}", host, port);
    let mut peer_nodes = req.state().peer_nodes.lock().expect("peer_nodes.lock() error");
    match peer_nodes.iter().find(|x| **x == address) {
        None => {
            println!("add peer: {}", address);
            peer_nodes.push(address);
            "Ok".to_string()
        },
        _ => "Already exists".to_string()
    }
}

fn _standalone_blockchain() {
    let mut blockchain = vec![create_genesis_block("Genesis Block")];
    let mut previous_block = blockchain[0].clone();
    let num_of_blocks_to_add = 20;
    for _ in 0..num_of_blocks_to_add {
        let block_to_add = next_block(&previous_block);
        blockchain.push(block_to_add.clone());
        previous_block = block_to_add.clone();
        println!("Block #{} has been added to the blockchain!", block_to_add.index);
        println!("Hash: {}\n", block_to_add.hash);
    }
}

fn main() {
    let mut args = env::args();
    let address = if args.len() <= 1 {
        "127.0.0.1:5000".to_string()
    } else {
        args.nth(1).expect("arg error")
    };

    let d = serde_json::to_string(&BlockData {
        proof_of_work: 9,
        transactions: vec![],
    }).expect("genesis block data encode error");
    let blockchain = Arc::new(Mutex::new(vec![create_genesis_block(d.as_str())]));
    let transactions = Arc::new(Mutex::new(vec![]));
    let peer_nodes = Arc::new(Mutex::new(vec![]));

    println!("bind: {}", address);
    HttpServer::new(move
        || Application::with_state(AppState {
            blockchain: blockchain.clone(),
            this_node_transactions: transactions.clone(),
            peer_nodes: peer_nodes.clone(),
        })
        .resource("/mine", |r| r.f(mine))
        .resource("/blocks", |r| r.f(get_blocks))
        .resource("/add_peer", |r| r.method(Method::POST).f(add_peer))
        .resource("/txion", |r| r.method(Method::POST).f(transaction)))
        .bind(address.as_str()).expect("bind error")
        .run();
}
