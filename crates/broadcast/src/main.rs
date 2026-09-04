mod sync_ticket;
mod protocol;
mod chain;
mod sha256;
mod merkle;

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
use iroh_gossip::api::{GossipReceiver, GossipSender};
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
    let (mut sender, mut receiver) = gossip
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
    let mut set: JoinSet<Result<(), AnyError>> = JoinSet::new();
    set.spawn(async move {
        // and read messages from others
        process_messages(&mut receiver).await?;
        Ok(())
    });
    set.spawn(async move {
        // and read messages from others
        send_messages(&mut sender).await?;
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

async fn process_messages(receiver: &mut GossipReceiver) -> Result<(), AnyError> {
    while let Some(event) = receiver
        .next()
        .await
    {
        if let Event::Received(message) = event? {
            println!(
                "received: {:?} from: {:?}",
                from_utf8(&message.content),
                message.scope
            );
        }
    }
    Ok(())
}

async fn send_messages(sender: &mut GossipSender) -> Result<(), AnyError> {
    sender.broadcast(
        "Hello".as_bytes()
            .to_vec()
            .into(),
    ).await?;
    Ok(())
}

fn topic_id(topic_name: &str) -> TopicId {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(topic_name.as_bytes());
    let topic_id_bytes = hasher.finalize();
    TopicId::from_bytes(topic_id_bytes.into())
}
