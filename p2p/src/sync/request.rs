// Copyright (c) 2022 RBB S.r.l
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

//! Utility functions for sending header/block requests/responses

use super::*;

/// Request type
pub enum RequestType {
    /// Header request
    GetHeaders,

    /// Block request
    GetBlocks(Vec<Id<Block>>),
}

/// Request state
pub struct RequestState<T: NetworkingService> {
    /// Unique ID of the remote peer
    pub(super) _peer_id: T::PeerId,

    /// Request type
    pub(super) request_type: RequestType,

    /// How many times the request has been sent
    pub(super) retry_count: usize,
}

impl<T> SyncManager<T>
where
    T: NetworkingService,
    T::SyncingCodecHandle: SyncingCodecService<T>,
{
    /// Make a block request message
    ///
    /// Return `GetBlocks` message and an associated `GetBlocks` request type entry
    /// that is used to track the progress of the request.
    ///
    /// # Arguments
    /// * `block_ids` - IDs of the blocks that are requested
    pub fn make_block_request(&self, block_ids: Vec<Id<Block>>) -> (Message, RequestType) {
        (
            Message {
                magic: *self.config.magic_bytes(),
                msg: MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetBlocks {
                    block_ids: block_ids.clone(),
                })),
            },
            RequestType::GetBlocks(block_ids),
        )
    }

    /// Make header request message
    ///
    /// Return `GetHeaders` message and an associated `GetHeaders` request type entry
    /// that is used to track the progress of the request.
    ///
    /// # Arguments
    /// * `locator` - locator object that shows the state of the local node
    pub fn make_header_request(&self, locator: Vec<BlockHeader>) -> (Message, RequestType) {
        (
            Message {
                magic: *self.config.magic_bytes(),
                msg: MessageType::Syncing(SyncingMessage::Request(SyncingRequest::GetHeaders {
                    locator,
                })),
            },
            RequestType::GetHeaders,
        )
    }

    /// Make header response
    ///
    /// # Arguments
    /// * `headers` - the headers that were requested
    pub fn make_header_response(&self, headers: Vec<BlockHeader>) -> Message {
        Message {
            magic: *self.config.magic_bytes(),
            msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Headers {
                headers,
            })),
        }
    }

    /// Make block response
    ///
    /// # Arguments
    /// * `blocks` - the blocks that were requested
    pub fn make_block_response(&self, blocks: Vec<Block>) -> Message {
        Message {
            magic: *self.config.magic_bytes(),
            msg: MessageType::Syncing(SyncingMessage::Response(SyncingResponse::Blocks { blocks })),
        }
    }

    /// Helper function for sending a request to remote
    ///
    /// Send request to remote and create [`RequestState`] entry which tracks how many
    /// times the request has failed. If the number of resends is more than the configured
    /// limit, the request is deemed failed and connection to the peer is closed.
    ///
    /// # Arguments
    /// * `peer_id` - peer ID of the remote node
    /// * `message` - [`crate::message::Message`] containing the request
    /// * `request_type` - [`RequestType`] indicating the type, used for tracking progress
    /// * `retry_count` - how many times the request has been resent
    async fn send_request(
        &mut self,
        peer_id: T::PeerId,
        message: Message,
        request_type: RequestType,
        retry_count: usize,
    ) -> crate::Result<()> {
        let request_id = self.handle.send_request(peer_id, message).await?;
        self.requests.insert(
            request_id,
            RequestState {
                _peer_id: peer_id,
                request_type,
                retry_count,
            },
        );
        Ok(())
    }

    /// Send block request to remote peer
    ///
    /// Send block request to remote peer and update the state to `UploadingBlocks`.
    /// For now only one block can be requested at a time
    ///
    /// # Arguments
    /// * `peer_id` - peer ID of the remote node
    /// * `block_id` - ID of the block that is requested
    /// * `retry_count` - how many times the request has been resent
    pub async fn send_block_request(
        &mut self,
        peer_id: T::PeerId,
        block_id: Id<Block>,
        retry_count: usize,
    ) -> crate::Result<()> {
        ensure!(
            self.peers.contains_key(&peer_id),
            P2pError::PeerError(PeerError::PeerDoesntExist),
        );

        // send request to remote peer and start tracking its progress
        let (wanted_blocks, request_type) = self.make_block_request(vec![block_id.clone()]);
        self.send_request(peer_id, wanted_blocks, request_type, retry_count).await?;

        self.peers
            .get_mut(&peer_id)
            .ok_or(P2pError::PeerError(PeerError::PeerDoesntExist))?
            .set_state(peer::PeerSyncState::UploadingBlocks(block_id));
        Ok(())
    }

    /// Send header request to remote peer
    ///
    /// Send header request to remote peer and update the state to `UploadingHeaders`.
    /// For now the number of headers is limited to one header
    ///
    /// # Arguments
    /// * `peer_id` - peer ID of the remote node
    /// * `locator` - local locator object
    /// * `retry_count` - how many times the request has been resent
    pub async fn send_header_request(
        &mut self,
        peer_id: T::PeerId,
        locator: Vec<BlockHeader>,
        retry_count: usize,
    ) -> crate::Result<()> {
        ensure!(
            self.peers.contains_key(&peer_id),
            P2pError::PeerError(PeerError::PeerDoesntExist),
        );

        // send header request and start tracking its progress
        let (wanted_headers, request_type) = self.make_header_request(locator.clone());
        self.send_request(peer_id, wanted_headers, request_type, retry_count).await?;

        self.peers
            .get_mut(&peer_id)
            .ok_or(P2pError::PeerError(PeerError::PeerDoesntExist))?
            .set_state(peer::PeerSyncState::UploadingHeaders(locator));
        Ok(())
    }

    /// Send header response to remote peer
    ///
    /// The header request that is removed from remote peer contains
    /// a locator object. Local node uses this object to find common
    ///
    /// # Arguments
    /// * `request_id` - ID of the request that this is a response to
    /// * `headers` - headers that the remote requested
    pub async fn send_header_response(
        &mut self,
        request_id: T::RequestId,
        headers: Vec<BlockHeader>,
    ) -> crate::Result<()> {
        // TODO: save sent header IDs somewhere and validate future requests against those?
        let message = self.make_header_response(headers);
        self.handle.send_response(request_id, message).await
    }

    /// Send header response to remote peer
    ///
    /// The header request that is removed from remote peer contains
    /// a locator object. Local node uses this object to find common
    ///
    /// # Arguments
    /// * `request_id` - ID of the request that this is a response to
    /// * `headers` - headers that the remote requested
    pub async fn send_block_response(
        &mut self,
        request_id: T::RequestId,
        blocks: Vec<Block>,
    ) -> crate::Result<()> {
        // TODO: save sent block IDs somewhere and validate future requests against those?
        let message = self.make_block_response(blocks);
        self.handle.send_response(request_id, message).await
    }
}
