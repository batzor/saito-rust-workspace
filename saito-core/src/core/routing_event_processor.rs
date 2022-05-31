use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::{debug, info, trace};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

use crate::common::command::NetworkEvent;
use crate::common::defs::SaitoHash;
use crate::common::interface_io::InterfaceIO;
use crate::common::keep_time::KeepTime;
use crate::common::process_event::ProcessEvent;
use crate::core::consensus_event_processor::ConsensusEvent;
use crate::core::data;
use crate::core::data::blockchain::Blockchain;
use crate::core::data::configuration::Configuration;
use crate::core::data::msg::block_request::BlockchainRequest;
use crate::core::data::msg::message::Message;
use crate::core::data::network::Network;
use crate::core::data::peer::Peer;
use crate::core::data::wallet::Wallet;
use crate::core::mining_event_processor::MiningEvent;

#[derive(Debug)]
pub enum RoutingEvent {}

#[derive(Debug)]
pub enum PeerState {
    Connected,
    Connecting,
    Disconnected,
}

pub struct StaticPeer {
    pub peer_details: data::configuration::PeerConfig,
    pub peer_state: PeerState,
    pub peer_index: u64,
}

/// Manages peers and routes messages to correct controller
pub struct RoutingEventProcessor {
    pub blockchain: Arc<RwLock<Blockchain>>,
    pub sender_to_mempool: Sender<ConsensusEvent>,
    pub sender_to_miner: Sender<MiningEvent>,
    // TODO : remove this if not needed
    pub static_peers: Vec<StaticPeer>,
    pub configs: Arc<RwLock<Configuration>>,
    pub time_keeper: Box<dyn KeepTime + Send + Sync>,
    pub wallet: Arc<RwLock<Wallet>>,
    pub network: Network,
}

impl RoutingEventProcessor {
    ///
    ///
    /// # Arguments
    ///
    /// * `peer_index`:
    /// * `message`:
    ///
    /// returns: ()
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// ```
    async fn process_incoming_message(&mut self, peer_index: u64, message: Message) {
        debug!(
            "processing incoming message type : {:?} from peer : {:?}",
            message.get_type_value(),
            peer_index
        );
        match message {
            Message::HandshakeChallenge(challenge) => {
                debug!("received handshake challenge");
                let mut peers = self.network.peers.write().await;
                let peer = peers.index_to_peers.get_mut(&peer_index);
                if peer.is_none() {
                    todo!()
                }
                let peer = peer.unwrap();
                peer.handle_handshake_challenge(
                    challenge,
                    &self.network.io_interface,
                    self.wallet.clone(),
                    self.configs.clone(),
                )
                .await
                .unwrap();
            }
            Message::HandshakeResponse(response) => {
                debug!("received handshake response");
                let mut peers = self.network.peers.write().await;
                let peer = peers.index_to_peers.get_mut(&peer_index);
                if peer.is_none() {
                    todo!()
                }
                let peer = peer.unwrap();
                peer.handle_handshake_response(
                    response,
                    &self.network.io_interface,
                    self.wallet.clone(),
                )
                .await
                .unwrap();
                if peer.handshake_done {
                    debug!(
                        "peer : {:?} handshake successful for peer : {:?}",
                        peer.peer_index,
                        hex::encode(peer.peer_public_key)
                    );
                    // start block syncing here
                    self.request_blockchain_from_peer(peer_index).await;
                }
            }
            Message::HandshakeCompletion(response) => {
                debug!("received handshake completion");
                let mut peers = self.network.peers.write().await;
                let peer = peers.index_to_peers.get_mut(&peer_index);
                if peer.is_none() {
                    todo!()
                }
                let peer = peer.unwrap();
                let result = peer
                    .handle_handshake_completion(response, &self.network.io_interface)
                    .await;
                if peer.handshake_done {
                    debug!(
                        "peer : {:?} handshake successful for peer : {:?}",
                        peer.peer_index,
                        hex::encode(peer.peer_public_key)
                    );
                    // start block syncing here
                    self.request_blockchain_from_peer(peer_index).await;
                }
            }
            Message::ApplicationMessage(_) => {
                debug!("received buffer");
            }
            Message::Block(_) => {
                debug!("received block");
            }
            Message::Transaction(_) => {
                debug!("received transaction");
            }
            Message::BlockchainRequest(request) => {
                self.process_incoming_blockchain_request(request, peer_index)
                    .await;
            }
            Message::BlockHeaderHash(hash) => {
                self.process_incoming_block_hash(hash, peer_index).await;
            }
        }
        debug!("incoming message processed");
    }

    // async fn propagate_block_to_peers(&self, block_hash: SaitoHash) {
    //     debug!("propagating blocks to peers");
    //     let buffer: Vec<u8>;
    //     let mut exceptions = vec![];
    //     {
    //         trace!("waiting for the blockchain write lock");
    //         let blockchain = self.blockchain.read().await;
    //         trace!("acquired the blockchain write lock");
    //         let block = blockchain.blocks.get(&block_hash);
    //         if block.is_none() {
    //             // TODO : handle
    //         }
    //         let block = block.unwrap();
    //         buffer = block.serialize_for_net(BlockType::Header);
    //
    //         // finding block sender to avoid resending the block to that node
    //         if block.source_connection_id.is_some() {
    //             trace!("waiting for the peers read lock");
    //             let peers = self.peers.read().await;
    //             trace!("acquired the peers read lock");
    //             let peer = peers
    //                 .address_to_peers
    //                 .get(&block.source_connection_id.unwrap());
    //             if peer.is_some() {
    //                 exceptions.push(*peer.unwrap());
    //             }
    //         }
    //     }
    //
    //     self.io_handler
    //         .send_message_to_all(buffer, exceptions)
    //         .await
    //         .unwrap();
    //     debug!("block sent to peers");
    // }

    async fn connect_to_static_peers(&mut self) {
        debug!("connect to peers from config",);
        trace!("waiting for the configs read lock");
        let configs = self.configs.read().await;
        trace!("acquired the configs read lock");

        for peer in &configs.peers {
            self.network
                .io_interface
                .connect_to_peer(peer.clone())
                .await
                .unwrap();
        }
        debug!("connected to peers");
    }
    async fn handle_new_peer(
        &mut self,
        peer_data: Option<data::configuration::PeerConfig>,
        peer_index: u64,
    ) {
        // TODO : if an incoming peer is same as static peer, handle the scenario
        debug!("handing new peer : {:?}", peer_index);
        trace!("waiting for the peers write lock");
        let mut peers = self.network.peers.write().await;
        trace!("acquired the peers write lock");
        // for mut static_peer in &mut self.static_peers {
        //     if static_peer.peer_details == peer {
        //         static_peer.peer_state = PeerState::Connected;
        //     }
        // }
        let mut peer = Peer::new(peer_index);
        peer.static_peer_config = peer_data;

        if peer.static_peer_config.is_none() {
            // if we don't have peer data it means this is an incoming connection. so we initiate the handshake
            peer.initiate_handshake(
                &self.network.io_interface,
                self.wallet.clone(),
                self.configs.clone(),
            )
            .await
            .unwrap();
        }

        peers.index_to_peers.insert(peer_index, peer);
        info!("new peer added : {:?}", peer_index);
    }

    async fn handle_peer_disconnect(&mut self, peer_index: u64) {
        trace!("handling peer disconnect, peer_index = {}", peer_index);
        let peers = self.network.peers.read().await;
        let result = peers.find_peer_by_index(peer_index);

        if result.is_some() {
            let peer = result.unwrap();

            if peer.static_peer_config.is_some() {
                // This means the connection has been initiated from this side, therefore we must
                // try to re-establish the connection again
                // TODO : Add a delay so that there won't be a runaway issue with connects and
                // disconnects, check the best place to add (here or network_controller)
                info!(
                    "Static peer disconnected, reconnecting .., Peer ID = {}, Public Key = {:?}",
                    peer.peer_index,
                    hex::encode(peer.peer_public_key)
                );

                self.network
                    .io_interface
                    .connect_to_peer(peer.static_peer_config.as_ref().unwrap().clone())
                    .await
                    .unwrap();
            } else {
                info!("Peer disconnected, expecting a reconnection from the other side, Peer ID = {}, Public Key = {:?}",
                    peer.peer_index, hex::encode(peer.peer_public_key));
            }
        } else {
            todo!("Handle the unknown peer disconnect");
        }
    }

    async fn request_blockchain_from_peer(&self, peer_index: u64) {
        debug!("requesting blockchain from peer : {:?}", peer_index);

        // TODO : should this be moved inside peer ?
        let request;
        {
            let blockchain = self.blockchain.read().await;
            request = BlockchainRequest {
                latest_block_id: blockchain.get_latest_block_id(),
                latest_block_hash: blockchain.get_latest_block_hash(),
                fork_id: blockchain.get_fork_id(),
            };
        }

        let buffer = Message::BlockchainRequest(request).serialize();
        self.network
            .io_interface
            .send_message(peer_index, buffer)
            .await
            .unwrap();
    }

    pub async fn process_incoming_blockchain_request(
        &self,
        request: BlockchainRequest,
        peer_index: u64,
    ) {
        debug!(
            "processing incoming blockchain request : {:?}-{:?}-{:?} from peer : {:?}",
            request.latest_block_id,
            hex::encode(request.latest_block_hash),
            hex::encode(request.fork_id),
            peer_index
        );
        // TODO : can we ignore the functionality if it's a lite node ?

        let blockchain = self.blockchain.read().await;

        let last_shared_ancestor =
            blockchain.generate_last_shared_ancestor(request.latest_block_id, request.fork_id);
        debug!("last shared ancestor = {:?}", last_shared_ancestor);

        for i in last_shared_ancestor..(blockchain.blockring.get_latest_block_id() + 1) {
            let block_hash = blockchain
                .blockring
                .get_longest_chain_block_hash_by_block_id(i);
            if block_hash == [0; 32] {
                // TODO : can the block hash not be in the ring if we are going through the longest chain ?
                continue;
            }
            let buffer = Message::BlockHeaderHash(block_hash).serialize();
            self.network
                .io_interface
                .send_message(peer_index, buffer)
                .await
                .unwrap();
        }
    }
    async fn process_incoming_block_hash(&self, block_hash: SaitoHash, peer_index: u64) {
        debug!(
            "processing incoming block hash : {:?} from peer : {:?}",
            hex::encode(block_hash),
            peer_index
        );

        let block_exists;
        {
            let blockchain = self.blockchain.read().await;
            block_exists = blockchain.is_block_indexed(block_hash);
        }
        let url;
        {
            let peers = self.network.peers.read().await;
            let peer = peers
                .index_to_peers
                .get(&peer_index)
                .expect("peer not found");
            url = peer.get_block_fetch_url(block_hash);
        }
        if !block_exists {
            self.network
                .io_interface
                .fetch_block_from_peer(block_hash, peer_index, url)
                .await
                .unwrap();
        }
    }
}

#[async_trait]
impl ProcessEvent<RoutingEvent> for RoutingEventProcessor {
    async fn process_network_event(&mut self, event: NetworkEvent) -> Option<()> {
        debug!("processing new interface event");
        match event {
            NetworkEvent::OutgoingNetworkMessage {
                peer_index: _,
                buffer: _,
            } => {
                // TODO : remove this case if not being used
                unreachable!()
            }
            NetworkEvent::IncomingNetworkMessage { peer_index, buffer } => {
                debug!("incoming message received from peer : {:?}", peer_index);
                let message = Message::deserialize(buffer);
                if message.is_err() {
                    todo!()
                }
                self.process_incoming_message(peer_index, message.unwrap())
                    .await;
            }
            NetworkEvent::PeerConnectionResult {
                peer_details,
                result,
            } => {
                if result.is_ok() {
                    self.handle_new_peer(peer_details, result.unwrap()).await;
                }
            }
            NetworkEvent::PeerDisconnected { peer_index } => {
                self.handle_peer_disconnect(peer_index).await;
            }

            NetworkEvent::OutgoingNetworkMessageForAll { .. } => {
                unreachable!()
            }
            NetworkEvent::ConnectToPeer { .. } => {
                unreachable!()
            }
            NetworkEvent::BlockFetchRequest { .. } => {
                unreachable!()
            }
            NetworkEvent::BlockFetched {
                block_hash,
                peer_index,
                buffer,
            } => {
                debug!("block received : {:?}", hex::encode(block_hash));
                self.sender_to_mempool
                    .send(ConsensusEvent::BlockFetched { peer_index, buffer })
                    .await
                    .unwrap();
            }
        }
        None
    }
    async fn process_timer_event(&mut self, _duration: Duration) -> Option<()> {
        // trace!("processing timer event : {:?}", duration.as_micros());

        None
    }

    async fn process_event(&mut self, _event: RoutingEvent) -> Option<()> {
        debug!("processing blockchain event");

        // match event {}

        debug!("blockchain event processed successfully");
        None
    }

    async fn on_init(&mut self) {
        // connect to peers
        self.connect_to_static_peers().await;
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn process_new_transaction() {}
}