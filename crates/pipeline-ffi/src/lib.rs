//! Packaging spike for PL-025: the smallest uniffi surface that proves the native library loads
//! from Kotlin and that iroh runs inside it. A [SyncNode] binds an iroh endpoint and answers an
//! echo protocol; [SyncNode::echo] dials another node by its endpoint id and round-trips a
//! message, through whatever path iroh finds (direct, mDNS on the LAN, or n0's relays).

use std::str::FromStr;
use std::sync::Arc;

use iroh::endpoint::{Connection, presets};
use iroh::protocol::{AcceptError, ProtocolHandler, Router};
use iroh::{Endpoint, EndpointId};
use iroh_mdns_address_lookup::MdnsAddressLookup;

uniffi::setup_scaffolding!();

const ECHO_ALPN: &[u8] = b"pipeline/spike-echo/0";

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SpikeError {
    #[error("{0}")]
    Failed(String),
}

fn failed(e: impl std::fmt::Display) -> SpikeError {
    SpikeError::Failed(e.to_string())
}

/// The library's version, callable without starting anything: the first proof that it loaded.
#[uniffi::export]
pub fn spike_version() -> String {
    format!("pipeline-ffi {}", env!("CARGO_PKG_VERSION"))
}

#[derive(uniffi::Object)]
pub struct SyncNode {
    router: Router,
}

#[uniffi::export(async_runtime = "tokio")]
impl SyncNode {
    /// Binds an endpoint with n0's relays and DNS lookup plus mDNS, and starts answering echoes.
    #[uniffi::constructor]
    pub async fn start() -> Result<Arc<Self>, SpikeError> {
        let endpoint = Endpoint::builder(presets::N0)
            .address_lookup(MdnsAddressLookup::builder())
            .bind()
            .await
            .map_err(failed)?;
        let router = Router::builder(endpoint).accept(ECHO_ALPN, Echo).spawn();
        Ok(Arc::new(Self { router }))
    }

    pub fn endpoint_id(&self) -> String {
        self.router.endpoint().id().to_string()
    }

    /// Waits until the endpoint has a home relay, so others can reach it by id.
    pub async fn wait_online(&self) {
        self.router.endpoint().online().await;
    }

    /// Dials `peer` by endpoint id, sends `message` and returns what came back.
    pub async fn echo(&self, peer: String, message: String) -> Result<String, SpikeError> {
        let id = EndpointId::from_str(&peer).map_err(failed)?;
        let conn = self.router.endpoint().connect(id, ECHO_ALPN).await.map_err(failed)?;
        let (mut send, mut recv) = conn.open_bi().await.map_err(failed)?;
        send.write_all(message.as_bytes()).await.map_err(failed)?;
        send.finish().map_err(failed)?;
        let reply = recv.read_to_end(64 * 1024).await.map_err(failed)?;
        conn.close(0u32.into(), b"done");
        String::from_utf8(reply).map_err(failed)
    }

    pub async fn shutdown(&self) -> Result<(), SpikeError> {
        self.router.shutdown().await.map_err(failed)
    }
}

#[derive(Debug, Clone)]
struct Echo;

impl ProtocolHandler for Echo {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let (mut send, mut recv) = connection.accept_bi().await?;
        tokio::io::copy(&mut recv, &mut send).await?;
        send.finish()?;
        connection.closed().await;
        Ok(())
    }
}
