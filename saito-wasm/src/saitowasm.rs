use std::collections::HashMap;
use std::future::Future;
use std::io::Error;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Poll, Waker};
use std::time::Duration;

use js_sys::{Array, BigInt, Uint8Array};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{Mutex, RwLock};
use wasm_bindgen::prelude::*;

use saito_core::common::defs::{Currency, SaitoHash, SaitoPublicKey, SaitoSignature};
use saito_core::common::process_event::ProcessEvent;
use saito_core::core::consensus_event_processor::{ConsensusEvent, ConsensusEventProcessor};
use saito_core::core::data::blockchain::Blockchain;
use saito_core::core::data::configuration::Configuration;
use saito_core::core::data::context::Context;
use saito_core::core::data::mempool::Mempool;
use saito_core::core::data::miner::Miner;
use saito_core::core::data::network::Network;
use saito_core::core::data::peer_collection::PeerCollection;
use saito_core::core::data::storage::Storage;
use saito_core::core::data::transaction::Transaction;
use saito_core::core::data::wallet::Wallet;
use saito_core::core::mining_event_processor::{MiningEvent, MiningEventProcessor};
use saito_core::core::routing_event_processor::{RoutingEvent, RoutingEventProcessor};

use crate::wasm_io_handler::WasmIoHandler;
use crate::wasm_slip::WasmSlip;
use crate::wasm_task_runner::WasmTaskRunner;
use crate::wasm_time_keeper::WasmTimeKeeper;
use crate::wasm_transaction::WasmTransaction;

pub(crate) struct NetworkResultFuture {
    pub result: Option<Result<Vec<u8>, Error>>,
    pub key: u64,
}

// TODO : check if this gets called from somewhere or need a runtime
impl Future for NetworkResultFuture {
    type Output = Result<Vec<u8>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let mut saito = SAITO.blocking_lock();
        let result = saito.results.remove(&self.key);
        if result.is_some() {
            let result = result.unwrap();
            return Poll::Ready(result);
        }
        let waker = cx.waker().clone();
        saito.wakers.insert(self.key, waker);
        return Poll::Pending;
    }
}

#[wasm_bindgen]
pub struct SaitoWasm {
    consensus_event_processor: RoutingEventProcessor,
    routing_event_processor: ConsensusEventProcessor,
    mining_event_processor: MiningEventProcessor,
    receiver_in_blockchain: Receiver<RoutingEvent>,
    receiver_in_mempool: Receiver<ConsensusEvent>,
    receiver_in_miner: Receiver<MiningEvent>,
    context: Context,
    wakers: HashMap<u64, Waker>,
    results: HashMap<u64, Result<Vec<u8>, Error>>,
}

lazy_static! {
    static ref SAITO: Mutex<SaitoWasm> = Mutex::new(new());
}

// #[wasm_bindgen]
// impl SaitoWasm {}

pub fn new() -> SaitoWasm {
    let wallet = Arc::new(RwLock::new(Wallet::new()));
    let configuration = Arc::new(RwLock::new(Configuration::new()));

    let peers = Arc::new(RwLock::new(PeerCollection::new()));
    let context = Context {
        blockchain: Arc::new(RwLock::new(Blockchain::new(wallet.clone()))),
        mempool: Arc::new(RwLock::new(Mempool::new(wallet.clone()))),
        wallet: wallet.clone(),
        miner: Arc::new(RwLock::new(Miner::new(wallet.clone()))),
        configuration: configuration.clone(),
    };

    let (sender_to_mempool, receiver_in_mempool) = tokio::sync::mpsc::channel(100);
    let (sender_to_blockchain, receiver_in_blockchain) = tokio::sync::mpsc::channel(100);
    let (sender_to_miner, receiver_in_miner) = tokio::sync::mpsc::channel(100);
    SaitoWasm {
        consensus_event_processor: RoutingEventProcessor {
            blockchain: context.blockchain.clone(),
            sender_to_mempool: sender_to_mempool.clone(),
            sender_to_miner: sender_to_miner.clone(),
            static_peers: vec![],
            configs: context.configuration.clone(),
            time_keeper: Box::new(WasmTimeKeeper {}),
            wallet,
            network: Network::new(Box::new(WasmIoHandler {}), peers.clone()),
        },
        routing_event_processor: ConsensusEventProcessor {
            mempool: context.mempool.clone(),
            blockchain: context.blockchain.clone(),
            wallet: context.wallet.clone(),
            sender_to_router: sender_to_blockchain.clone(),
            sender_to_miner: sender_to_miner.clone(),
            // sender_global: (),
            block_producing_timer: 0,
            tx_producing_timer: 0,
            generate_test_tx: false,
            time_keeper: Box::new(WasmTimeKeeper {}),
            network: Network::new(Box::new(WasmIoHandler {}), peers.clone()),
            storage: Storage::new(Box::new(WasmIoHandler {})),
        },
        mining_event_processor: MiningEventProcessor {
            miner: context.miner.clone(),
            sender_to_blockchain: sender_to_blockchain.clone(),
            sender_to_mempool: sender_to_mempool.clone(),
            time_keeper: Box::new(WasmTimeKeeper {}),
            miner_timer: 0,
            new_miner_event_received: false,
        },
        receiver_in_blockchain,
        receiver_in_mempool,
        receiver_in_miner,
        context,
        wakers: Default::default(),
        results: Default::default(),
    }
}

#[wasm_bindgen]
pub async fn initialize() -> Result<JsValue, JsValue> {
    println!("initializing sakviti-wasm");

    return Ok(JsValue::from("initialized"));
}

#[wasm_bindgen]
pub fn initialize_sync() -> Result<JsValue, JsValue> {
    println!("initializing sakviti-wasm");

    return Ok(JsValue::from("initialized"));
}

#[wasm_bindgen]
pub async fn create_transaction() -> Result<WasmTransaction, JsValue> {
    let saito = SAITO.lock().await;
    let wallet = saito.context.wallet.write().await;
    let transaction = wallet.create_transaction_with_default_fees().await;
    let wasm_transaction = WasmTransaction::from_transaction(transaction);
    return Ok(wasm_transaction);
}

#[wasm_bindgen]
pub async fn send_transaction(transaction: WasmTransaction) -> Result<JsValue, JsValue> {
    // todo : convert transaction

    let saito = SAITO.lock().await;
    // saito.blockchain_controller.
    Ok(JsValue::from("test"))
}

#[wasm_bindgen]
pub fn get_latest_block_hash() -> Result<JsValue, JsValue> {
    Ok(JsValue::from("latestblockhash"))
}

#[wasm_bindgen]
pub fn get_public_key() -> Result<JsValue, JsValue> {
    Ok(JsValue::from("publickey"))
}

#[wasm_bindgen]
pub async fn process_timer_event(duration: u64) {
    // println!("processing timer event : {:?}", duration);

    let mut saito = SAITO.lock().await;

    let duration = Duration::new(0, 1_000_000 * duration as u32);

    // blockchain controller
    let result = saito.receiver_in_blockchain.try_recv();
    if result.is_ok() {
        let event = result.unwrap();
        let result = saito.consensus_event_processor.process_event(event).await;
    }

    saito
        .consensus_event_processor
        .process_timer_event(duration.clone())
        .await;
    // mempool controller
    let result = saito.receiver_in_mempool.try_recv();
    if result.is_ok() {
        let event = result.unwrap();
        let result = saito.routing_event_processor.process_event(event).await;
    }
    saito
        .routing_event_processor
        .process_timer_event(duration.clone())
        .await;

    // miner controller
    let result = saito.receiver_in_miner.try_recv();
    if result.is_ok() {
        let event = result.unwrap();
        let result = saito.mining_event_processor.process_event(event).await;
    }
    saito
        .mining_event_processor
        .process_timer_event(duration.clone());
}
