// // Copyright (c) 2021 Protocol Labs
// // Copyright (c) 2022 RBB S.r.l
// // opensource@mintlayer.org
// // SPDX-License-Identifier: MIT
// // Licensed under the MIT License;
// // you may not use this file except in compliance with the License.
// // You may obtain a copy of the License at
// //
// // 	http://spdx.org/licenses/MIT
// //
// // Unless required by applicable law or agreed to in writing, software
// // distributed under the License is distributed on an "AS IS" BASIS,
// // WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// // See the License for the specific language governing permissions and
// // limitations under the License.
// //
// // Author(s): A. Altonen
use super::*;
use crate::net::libp2p::{behaviour, types::*};
use futures::StreamExt;
use libp2p::{gossipsub::IdentTopic as Topic, Multiaddr};
use serialization::Encode;

impl PartialEq for types::PubSubEvent {
    fn eq(&self, other: &Self) -> bool {
        let types::PubSubEvent::MessageReceived {
            peer_id: p1,
            message_id: m1,
            message: msg1,
        } = self;
        let types::PubSubEvent::MessageReceived {
            peer_id: p2,
            message_id: m2,
            message: msg2,
        } = other;

        (p1 == p2) && (m1 == m2) && (msg1 == msg2)
    }
}

#[tokio::test]
async fn test_invalid_message() {
    let addr: Multiaddr = test_utils::make_address("/ip6/::1/tcp/");
    let (mut backend1, _cmd1, _conn1, _gossip, _sync1) = make_libp2p(
        common::chain::config::create_mainnet(),
        addr.clone(),
        &[net::types::PubSubTopic::Blocks],
        false,
    )
    .await;

    let (mut backend2, _cmd2, _conn2, _gossip2, _sync2) = make_libp2p(
        common::chain::config::create_mainnet(),
        test_utils::make_address("/ip6/::1/tcp/"),
        &[net::types::PubSubTopic::Blocks],
        false,
    )
    .await;

    connect_swarms::<behaviour::Libp2pBehaviour, behaviour::Libp2pBehaviour>(
        addr,
        &mut backend1.swarm,
        &mut backend2.swarm,
    )
    .await;

    while backend1
        .swarm
        .behaviour_mut()
        .gossipsub
        .publish(Topic::new("mintlayer-gossipsub-blocks"), vec![999].encode())
        .is_err()
    {
        tokio::select! {
            event = backend1.swarm.select_next_some() =>
                if let SwarmEvent::ConnectionEstablished { .. } = event { break },
            event = backend2.swarm.select_next_some() =>
                if let SwarmEvent::ConnectionEstablished { .. } = event { break },
            _event = tokio::time::sleep(std::time::Duration::from_millis(500)) => {},
        }
    }

    loop {
        tokio::select! {
            _event = backend1.swarm.select_next_some() => {},
            event = backend2.swarm.select_next_some() =>
                if let SwarmEvent::Behaviour(Libp2pBehaviourEvent::Connectivity(
                    ConnectivityEvent::Misbehaved { .. })
                ) = event {
                    break;
            },
            _event = tokio::time::sleep(std::time::Duration::from_millis(500)) => {},
        }

        let _ = backend1
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(Topic::new("mintlayer-gossipsub-blocks"), vec![999].encode());
    }
}

#[tokio::test]
async fn test_gossipsub_not_supported() {
    let addr: Multiaddr = test_utils::make_address("/ip6/::1/tcp/");
    let config = common::chain::config::create_mainnet();
    let (mut backend1, _cmd, _conn_rx, _gossip_rx, _sync_rx) = make_libp2p(
        config.clone(),
        addr.clone(),
        &[net::types::PubSubTopic::Blocks],
        false,
    )
    .await;

    let (transport, peer_id, id_keys) = make_transport_and_keys();
    let mut swarm = SwarmBuilder::new(transport, make_identify(config, id_keys), peer_id).build();

    connect_swarms::<behaviour::Libp2pBehaviour, Identify>(addr, &mut backend1.swarm, &mut swarm)
        .await;

    loop {
        tokio::select! {
            event = backend1.swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(Libp2pBehaviourEvent::Connectivity(
                    ConnectivityEvent::Misbehaved { .. })
                ) => {
                    break;
                }
                _event => {},
            },
            _event = swarm.next() => {},
            _event = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                panic!("didn't receive subscriptions in time");
            }
        }
    }
}
