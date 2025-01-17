// Copyright (c) 2022 RBB S.r.l
// opensource@mintlayer.org
// SPDX-License-Identifier: MIT
// Licensed under the MIT License;
//  may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//  http://spdx.org/licenses/MIT
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Author(s): A. Altonen

use chainstate::{chainstate_interface::ChainstateInterface, BlockSource};
use common::{
    chain::{block::Block, config::ChainConfig},
    primitives::{Id, Idable},
};
use p2p::{
    error::P2pError,
    event::{PubSubControlEvent, SwarmEvent, SyncControlEvent},
    message::{Message, MessageType, SyncingMessage, SyncingRequest, SyncingResponse},
    net::{
        self, libp2p::Libp2pService, types::ConnectivityEvent, ConnectivityService,
        NetworkingService, SyncingCodecService,
    },
    sync::SyncManager,
    sync::SyncState,
};
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};
use tokio::sync::mpsc;

async fn make_sync_manager<T>(
    addr: T::Address,
    handle: subsystem::Handle<Box<dyn ChainstateInterface>>,
) -> (
    SyncManager<T>,
    T::ConnectivityHandle,
    mpsc::Sender<SyncControlEvent<T>>,
    mpsc::Receiver<PubSubControlEvent>,
    mpsc::Receiver<SwarmEvent<T>>,
)
where
    T: NetworkingService,
    T::ConnectivityHandle: ConnectivityService<T>,
    T::SyncingCodecHandle: SyncingCodecService<T>,
{
    let (tx_p2p_sync, rx_p2p_sync) = mpsc::channel(16);
    let (tx_pubsub, rx_pubsub) = mpsc::channel(16);
    let (tx_swarm, rx_swarm) = mpsc::channel(16);

    let config = Arc::new(common::chain::config::create_mainnet());
    let (conn, _, sync) = T::start(
        addr,
        &[],
        Arc::clone(&config),
        std::time::Duration::from_secs(10),
    )
    .await
    .unwrap();

    (
        SyncManager::<T>::new(
            Arc::clone(&config),
            sync,
            handle,
            rx_p2p_sync,
            tx_swarm,
            tx_pubsub,
        ),
        conn,
        tx_p2p_sync,
        rx_pubsub,
        rx_swarm,
    )
}

async fn connect_services<T>(conn1: &mut T::ConnectivityHandle, conn2: &mut T::ConnectivityHandle)
where
    T: NetworkingService,
    T::ConnectivityHandle: ConnectivityService<T>,
{
    let (_conn1_res, conn2_res) =
        tokio::join!(conn1.connect(conn2.local_addr().clone()), conn2.poll_next());
    let conn2_res: ConnectivityEvent<T> = conn2_res.unwrap();
    let _conn1_id = match conn2_res {
        ConnectivityEvent::IncomingConnection { peer_info, .. } => peer_info.peer_id,
        _ => panic!("invalid event received, expected incoming connection"),
    };
}

// initialize two blockchains which have the same longest chain
// that is `num_blocks` long
async fn init_chainstate_2(
    config: Arc<ChainConfig>,
    num_blocks: usize,
) -> (
    subsystem::Handle<Box<dyn ChainstateInterface>>,
    subsystem::Handle<Box<dyn ChainstateInterface>>,
) {
    let handle1 = test_utils::start_chainstate(Arc::clone(&config)).await;
    let handle2 = test_utils::start_chainstate(Arc::clone(&config)).await;
    let blocks =
        test_utils::create_n_blocks(Arc::clone(&config), config.genesis_block(), num_blocks);

    test_utils::import_blocks(&handle1, blocks.clone()).await;
    test_utils::import_blocks(&handle2, blocks).await;

    (handle1, handle2)
}

async fn init_chainstate_3(
    config: Arc<ChainConfig>,
    num_blocks: usize,
) -> (
    subsystem::Handle<Box<dyn ChainstateInterface>>,
    subsystem::Handle<Box<dyn ChainstateInterface>>,
    subsystem::Handle<Box<dyn ChainstateInterface>>,
) {
    let handle1 = test_utils::start_chainstate(Arc::clone(&config)).await;
    let handle2 = test_utils::start_chainstate(Arc::clone(&config)).await;
    let handle3 = test_utils::start_chainstate(Arc::clone(&config)).await;
    let blocks =
        test_utils::create_n_blocks(Arc::clone(&config), config.genesis_block(), num_blocks);

    test_utils::import_blocks(&handle1, blocks.clone()).await;
    test_utils::import_blocks(&handle2, blocks.clone()).await;
    test_utils::import_blocks(&handle3, blocks).await;

    (handle1, handle2, handle3)
}

async fn same_tip(
    handle1: &subsystem::Handle<Box<dyn ChainstateInterface>>,
    handle2: &subsystem::Handle<Box<dyn ChainstateInterface>>,
) -> bool {
    get_tip(handle1).await == get_tip(handle2).await
}

async fn get_tip(handle: &subsystem::Handle<Box<dyn ChainstateInterface>>) -> Id<Block> {
    handle.call(move |this| this.get_best_block_id()).await.unwrap().unwrap()
}

async fn process_header_request<T>(
    mgr: &mut SyncManager<T>,
    handle: &subsystem::Handle<Box<dyn ChainstateInterface>>,
) -> Result<(), P2pError>
where
    T: NetworkingService,
    T::SyncingCodecHandle: SyncingCodecService<T>,
{
    match mgr.handle_mut().poll_next().await.unwrap() {
        net::types::SyncingEvent::Request {
            peer_id: _,
            request_id,
            request:
                Message {
                    msg:
                        MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                            locator,
                        })),
                    magic,
                },
        } => {
            let headers =
                handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
            mgr.handle_mut()
                .send_response(
                    request_id,
                    Message {
                        magic,
                        msg: MessageType::Syncing(SyncingMessage::Response(
                            SyncingResponse::Headers { headers },
                        )),
                    },
                )
                .await
        }
        _ => panic!("invalid message"),
    }
}

async fn advance_mgr_state<T>(mgr: &mut SyncManager<T>) -> Result<(), P2pError>
where
    T: NetworkingService,
    T::SyncingCodecHandle: SyncingCodecService<T>,
{
    match mgr.handle_mut().poll_next().await.unwrap() {
        net::types::SyncingEvent::Request {
            peer_id,
            request_id,
            request:
                Message {
                    msg: MessageType::Syncing(SyncingMessage::Request(message)),
                    magic: _,
                },
        } => match message {
            SyncingRequest::GetHeaders { locator } => {
                mgr.process_header_request(peer_id, request_id, locator).await?;
            }
            SyncingRequest::GetBlocks { block_ids } => {
                mgr.process_block_request(peer_id, request_id, block_ids).await?;
            }
        },
        net::types::SyncingEvent::Response {
            peer_id,
            request_id: _,
            response:
                Message {
                    msg: MessageType::Syncing(SyncingMessage::Response(message)),
                    magic: _,
                },
        } => match message {
            SyncingResponse::Headers { headers } => {
                mgr.process_header_response(peer_id, headers).await?;
            }
            SyncingResponse::Blocks { blocks } => {
                mgr.process_block_response(peer_id, blocks).await?;
            }
        },
        net::types::SyncingEvent::Error {
            peer_id,
            request_id,
            error,
        } => {
            mgr.process_error(peer_id, request_id, error).await?;
        }
        _ => {}
    }

    mgr.check_state().await
}

#[tokio::test]
async fn local_and_remote_in_sync() {
    logging::init_logging::<&str>(None);

    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2) = init_chainstate_2(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;

    // connect the two managers together so that they can exchange messages
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));

    // ensure that only a header request is received from the remote and
    // as the nodes are tracking the same chain, no further messages are exchanged
    let (res1, res2) = tokio::join!(
        process_header_request(&mut mgr2, &mgr2_handle),
        advance_mgr_state(&mut mgr1)
    );

    assert_eq!(res1, Ok(()));
    assert_eq!(res2, Ok(()));

    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

// local and remote nodes are in the same chain but remote is ahead 7 blocks
//
// this the remote node is synced first and as it's ahead of local node,
// no blocks are downloaded whereas loca node downloads the 7 new blocks from remote
#[tokio::test]
async fn remote_ahead_by_7_blocks() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2) = init_chainstate_2(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;

    // add 7 more blocks on top of the best block (which is also known by mgr1)
    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    test_utils::add_more_blocks(Arc::clone(&config), &mgr2_handle, 7).await;
    assert!(!same_tip(&mgr1_handle, &mgr2_handle).await);

    // add peer to the hashmap of known peers and send getheaders request to them
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));

    let handle = tokio::spawn(async move {
        for _ in 0..9 {
            advance_mgr_state(&mut mgr1).await.unwrap();
        }

        mgr1
    });

    for _ in 0..9 {
        match mgr2.handle_mut().poll_next().await.unwrap() {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr2_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Headers { headers },
                            )),
                        },
                    )
                    .await
                    .unwrap()
            }
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                                block_ids,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(block_ids.len(), 1);
                let id = block_ids[0].clone();
                let blocks = vec![mgr2_handle
                    .call(move |this| this.get_block(id))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap()];
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Blocks { blocks },
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            net::types::SyncingEvent::Response {
                peer_id: _,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers: _,
                            })),
                        magic: _,
                    },
            } => {}
            msg => panic!("invalid message received: {:?}", msg),
        }
    }

    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

// local and remote nodes are in the same chain but local is ahead of remote by 12 blocks
#[tokio::test]
async fn local_ahead_by_12_blocks() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2) = init_chainstate_2(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _pubsub2, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;

    // add 12 more blocks on top of the best block (which is also known by mgr2)
    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    test_utils::add_more_blocks(Arc::clone(&config), &mgr1_handle, 12).await;
    assert!(!same_tip(&mgr1_handle, &mgr2_handle).await);

    // add peer to the hashmap of known peers and send getheaders request to them
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));
    assert_eq!(mgr2.register_peer(*conn1.peer_id()).await, Ok(()));

    let handle = tokio::spawn(async move {
        for _ in 0..14 {
            advance_mgr_state(&mut mgr1).await.unwrap();
        }

        mgr1
    });

    let mut work = VecDeque::new();

    loop {
        match mgr2.handle_mut().poll_next().await.unwrap() {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr2_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Headers { headers },
                            )),
                        },
                    )
                    .await
                    .unwrap()
            }
            net::types::SyncingEvent::Response {
                peer_id,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Blocks {
                                blocks,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(blocks.len(), 1);
                let block = blocks[0].clone();
                mgr2_handle
                    .call_mut(move |this| this.process_block(block, BlockSource::Peer))
                    .await
                    .unwrap()
                    .unwrap();

                if let Some(header) = work.pop_front() {
                    mgr2.handle_mut()
                        .send_request(
                            peer_id,
                            Message {
                                magic,
                                msg: MessageType::Syncing(SyncingMessage::Request(
                                    SyncingRequest::GetBlocks {
                                        block_ids: vec![header],
                                    },
                                )),
                            },
                        )
                        .await
                        .unwrap();
                } else {
                    // all blocks have been downloaded
                    break;
                }
            }
            net::types::SyncingEvent::Response {
                peer_id,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(headers.len(), 12);
                let headers = mgr2_handle
                    .call(move |this| this.filter_already_existing_blocks(headers))
                    .await
                    .unwrap()
                    .unwrap();
                work = headers.into_iter().map(|header| header.get_id()).collect::<VecDeque<_>>();
                let header = work.pop_front().unwrap();
                mgr2.handle_mut()
                    .send_request(
                        peer_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Request(
                                SyncingRequest::GetBlocks {
                                    block_ids: vec![header],
                                },
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            msg => panic!("invalid message received: {:?}", msg),
        }
    }

    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();
    mgr2.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

// local and remote nodes are in the same chain but local is ahead of remote by 14 blocks
// verify that remote nodes does a reorg
#[tokio::test]
async fn remote_local_diff_chains_local_higher() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2) = init_chainstate_2(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;

    // add 14 more blocks to local chain and 7 more blocks to remote chain
    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    test_utils::add_more_blocks(Arc::clone(&config), &mgr1_handle, 14).await;

    assert!(!same_tip(&mgr1_handle, &mgr2_handle).await);
    test_utils::add_more_blocks(Arc::clone(&config), &mgr2_handle, 7).await;

    // save local and remote tips so we can verify who did a reorg
    let local_tip = get_tip(&mgr1_handle).await;
    let remote_tip = get_tip(&mgr2_handle).await;

    // add peer to the hashmap of known peers and send getheaders request to them
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));
    assert_eq!(mgr2.register_peer(*conn1.peer_id()).await, Ok(()));

    let handle = tokio::spawn(async move {
        for _ in 0..24 {
            advance_mgr_state(&mut mgr1).await.unwrap();
        }

        mgr1
    });

    let mut work = VecDeque::new();

    for _ in 0..24 {
        match mgr2.handle_mut().poll_next().await.unwrap() {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr2_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Headers { headers },
                            )),
                        },
                    )
                    .await
                    .unwrap()
            }
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                                block_ids,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(block_ids.len(), 1);
                let id = block_ids[0].clone();
                let blocks = vec![mgr2_handle
                    .call(move |this| this.get_block(id))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap()];
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Blocks { blocks },
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            net::types::SyncingEvent::Response {
                peer_id,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Blocks {
                                blocks,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(blocks.len(), 1);
                let block = blocks[0].clone();
                mgr2_handle
                    .call_mut(move |this| this.process_block(block, BlockSource::Peer))
                    .await
                    .unwrap()
                    .unwrap();

                if let Some(header) = work.pop_front() {
                    mgr2.handle_mut()
                        .send_request(
                            peer_id,
                            Message {
                                magic,
                                msg: MessageType::Syncing(SyncingMessage::Request(
                                    SyncingRequest::GetBlocks {
                                        block_ids: vec![header],
                                    },
                                )),
                            },
                        )
                        .await
                        .unwrap();
                }
            }
            net::types::SyncingEvent::Response {
                peer_id,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers,
                            })),
                        magic,
                    },
            } => {
                let headers = mgr2_handle
                    .call(move |this| this.filter_already_existing_blocks(headers))
                    .await
                    .unwrap()
                    .unwrap();
                work = headers.into_iter().map(|header| header.get_id()).collect::<VecDeque<_>>();
                let header = work.pop_front().unwrap();
                mgr2.handle_mut()
                    .send_request(
                        peer_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Request(
                                SyncingRequest::GetBlocks {
                                    block_ids: vec![header],
                                },
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            msg => panic!("invalid message received: {:?}", msg),
        }
    }

    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();
    mgr2.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    assert!(get_tip(&mgr1_handle).await == local_tip);
    assert!(get_tip(&mgr2_handle).await != remote_tip);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

// local and remote nodes are in different chains and remote has longer chain
// verify that local node does a reorg
#[tokio::test]
async fn remote_local_diff_chains_remote_higher() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2) = init_chainstate_2(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _pubsub2, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;

    // add 5 more blocks to local chain and 12 more blocks to remote chain
    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    test_utils::add_more_blocks(Arc::clone(&config), &mgr1_handle, 5).await;

    assert!(!same_tip(&mgr1_handle, &mgr2_handle).await);
    test_utils::add_more_blocks(Arc::clone(&config), &mgr2_handle, 12).await;

    // save local and remote tips so we can verify who did a reorg
    let local_tip = get_tip(&mgr1_handle).await;
    let remote_tip = get_tip(&mgr2_handle).await;

    // add peer to the hashmap of known peers and send getheaders request to them
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));
    assert_eq!(mgr2.register_peer(*conn1.peer_id()).await, Ok(()));

    let handle = tokio::spawn(async move {
        for _ in 0..20 {
            advance_mgr_state(&mut mgr1).await.unwrap();
        }

        mgr1
    });

    let mut work = VecDeque::new();

    for _ in 0..20 {
        match mgr2.handle_mut().poll_next().await.unwrap() {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr2_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Headers { headers },
                            )),
                        },
                    )
                    .await
                    .unwrap()
            }
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                                block_ids,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(block_ids.len(), 1);
                let id = block_ids[0].clone();
                let blocks = vec![mgr2_handle
                    .call(move |this| this.get_block(id))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap()];
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Blocks { blocks },
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            net::types::SyncingEvent::Response {
                peer_id,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Blocks {
                                blocks,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(blocks.len(), 1);
                let block = blocks[0].clone();
                mgr2_handle
                    .call_mut(move |this| this.process_block(block, BlockSource::Peer))
                    .await
                    .unwrap()
                    .unwrap();

                if let Some(header) = work.pop_front() {
                    mgr2.handle_mut()
                        .send_request(
                            peer_id,
                            Message {
                                magic,
                                msg: MessageType::Syncing(SyncingMessage::Request(
                                    SyncingRequest::GetBlocks {
                                        block_ids: vec![header],
                                    },
                                )),
                            },
                        )
                        .await
                        .unwrap();
                }
            }
            net::types::SyncingEvent::Response {
                peer_id,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers,
                            })),
                        magic,
                    },
            } => {
                let headers = mgr2_handle
                    .call(move |this| this.filter_already_existing_blocks(headers))
                    .await
                    .unwrap()
                    .unwrap();
                work = headers.into_iter().map(|header| header.get_id()).collect::<VecDeque<_>>();
                let header = work.pop_front().unwrap();
                mgr2.handle_mut()
                    .send_request(
                        peer_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Request(
                                SyncingRequest::GetBlocks {
                                    block_ids: vec![header],
                                },
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            msg => panic!("invalid message received: {:?}", msg),
        }
    }

    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();
    mgr2.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    assert!(get_tip(&mgr1_handle).await != local_tip);
    assert!(get_tip(&mgr2_handle).await == remote_tip);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

#[tokio::test]
async fn two_remote_nodes_different_chains() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2, handle3) = init_chainstate_3(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();
    let mgr3_handle = handle3.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;
    let (mut mgr3, mut conn3, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle3)
            .await;

    // add 5 more blocks for first remote and 7 blocks to second remote
    test_utils::add_more_blocks(Arc::clone(&config), &mgr2_handle, 5).await;
    test_utils::add_more_blocks(Arc::clone(&config), &mgr3_handle, 7).await;

    // save local and remote tips so we can verify who did a reorg
    let mgr2_tip = get_tip(&mgr2_handle).await;
    let mgr3_tip = get_tip(&mgr3_handle).await;

    // connect remote peers to local peer
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    connect_services::<Libp2pService>(&mut conn1, &mut conn3).await;

    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));
    assert_eq!(mgr1.register_peer(*conn3.peer_id()).await, Ok(()));
    assert_eq!(mgr2.register_peer(*conn1.peer_id()).await, Ok(()));
    assert_eq!(mgr3.register_peer(*conn1.peer_id()).await, Ok(()));

    let handle = tokio::spawn(async move {
        for _ in 0..18 {
            advance_mgr_state(&mut mgr1).await.unwrap();
        }

        mgr1
    });

    for _ in 0..18 {
        let (event, dest_peer_id, mgr_handle) = tokio::select! {
            event = mgr2.handle_mut().poll_next() => { (event.unwrap(), conn2.peer_id(), &mgr2_handle) },
            event = mgr3.handle_mut().poll_next() => { (event.unwrap(), conn3.peer_id(), &mgr3_handle) },
        };

        match event {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                let msg = Message {
                    magic,
                    msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                        headers,
                    })),
                };

                if dest_peer_id == conn2.peer_id() {
                    mgr2.handle_mut().send_response(request_id, msg).await.unwrap()
                } else {
                    mgr3.handle_mut().send_response(request_id, msg).await.unwrap()
                }
            }
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                                block_ids,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(block_ids.len(), 1);
                let id = block_ids[0].clone();
                let msg = Message {
                    magic,
                    msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Blocks {
                        blocks: vec![mgr_handle
                            .call(move |this| this.get_block(id))
                            .await
                            .unwrap()
                            .unwrap()
                            .unwrap()],
                    })),
                };

                if dest_peer_id == conn2.peer_id() {
                    mgr2.handle_mut().send_response(request_id, msg).await.unwrap();
                } else {
                    mgr3.handle_mut().send_response(request_id, msg).await.unwrap();
                }
            }
            net::types::SyncingEvent::Response {
                peer_id: _,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers: _,
                            })),
                        magic: _,
                    },
            } => {}
            msg => panic!("invalid message received: {:?}", msg),
        }
    }
    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr3_handle).await);
    assert!(get_tip(&mgr2_handle).await == mgr2_tip);
    assert!(get_tip(&mgr3_handle).await == mgr3_tip);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

#[tokio::test]
async fn two_remote_nodes_same_chains() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2, handle3) = init_chainstate_3(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();
    let mgr3_handle = handle3.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;
    let (mut mgr3, mut conn3, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle3)
            .await;

    // add the same 32 new blocks for both mgr2 and mgr3
    let id = mgr2_handle.call(move |this| this.get_best_block_id()).await.unwrap().unwrap();
    let parent = mgr2_handle.call(move |this| this.get_block(id)).await.unwrap().unwrap();
    let blocks = test_utils::create_n_blocks(Arc::clone(&config), &parent.unwrap(), 32);

    test_utils::import_blocks(&mgr2_handle, blocks.clone()).await;
    test_utils::import_blocks(&mgr3_handle, blocks).await;

    // save local and remote tips so we can verify who did a reorg
    let mgr2_tip = get_tip(&mgr2_handle).await;
    let mgr3_tip = get_tip(&mgr3_handle).await;

    assert!(same_tip(&mgr2_handle, &mgr3_handle).await);
    assert!(!same_tip(&mgr2_handle, &mgr1_handle).await);

    // connect remote peers to local peer
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    connect_services::<Libp2pService>(&mut conn1, &mut conn3).await;

    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));
    assert_eq!(mgr1.register_peer(*conn3.peer_id()).await, Ok(()));
    assert_eq!(mgr2.register_peer(*conn1.peer_id()).await, Ok(()));
    assert_eq!(mgr3.register_peer(*conn1.peer_id()).await, Ok(()));

    let (tx, mut rx) = mpsc::channel(1);
    let handle = tokio::spawn(async move {
        loop {
            advance_mgr_state(&mut mgr1).await.unwrap();

            if mgr1.state() == &SyncState::Idle {
                break;
            }
        }

        tx.send(()).await.unwrap();
        mgr1
    });

    loop {
        let (event, dest_peer_id, mgr_handle) = tokio::select! {
            event = mgr2.handle_mut().poll_next() => { (event.unwrap(), conn2.peer_id(), &mgr2_handle) },
            event = mgr3.handle_mut().poll_next() => { (event.unwrap(), conn3.peer_id(), &mgr3_handle) },
            _event = rx.recv() => { break },
        };

        match event {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                let msg = Message {
                    magic,
                    msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                        headers,
                    })),
                };

                if dest_peer_id == conn2.peer_id() {
                    mgr2.handle_mut().send_response(request_id, msg).await.unwrap()
                } else {
                    mgr3.handle_mut().send_response(request_id, msg).await.unwrap()
                }
            }
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                                block_ids,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(block_ids.len(), 1);
                let id = block_ids[0].clone();
                let msg = Message {
                    magic,
                    msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Blocks {
                        blocks: vec![mgr_handle
                            .call(move |this| this.get_block(id))
                            .await
                            .unwrap()
                            .unwrap()
                            .unwrap()],
                    })),
                };

                if dest_peer_id == conn2.peer_id() {
                    mgr2.handle_mut().send_response(request_id, msg).await.unwrap();
                } else {
                    mgr3.handle_mut().send_response(request_id, msg).await.unwrap();
                }
            }
            net::types::SyncingEvent::Response {
                peer_id: _,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers: _,
                            })),
                        magic: _,
                    },
            } => {}
            msg => panic!("invalid message received: {:?}", msg),
        }
    }
    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr3_handle).await);
    assert!(get_tip(&mgr2_handle).await == mgr2_tip);
    assert!(get_tip(&mgr3_handle).await == mgr3_tip);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

#[tokio::test]
async fn two_remote_nodes_same_chains_new_blocks() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2, handle3) = init_chainstate_3(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();
    let mgr3_handle = handle3.clone();

    let (mut mgr1, mut conn1, _, mut pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;
    let (mut mgr3, mut conn3, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle3)
            .await;

    // add the same 32 new blocks for both mgr2 and mgr3
    let id = mgr2_handle.call(move |this| this.get_best_block_id()).await.unwrap().unwrap();
    let parent = mgr2_handle.call(move |this| this.get_block(id)).await.unwrap().unwrap();
    let blocks = test_utils::create_n_blocks(Arc::clone(&config), &parent.unwrap(), 32);

    test_utils::import_blocks(&mgr2_handle, blocks.clone()).await;
    test_utils::import_blocks(&mgr3_handle, blocks).await;

    // connect remote peers to local peer
    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    connect_services::<Libp2pService>(&mut conn1, &mut conn3).await;

    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));
    assert_eq!(mgr1.register_peer(*conn3.peer_id()).await, Ok(()));
    assert_eq!(mgr2.register_peer(*conn1.peer_id()).await, Ok(()));
    assert_eq!(mgr3.register_peer(*conn1.peer_id()).await, Ok(()));

    let (tx, mut rx) = mpsc::channel(1);
    let mut gethdr_received = HashSet::new();
    let mut blocks = vec![];

    let handle = tokio::spawn(async move {
        loop {
            advance_mgr_state(&mut mgr1).await.unwrap();

            if mgr1.state() == &SyncState::Idle {
                break;
            }
        }

        tx.send(()).await.unwrap();
        mgr1
    });

    loop {
        let (event, dest_peer_id, mgr_handle) = tokio::select! {
            event = mgr2.handle_mut().poll_next() => { (event.unwrap(), conn2.peer_id(), &mgr2_handle) },
            event = mgr3.handle_mut().poll_next() => { (event.unwrap(), conn3.peer_id(), &mgr3_handle) },
            _event = rx.recv() => { break },
        };

        match event {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                let msg = Message {
                    magic,
                    msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                        headers,
                    })),
                };

                if dest_peer_id == conn2.peer_id() {
                    mgr2.handle_mut().send_response(request_id, msg).await.unwrap()
                } else {
                    mgr3.handle_mut().send_response(request_id, msg).await.unwrap()
                }

                if gethdr_received.insert(dest_peer_id) {
                    if blocks.is_empty() {
                        let parent = mgr2_handle
                            .call(move |this| this.get_block(this.get_best_block_id().unwrap()))
                            .await
                            .unwrap()
                            .unwrap();
                        blocks =
                            test_utils::create_n_blocks(Arc::clone(&config), &parent.unwrap(), 10);
                    }

                    if dest_peer_id == conn2.peer_id() {
                        test_utils::import_blocks(&mgr2_handle, blocks.clone()).await;
                    } else {
                        test_utils::import_blocks(&mgr3_handle, blocks.clone()).await;
                    }
                }
            }
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                                block_ids,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(block_ids.len(), 1);
                let id = block_ids[0].clone();
                let msg = Message {
                    magic,
                    msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Blocks {
                        blocks: vec![mgr_handle
                            .call(move |this| this.get_block(id))
                            .await
                            .unwrap()
                            .unwrap()
                            .unwrap()],
                    })),
                };

                if dest_peer_id == conn2.peer_id() {
                    mgr2.handle_mut().send_response(request_id, msg).await.unwrap();
                } else {
                    mgr3.handle_mut().send_response(request_id, msg).await.unwrap();
                }
            }
            net::types::SyncingEvent::Response {
                peer_id: _,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers: _,
                            })),
                        magic: _,
                    },
            } => {}
            msg => panic!("invalid message received: {:?}", msg),
        }
    }
    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr3_handle).await);
    assert!(same_tip(&mgr2_handle, &mgr3_handle).await);
    assert_eq!(mgr1.state(), &SyncState::Idle);
    assert_eq!(
        pubsub.try_recv(),
        Ok(PubSubControlEvent::InitialBlockDownloadDone),
    );
}

// connect two nodes, they are in sync so no blocks are downloaded
// then disconnect them, add more blocks to remote chains and reconnect the nodes
// verify that local node downloads the blocks and after that they are in sync
#[tokio::test]
async fn test_connect_disconnect_resyncing() {
    let config = Arc::new(common::chain::config::create_unit_test_config());
    let (handle1, handle2) = init_chainstate_2(Arc::clone(&config), 8).await;
    let mgr1_handle = handle1.clone();
    let mgr2_handle = handle2.clone();

    let (mut mgr1, mut conn1, _, _pubsub, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle1)
            .await;
    let (mut mgr2, mut conn2, _, _, _) =
        make_sync_manager::<Libp2pService>(test_utils::make_address("/ip6/::1/tcp/"), handle2)
            .await;

    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));

    // ensure that only a header request is received from the remote and
    // as the nodes are tracking the same chain, no further messages are exchanged
    let (res1, res2) = tokio::join!(
        process_header_request(&mut mgr2, &mgr2_handle),
        advance_mgr_state(&mut mgr1)
    );

    assert_eq!(res1, Ok(()));
    assert_eq!(res2, Ok(()));

    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    assert_eq!(mgr1.state(), &SyncState::Idle);

    mgr1.unregister_peer(*conn2.peer_id());
    assert_eq!(conn1.disconnect(*conn2.peer_id()).await, Ok(()));
    assert!(std::matches!(
        conn2.poll_next().await,
        Ok(ConnectivityEvent::ConnectionClosed { .. })
    ));

    let parent = mgr1_handle
        .call(move |this| {
            let id = this.get_best_block_id().unwrap();
            this.get_block(id)
        })
        .await
        .unwrap()
        .unwrap();
    let blocks = test_utils::create_n_blocks(Arc::clone(&config), &parent.unwrap(), 7);
    test_utils::import_blocks(&mgr2_handle, blocks.clone()).await;

    connect_services::<Libp2pService>(&mut conn1, &mut conn2).await;
    assert_eq!(mgr1.register_peer(*conn2.peer_id()).await, Ok(()));

    let handle = tokio::spawn(async move {
        for _ in 0..9 {
            advance_mgr_state(&mut mgr1).await.unwrap();
        }

        mgr1
    });

    for _ in 0..9 {
        match mgr2.handle_mut().poll_next().await.unwrap() {
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                                locator,
                            })),
                        magic,
                    },
            } => {
                let headers =
                    mgr2_handle.call(move |this| this.get_headers(locator)).await.unwrap().unwrap();
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Headers { headers },
                            )),
                        },
                    )
                    .await
                    .unwrap()
            }
            net::types::SyncingEvent::Request {
                peer_id: _,
                request_id,
                request:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                                block_ids,
                            })),
                        magic,
                    },
            } => {
                assert_eq!(block_ids.len(), 1);
                let id = block_ids[0].clone();
                let blocks = vec![mgr2_handle
                    .call(move |this| this.get_block(id))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap()];
                mgr2.handle_mut()
                    .send_response(
                        request_id,
                        Message {
                            magic,
                            msg: MessageType::Syncing(SyncingMessage::Response(
                                SyncingResponse::Blocks { blocks },
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            net::types::SyncingEvent::Response {
                peer_id: _,
                request_id: _,
                response:
                    Message {
                        msg:
                            MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                                headers: _,
                            })),
                        magic: _,
                    },
            } => {}
            msg => panic!("invalid message received: {:?}", msg),
        }
    }

    let mut mgr1 = handle.await.unwrap();
    mgr1.check_state().await.unwrap();

    assert!(same_tip(&mgr1_handle, &mgr2_handle).await);
    assert_eq!(mgr1.state(), &SyncState::Idle);
}
