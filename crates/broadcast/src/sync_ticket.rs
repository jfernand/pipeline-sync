use iroh::{Endpoint, EndpointAddr};
use iroh_gossip::proto::TopicId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncTicket {
    pub topic: TopicId,
    pub peers: Vec<EndpointAddr>,
}

use anyhow::Result;
use std::str::FromStr;

impl FromStr for SyncTicket {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if !s.starts_with("pipeline_") {
            return Err(anyhow::anyhow!(
                "Invalid ticket format. Tickets start with 'pipeline_'"
            ));
        }
        let s = s.replace("pipeline_", "");
        let s = s.to_ascii_uppercase();

        let bytes = data_encoding::BASE32_NOPAD.decode(s.as_bytes())?;
        let ticket: SyncTicket = postcard::from_bytes(&bytes)?;
        Ok(ticket)
    }
}

impl SyncTicket {
    pub fn new(topic: TopicId, endpoint: Endpoint) -> Self {
        SyncTicket {
            topic,
            peers: vec![
                endpoint
                    .addr(),
            ],
        }
    }
}

impl Display for SyncTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Serialize the struct to bytes using postcard
        let bytes = postcard::to_allocvec(self).map_err(|_| fmt::Error)?;
        // Encode the bytes into a clean base32 string
        let base32_string = data_encoding::BASE32_NOPAD.encode(&bytes);
        write!(f, "pipeline_{}", base32_string.to_lowercase())
    }
}

pub fn topic_id(topic_name: &str) -> TopicId {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(topic_name.as_bytes());
    let topic_id_bytes = hasher.finalize();
    TopicId::from_bytes(topic_id_bytes.into())
}

#[cfg(test)]
mod tests {
    use crate::sync_ticket::SyncTicket;
    use super::topic_id;
    use iroh::address_lookup::memory::MemoryLookup;
    use iroh::endpoint::presets;
    use iroh::protocol::Router;
    use iroh::Endpoint;
    use iroh_gossip::api::Event;
    use iroh_gossip::Gossip;
    use n0_future::StreamExt;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_sync_ticket() {
        let ticket = SyncTicket::new(
            topic_id("test"),
            Endpoint::builder(presets::N0).bind().await.unwrap(),
        );
        let ticket_str = ticket.to_string();
        let parsed_ticket = SyncTicket::from_str(&ticket_str).unwrap();
        assert_eq!(ticket.to_string(), parsed_ticket.to_string());
    }

    #[tokio::test]
    async fn test_gossip_broadcast_with_ticket() {
        let topic = topic_id("test-topic");

        let memory_lookup1 = MemoryLookup::new();
        let endpoint1 = Endpoint::builder(presets::N0)
            .address_lookup(memory_lookup1.clone())
            .bind()
            .await
            .unwrap();

        let gossip1 = Gossip::builder().spawn(endpoint1.clone());
        let _router1 = Router::builder(endpoint1.clone())
            .accept(iroh_gossip::ALPN, gossip1.clone())
            .spawn();

        let (sender1, mut receiver1) = gossip1.subscribe(topic, vec![]).await.unwrap().split();

        let ticket = SyncTicket::new(topic, endpoint1.clone());
        let ticket_str = ticket.to_string();
        let parsed_ticket = SyncTicket::from_str(&ticket_str).unwrap();

        let memory_lookup2 = MemoryLookup::new();
        for peer in &parsed_ticket.peers {
            memory_lookup2.add_endpoint_info(peer.clone());
        }

        let endpoint2 = Endpoint::builder(presets::N0)
            .address_lookup(memory_lookup2.clone())
            .bind()
            .await
            .unwrap();

        let gossip2 = Gossip::builder().spawn(endpoint2.clone());
        let _router2 = Router::builder(endpoint2.clone())
            .accept(iroh_gossip::ALPN, gossip2.clone())
            .spawn();

        let peer_ids = parsed_ticket.peers.iter().map(|p| p.id).collect();
        let (sender2, mut receiver2) = gossip2
            .subscribe(parsed_ticket.topic, peer_ids)
            .await
            .unwrap()
            .split();

        // Wait for both to join
        tokio::try_join!(receiver1.joined(), receiver2.joined()).unwrap();

        // Node 2 sends a message
        sender2
            .broadcast(b"hello from node 2".to_vec().into())
            .await
            .unwrap();

        // Node 1 receives the message
        let event = tokio::time::timeout(std::time::Duration::from_secs(5), receiver1.next())
            .await
            .expect("timeout waiting for message")
            .expect("stream ended")
            .unwrap();

        if let Event::Received(msg) = event {
            assert_eq!(&msg.content[..], b"hello from node 2");
        } else {
            panic!("Expected Received event, got {:?}", event);
        }

        // Node 1 sends a message
        sender1
            .broadcast(b"hello from node 1".to_vec().into())
            .await
            .unwrap();

        // Node 2 receives the message
        let event2 = tokio::time::timeout(std::time::Duration::from_secs(5), receiver2.next())
            .await
            .expect("timeout waiting for message")
            .expect("stream ended")
            .unwrap();

        if let Event::Received(msg) = event2 {
            assert_eq!(&msg.content[..], b"hello from node 1");
        } else {
            panic!("Expected Received event, got {:?}", event2);
        }
    }
}
