use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::{debug, trace};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

use crate::common::command::{GlobalEvent, NetworkEvent};
use crate::common::defs::SaitoHash;
use crate::common::keep_time::KeepTime;
use crate::common::process_event::ProcessEvent;
use crate::core::blockchain_controller::MempoolEvent;
use crate::core::data::miner::Miner;
use crate::core::routing_controller::RoutingEvent;

const MINER_INTERVAL: u128 = 100_000;

#[derive(Debug)]
pub enum MinerEvent {
    LongestChainBlockAdded { hash: SaitoHash, difficulty: u64 },
}

pub struct MinerController {
    pub miner: Arc<RwLock<Miner>>,
    pub sender_to_blockchain: Sender<RoutingEvent>,
    pub sender_to_mempool: Sender<MempoolEvent>,
    pub time_keeper: Box<dyn KeepTime + Send + Sync>,
    pub miner_timer: u128,
    pub new_miner_event_received: bool,
}

impl MinerController {}

#[async_trait]
impl ProcessEvent<MinerEvent> for MinerController {
    async fn process_global_event(&mut self, _event: GlobalEvent) -> Option<()> {
        debug!("processing new global event");
        None
    }

    async fn process_network_event(&mut self, _event: NetworkEvent) -> Option<()> {
        debug!("processing new interface event");

        None
    }

    async fn process_timer_event(&mut self, duration: Duration) -> Option<()> {
        // trace!("processing timer event : {:?}", duration.as_micros());

        if self.new_miner_event_received {
            self.miner_timer += duration.as_micros();
            if self.miner_timer > MINER_INTERVAL {
                self.miner_timer = 0;
                self.new_miner_event_received = false;
                let miner = self.miner.read().await;
                miner.mine(self.sender_to_mempool.clone()).await;
            }
        }

        None
    }

    async fn process_event(&mut self, event: MinerEvent) -> Option<()> {
        debug!("event received : {:?}", event);
        match event {
            MinerEvent::LongestChainBlockAdded { hash, difficulty } => {
                trace!("waiting for the miner read lock");
                let mut miner = self.miner.write().await;
                trace!("acquired the miner read lock");
                miner.difficulty = difficulty;
                miner.target = hash;
                self.new_miner_event_received = true;
            }
        }
        None
    }

    async fn on_init(&mut self) {}
}
