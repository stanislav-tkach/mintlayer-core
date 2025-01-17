// Copyright (c) 2018 Parity Technologies (UK) Ltd.
// Copyright (c) 2021 Protocol Labs
// Copyright (c) 2021-2022 RBB S.r.l
// opensource@mintlayer.org
// SPDX-License-Identifier: MIT
// Licensed under the MIT License;
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://spdx.org/licenses/MIT
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Author(s): A. Altonen
use crate::{
    error::{ConversionError, DialError, P2pError, ProtocolError, PublishError},
    message,
    net::{
        self,
        libp2p::sync::{SyncRequest, SyncResponse},
        types::{ConnectivityEvent, PubSubEvent, PubSubTopic, SyncingEvent},
        ConnectivityService, NetworkingService, PubSubService, SyncingCodecService,
    },
};
use async_trait::async_trait;
use itertools::*;
use libp2p::{
    core::{upgrade, PeerId},
    gossipsub::MessageId,
    identify::IdentifyInfo,
    identity, mplex,
    multiaddr::Protocol,
    noise,
    request_response::*,
    swarm::SwarmBuilder,
    tcp::TcpConfig,
    Multiaddr, Transport,
};
use logging::log;
use serialization::{Decode, Encode};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use utils::ensure;

mod backend;
mod constants;
mod sync;
mod tests;
mod types;

pub mod behaviour;

/// libp2p-specifc peer discovery strategies
#[derive(Debug, PartialEq, Eq)]
pub enum Libp2pDiscoveryStrategy {
    /// Use mDNS to find peers in the local network
    MulticastDns,
}

#[derive(Debug)]
pub struct Libp2pService;

pub struct Libp2pConnectivityHandle<T>
where
    T: NetworkingService,
{
    /// Address where the network services has been bound
    bind_addr: Multiaddr,

    /// Peer Id of the local node
    peer_id: PeerId,

    /// Channel for sending commands to libp2p backend
    cmd_tx: mpsc::Sender<types::Command>,

    /// Channel for receiving connectivity events from libp2p backend
    conn_rx: mpsc::Receiver<types::ConnectivityEvent>,
    _marker: std::marker::PhantomData<fn() -> T>,
}

pub struct Libp2pPubSubHandle<T>
where
    T: NetworkingService,
{
    /// Channel for sending commands to libp2p backend
    cmd_tx: mpsc::Sender<types::Command>,

    /// Channel for receiving pubsub events from libp2p backend
    gossip_rx: mpsc::Receiver<types::PubSubEvent>,
    _marker: std::marker::PhantomData<fn() -> T>,
}

pub struct Libp2pSyncHandle<T>
where
    T: NetworkingService,
{
    /// Channel for sending commands to libp2p backend
    cmd_tx: mpsc::Sender<types::Command>,

    /// Channel for receiving pubsub events from libp2p backend
    sync_rx: mpsc::Receiver<types::SyncingEvent>,
    _marker: std::marker::PhantomData<fn() -> T>,
}

/// Verify that the discovered multiaddress is in a format that Mintlayer supports:
///   /ip4/<IPv4 address>/tcp/<TCP port>/p2p/<PeerId multihash>
///   /ip6/<IPv6 address>/tcp/<TCP port>/p2p/<PeerId multihash>
///
/// Documentation for libp2p-mdns doesn't mention if `peer_addr` includes the PeerId
/// so if it doesn't, add it. Otherwise just return the address
pub fn parse_discovered_addr(peer_id: PeerId, peer_addr: Multiaddr) -> Option<Multiaddr> {
    let mut components = peer_addr.iter();

    if !std::matches!(components.next(), Some(Protocol::Ip4(_) | Protocol::Ip6(_))) {
        return None;
    }

    if !std::matches!(components.next(), Some(Protocol::Tcp(_))) {
        return None;
    }

    match components.next() {
        Some(Protocol::P2p(_)) => Some(peer_addr.clone()),
        None => Some(peer_addr.with(Protocol::P2p(peer_id.into()))),
        Some(_) => None,
    }
}

/// Get the network layer protocol from `addr`
fn get_addr_from_multiaddr(addr: &Multiaddr) -> Option<Protocol> {
    addr.iter().next()
}

impl<T> FromIterator<(PeerId, Multiaddr)> for net::types::AddrInfo<T>
where
    T: NetworkingService<PeerId = PeerId, Address = Multiaddr>,
{
    fn from_iter<I: IntoIterator<Item = (PeerId, Multiaddr)>>(iter: I) -> Self {
        let mut entry = net::types::AddrInfo {
            id: PeerId::random(),
            ip4: Vec::new(),
            ip6: Vec::new(),
        };

        iter.into_iter().for_each(|(id, addr)| {
            entry.id = id;
            match get_addr_from_multiaddr(&addr) {
                Some(Protocol::Ip4(_)) => entry.ip4.push(addr),
                Some(Protocol::Ip6(_)) => entry.ip6.push(addr),
                _ => panic!("parse_discovered_addr() failed!"),
            }
        });

        log::trace!(
            "id {:?}, ipv4 {:#?}, ipv6 {:#?}",
            entry.id,
            entry.ip4,
            entry.ip6
        );

        entry
    }
}

/// Parse all discovered addresses and group them by PeerId
pub fn parse_peers<T>(mut peers: Vec<(PeerId, Multiaddr)>) -> Vec<net::types::AddrInfo<T>>
where
    T: NetworkingService<PeerId = PeerId, Address = Multiaddr>,
{
    peers.sort_by(|a, b| a.0.cmp(&b.0));
    peers
        .into_iter()
        .map(|(id, addr)| (id, parse_discovered_addr(id, addr)))
        .filter_map(|(id, addr)| addr.map(|addr| (id, addr)))
        .group_by(|info| info.0)
        .into_iter()
        .map(|(_id, addrs)| net::types::AddrInfo::from_iter(addrs))
        .collect::<Vec<net::types::AddrInfo<T>>>()
}

impl<T> TryInto<net::types::PeerInfo<T>> for IdentifyInfo
where
    T: NetworkingService<PeerId = PeerId, ProtocolId = String>,
{
    type Error = P2pError;

    fn try_into(self) -> Result<net::types::PeerInfo<T>, Self::Error> {
        let proto = self.protocol_version.clone();
        let (version, magic_bytes) =
            match sscanf::scanf!(proto, "/{}/{}.{}.{}-{:x}", String, u8, u8, u16, u32) {
                Err(_err) => Err(P2pError::ProtocolError(ProtocolError::InvalidProtocol)),
                Ok((proto, maj, min, pat, magic)) => {
                    if proto != "mintlayer" {
                        return Err(P2pError::ProtocolError(ProtocolError::InvalidProtocol));
                    }

                    Ok((
                        common::primitives::semver::SemVer::new(maj, min, pat),
                        magic.to_le_bytes(),
                    ))
                }
            }?;

        Ok(net::types::PeerInfo {
            peer_id: PeerId::from_public_key(&self.public_key),
            magic_bytes,
            version,
            agent: Some(self.agent_version),
            protocols: self.protocols,
        })
    }
}

#[async_trait]
impl NetworkingService for Libp2pService {
    type Address = Multiaddr;
    type DiscoveryStrategy = Libp2pDiscoveryStrategy;
    type PeerId = PeerId;
    type ProtocolId = String;
    type RequestId = RequestId;
    type MessageId = MessageId;
    type ConnectivityHandle = Libp2pConnectivityHandle<Self>;
    type PubSubHandle = Libp2pPubSubHandle<Self>;
    type SyncingCodecHandle = Libp2pSyncHandle<Self>;

    async fn start(
        bind_addr: Self::Address,
        strategies: &[Self::DiscoveryStrategy],
        chain_config: Arc<common::chain::ChainConfig>,
        timeout: std::time::Duration,
    ) -> crate::Result<(
        Self::ConnectivityHandle,
        Self::PubSubHandle,
        Self::SyncingCodecHandle,
    )> {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = id_keys.public().to_peer_id();
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&id_keys)
            .map_err(|_| P2pError::Other("Failed to create Noise keys"))?;

        let transport = TcpConfig::new()
            .nodelay(true)
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
            .multiplex(mplex::MplexConfig::new())
            .outbound_timeout(timeout)
            .boxed();

        // If mDNS has been specified as a peer discovery strategy for this Libp2pService,
        // pass that information to the backend so it knows to relay the mDNS events to P2P
        let relay_mdns = strategies.iter().any(|s| s == &Libp2pDiscoveryStrategy::MulticastDns);
        log::trace!("multicast dns enabled {}", relay_mdns);

        let swarm = SwarmBuilder::new(
            transport,
            behaviour::Libp2pBehaviour::new(Arc::clone(&chain_config), id_keys, relay_mdns).await,
            peer_id,
        )
        .build();

        let (cmd_tx, cmd_rx) = mpsc::channel(constants::CHANNEL_SIZE);
        let (gossip_tx, gossip_rx) = mpsc::channel(constants::CHANNEL_SIZE);
        let (conn_tx, conn_rx) = mpsc::channel(constants::CHANNEL_SIZE);
        let (sync_tx, sync_rx) = mpsc::channel(constants::CHANNEL_SIZE);

        // run the libp2p backend in a background task
        log::debug!("spawning libp2p backend to background");

        tokio::spawn(async move {
            let mut backend = backend::Backend::new(swarm, cmd_rx, conn_tx, gossip_tx, sync_tx);
            backend.run().await
        });

        // send listen command to the libp2p backend and if it succeeds,
        // create a multiaddress for local peer and return the Libp2pService object
        let (tx, rx) = oneshot::channel();
        cmd_tx
            .send(types::Command::Listen {
                addr: bind_addr.clone(),
                response: tx,
            })
            .await?;
        rx.await?
            .map_err(|_| P2pError::DialError(DialError::IoError(std::io::ErrorKind::AddrInUse)))?;

        Ok((
            Self::ConnectivityHandle {
                bind_addr: bind_addr.with(Protocol::P2p(peer_id.into())),
                peer_id,
                cmd_tx: cmd_tx.clone(),
                conn_rx,
                _marker: Default::default(),
            },
            Self::PubSubHandle {
                cmd_tx: cmd_tx.clone(),
                gossip_rx,
                _marker: Default::default(),
            },
            Self::SyncingCodecHandle {
                cmd_tx,
                sync_rx,
                _marker: Default::default(),
            },
        ))
    }
}

#[async_trait]
impl<T> ConnectivityService<T> for Libp2pConnectivityHandle<T>
where
    T: NetworkingService<Address = Multiaddr, PeerId = PeerId> + Send,
    IdentifyInfo: TryInto<net::types::PeerInfo<T>, Error = P2pError>,
{
    async fn connect(&mut self, addr: T::Address) -> crate::Result<()> {
        log::debug!("try to establish outbound connection, address {:?}", addr);

        let peer_id = match addr.iter().last() {
            Some(Protocol::P2p(hash)) => PeerId::from_multihash(hash).map_err(|_| {
                P2pError::ConversionError(ConversionError::InvalidAddress(addr.to_string()))
            })?,
            _ => {
                return Err(P2pError::ConversionError(ConversionError::InvalidAddress(
                    addr.to_string(),
                )))
            }
        };

        // try to connect to remote peer
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(types::Command::Connect {
                peer_id,
                peer_addr: addr.clone(),
                response: tx,
            })
            .await?;

        rx.await.map_err(P2pError::from)?.map_err(P2pError::from)
    }

    async fn disconnect(&mut self, peer_id: T::PeerId) -> crate::Result<()> {
        log::debug!("disconnect peer {:?}", peer_id);

        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(types::Command::Disconnect {
                peer_id,
                response: tx,
            })
            .await?;
        rx.await.map_err(P2pError::from)?.map_err(P2pError::from)
    }

    fn local_addr(&self) -> &T::Address {
        &self.bind_addr
    }

    fn peer_id(&self) -> &T::PeerId {
        &self.peer_id
    }

    async fn poll_next(&mut self) -> crate::Result<ConnectivityEvent<T>> {
        match self.conn_rx.recv().await.ok_or(P2pError::ChannelClosed)? {
            types::ConnectivityEvent::ConnectionAccepted { addr, peer_info } => {
                Ok(ConnectivityEvent::ConnectionAccepted {
                    addr,
                    peer_info: (*peer_info).try_into()?,
                })
            }
            types::ConnectivityEvent::ConnectionError { addr, error } => {
                Ok(ConnectivityEvent::ConnectionError { addr, error })
            }
            types::ConnectivityEvent::IncomingConnection { addr, peer_info } => {
                Ok(ConnectivityEvent::IncomingConnection {
                    addr,
                    peer_info: (*peer_info).try_into()?,
                })
            }
            types::ConnectivityEvent::ConnectionClosed { peer_id } => {
                Ok(ConnectivityEvent::ConnectionClosed { peer_id })
            }
            types::ConnectivityEvent::Discovered { peers } => Ok(ConnectivityEvent::Discovered {
                peers: parse_peers(peers),
            }),
            types::ConnectivityEvent::Expired { peers } => Ok(ConnectivityEvent::Expired {
                peers: parse_peers(peers),
            }),
            types::ConnectivityEvent::Disconnected { peer_id } => {
                Ok(ConnectivityEvent::Disconnected { peer_id })
            }
            types::ConnectivityEvent::Error { peer_id, error } => {
                Ok(ConnectivityEvent::Error { peer_id, error })
            }
            types::ConnectivityEvent::Misbehaved { peer_id, behaviour } => {
                Ok(ConnectivityEvent::Misbehaved { peer_id, behaviour })
            }
        }
    }
}

#[async_trait]
impl<T> PubSubService<T> for Libp2pPubSubHandle<T>
where
    T: NetworkingService<PeerId = PeerId, MessageId = MessageId> + Send,
{
    async fn publish(&mut self, message: message::Message) -> crate::Result<()> {
        let encoded = message.encode();
        ensure!(
            encoded.len() <= constants::GOSSIPSUB_MAX_TRANSMIT_SIZE,
            P2pError::PublishError(PublishError::MessageTooLarge(
                Some(encoded.len()),
                Some(constants::GOSSIPSUB_MAX_TRANSMIT_SIZE),
            ))
        );

        // TODO: add support for transactions in the future
        let topic =
            if let message::MessageType::PubSub(message::PubSubMessage::Block(_)) = message.msg {
                net::types::PubSubTopic::Blocks
            } else {
                return Err(P2pError::ProtocolError(ProtocolError::InvalidMessage));
            };

        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(types::Command::SendMessage {
                topic,
                message: encoded,
                response: tx,
            })
            .await?;

        rx.await
            .map_err(|e| e)? // channel closed
            .map_err(|e| e) // command failure
    }

    async fn report_validation_result(
        &mut self,
        source: T::PeerId,
        message_id: T::MessageId,
        result: net::types::ValidationResult,
    ) -> crate::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(types::Command::ReportValidationResult {
                message_id,
                source,
                result: result.into(),
                response: tx,
            })
            .await?;

        rx.await
            .map_err(|e| e)? // channel closed
            .map_err(|e| e) // command failure
    }

    async fn subscribe(&mut self, topics: &[PubSubTopic]) -> crate::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(types::Command::Subscribe {
                topics: topics.iter().map(|topic| topic.into()).collect::<Vec<_>>(),
                response: tx,
            })
            .await?;

        rx.await
            .map_err(|e| e)? // channel closed
            .map_err(|e| e) // command failure
    }

    async fn poll_next(&mut self) -> crate::Result<PubSubEvent<T>> {
        match self.gossip_rx.recv().await.ok_or(P2pError::ChannelClosed)? {
            types::PubSubEvent::MessageReceived {
                peer_id,
                message,
                message_id,
            } => Ok(PubSubEvent::MessageReceived {
                peer_id,
                message,
                message_id,
            }),
        }
    }
}

#[async_trait]
impl<T> SyncingCodecService<T> for Libp2pSyncHandle<T>
where
    T: NetworkingService<PeerId = PeerId, MessageId = MessageId, RequestId = RequestId> + Send,
{
    async fn send_request(
        &mut self,
        peer_id: T::PeerId,
        message: message::Message,
    ) -> crate::Result<T::RequestId> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(types::Command::SendRequest {
                peer_id,
                request: Box::new(SyncRequest::new(message.encode())),
                response: tx,
            })
            .await?;

        rx.await
            .map_err(|e| e)? // channel closed
            .map_err(|e| e) // command failure
    }

    async fn send_response(
        &mut self,
        request_id: T::RequestId,
        message: message::Message,
    ) -> crate::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(types::Command::SendResponse {
                request_id,
                response: Box::new(SyncResponse::new(message.encode())),
                channel: tx,
            })
            .await?;

        rx.await
            .map_err(|e| e)? // channel closed
            .map_err(|e| e) // command failure
    }

    async fn poll_next(&mut self) -> crate::Result<SyncingEvent<T>> {
        match self.sync_rx.recv().await.ok_or(P2pError::ChannelClosed)? {
            types::SyncingEvent::Request {
                peer_id,
                request_id,
                request,
            } => {
                let request = message::Message::decode(&mut &(*request)[..]).map_err(|err| {
                    log::error!(
                        "invalid request received from peer {:?}: {:?}",
                        peer_id,
                        err
                    );
                    P2pError::ProtocolError(ProtocolError::InvalidMessage)
                })?;

                Ok(SyncingEvent::Request {
                    peer_id,
                    request_id,
                    request,
                })
            }
            types::SyncingEvent::Response {
                peer_id,
                request_id,
                response,
            } => {
                let response = message::Message::decode(&mut &(*response)[..]).map_err(|err| {
                    log::error!(
                        "invalid response received from peer {:?}: {:?}",
                        peer_id,
                        err
                    );
                    P2pError::ProtocolError(ProtocolError::InvalidMessage)
                })?;

                Ok(SyncingEvent::Response {
                    peer_id,
                    request_id,
                    response,
                })
            }
            types::SyncingEvent::Error {
                peer_id,
                request_id,
                error,
            } => Ok(SyncingEvent::Error {
                peer_id,
                request_id,
                error,
            }),
        }
    }
}
