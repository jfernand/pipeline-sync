mod sync_ticket;
mod protocol;
mod chain;
mod sha256;
mod merkle;
mod mmr;
mod mmr_sync;
mod mmr_v2;
mod mmr_v2_sync;

use crate::chain::{DeviceId, EventChain};
use crate::mmr_v2::MerkleRangeTreeV2;
use crate::protocol::{SyncRequest, SyncResponse};
use crate::sync_ticket::SyncTicket;
use iroh::{
    address_lookup::memory::MemoryLookup, endpoint::presets, protocol::Router, Endpoint,
    EndpointAddr,
};
use iroh_gossip::{api::Event, Gossip, TopicId};
use iroh_mdns_address_lookup::MdnsAddressLookup;
use n0_error::{AnyError, Result, StdResultExt};
use n0_future::StreamExt;
use std::env;
use std::str::{from_utf8, FromStr};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use iroh_gossip::api::{GossipReceiver, GossipSender};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args();
    // create an iroh endpoint that includes the standard address lookup mechanisms
    // we've built at number0

    let memory_lookup = MemoryLookup::new();
    let endpoint = Endpoint::builder(presets::N0)
        .address_lookup(memory_lookup.clone())
        .address_lookup(MdnsAddressLookup::builder())
        .bind()
        .await?;
    let router = if args.len() < 2 {
        start_endpoint(&endpoint).await?
    } else {
        let ticket = args
            .nth(1)
            .unwrap();
        let ticket = SyncTicket::from_str(&ticket).map_err(|e| {dbg!(&e);e.context("parse ticket")})?;

        for peer in &ticket.peers {
            memory_lookup.add_endpoint_info(peer.clone());
        }

        listen(ticket.topic, &endpoint, ticket.peers).await?
    };
    // clean shutdown makes sure that other peers are notified that you went offline
    router
        .shutdown()
        .await
        .std_context("shutdown router")?;
    Ok(())
}

async fn start_endpoint(endpoint: &Endpoint) -> Result<Router, AnyError> {
    let topic_id = topic_id("org.cr.pipeline");

    endpoint.online().await;

    let ticket = SyncTicket::new(topic_id, endpoint.clone());
    println!("Share this ticket to let others join the gossip topic: {ticket}");

    let router = listen(topic_id, endpoint, vec![]).await?;
    Ok(router)
}

async fn listen(
    topic_id: TopicId,
    endpoint: &Endpoint,
    bootstrap_peers: Vec<EndpointAddr>,
) -> Result<Router, AnyError> {
    let gossip = Gossip::builder().spawn(endpoint.clone());

    let router = Router::builder(endpoint.clone())
        .accept(iroh_gossip::ALPN, gossip.clone())
        .spawn();
    // then, you can subscribe to the topic and join your initial peers
    let peer_ids = bootstrap_peers.iter().map(|p| p.id).collect();
    let (sender, mut receiver) = gossip
        .subscribe(topic_id, peer_ids)
        .await?
        .split();

    receiver
        .joined()
        .await?;

    let message = format!("HELO {:?}", endpoint.id().to_string());
    sender
        .broadcast(
            message.as_bytes()
                .to_vec()
                .into(),
        )
        .await?;

    // demonstrate the sync wiring: ask whoever's listening for their MMR root
    let get_root = postcard::to_allocvec(&SyncRequest::GetRoot).std_context("encode sync request")?;
    sender
        .broadcast(get_root.into())
        .await
        .std_context("broadcast sync request")?;

    let tree = Arc::new(Mutex::new(MerkleRangeTreeV2::new()));
    let chain = Arc::new(Mutex::new(EventChain::new()));
    let device_id = DeviceId::random();
    let response_sender = sender.clone();
    let receive_tree = tree.clone();
    let receive_chain = chain.clone();

    let mut set: JoinSet<Result<(), AnyError>> = JoinSet::new();
    set.spawn(async move {
        // and read messages from others, answering any sync requests among them
        process_messages(&mut receiver, response_sender, receive_tree, receive_chain).await?;
        Ok(())
    });
    set.spawn(async move {
        // turn stdin lines into real chain events + MMR leaves, and broadcast them
        author_local_events(sender, tree, chain, device_id).await?;
        Ok(())
    });

    if let Some(res) = set.join_next().await {
        match res {
            Ok(_) => {}
            Err(err) => {
                println!("Error: {:?}", err);
            }
        }
    }
    Ok(router)
}

/// Read events off the topic, answering any `SyncRequest`s found among them
/// (via `protocol::dispatch`) against this node's local `tree`/`chain`, and
/// logging anything else as before.
async fn process_messages(
    receiver: &mut GossipReceiver,
    sender: GossipSender,
    tree: Arc<Mutex<MerkleRangeTreeV2>>,
    chain: Arc<Mutex<EventChain>>,
) -> Result<(), AnyError> {
    while let Some(event) = receiver
        .next()
        .await
    {
        if let Event::Received(message) = event? {
            if let Ok(request) = postcard::from_bytes::<SyncRequest>(&message.content) {
                let response = {
                    let tree = tree.lock().await;
                    let mut chain = chain.lock().await;
                    protocol::dispatch(&tree, &mut chain, request)
                };
                let bytes = postcard::to_allocvec(&response).std_context("encode sync response")?;
                sender
                    .broadcast(bytes.into())
                    .await
                    .std_context("broadcast sync response")?;
            } else if let Ok(response) = postcard::from_bytes::<SyncResponse>(&message.content) {
                println!("received sync response: {:?} from: {:?}", response, message.scope);
            } else {
                println!(
                    "received: {:?} from: {:?}",
                    from_utf8(&message.content),
                    message.scope
                );
            }
        }
    }
    Ok(())
}

/// Read lines from stdin and turn each into a real local event: appended to
/// `chain` (authored by this node's `device_id`) and to `tree` as an MMR leaf,
/// then broadcast to the topic as before.
async fn author_local_events(
    sender: GossipSender,
    tree: Arc<Mutex<MerkleRangeTreeV2>>,
    chain: Arc<Mutex<EventChain>>,
    device_id: DeviceId,
) -> Result<(), AnyError> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines
        .next_line()
        .await
        .std_context("read stdin")?
    {
        if line.is_empty() {
            continue;
        }

        let timestamp_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        {
            let mut chain = chain.lock().await;
            chain.add_event(line.clone(), device_id.clone(), timestamp_millis);
        }
        {
            let mut tree = tree.lock().await;
            tree.append(line.clone());
        }

        sender
            .broadcast(line.into_bytes().into())
            .await
            .std_context("broadcast local event")?;
    }
    Ok(())
}

fn topic_id(topic_name: &str) -> TopicId {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(topic_name.as_bytes());
    let topic_id_bytes = hasher.finalize();
    TopicId::from_bytes(topic_id_bytes.into())
}
