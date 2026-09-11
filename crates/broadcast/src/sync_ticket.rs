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
