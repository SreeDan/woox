use serde::{Deserialize, Deserializer};

// WsQuote is a struct representation of the quote response apart of the WsQuote
#[derive(Debug, Clone, Copy)]
pub struct WsQuote {
    pub price: f64,
    pub quantity: f64,
}

impl<'de> Deserialize<'de> for WsQuote {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Vec<String> = Vec::deserialize(deserializer)?;
        if s.len() < 2 {
            return Err(serde::de::Error::custom("WsQuote array too short"));
        }
        let price = s[0].parse::<f64>().map_err(serde::de::Error::custom)?;
        let quantity = s[1].parse::<f64>().map_err(serde::de::Error::custom)?;
        Ok(WsQuote { price, quantity })
    }
}

fn f64_from_string<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
}

// RestQuote is a struct representation of the quore response apart of the REST endpoint
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct RestQuote {
    #[serde(deserialize_with = "f64_from_string")]
    pub price: f64,
    #[serde(deserialize_with = "f64_from_string")]
    pub quantity: f64,
}

// SnapshotData is a struct represntation of a snapshot provided from Woo X
#[derive(Debug, Deserialize)]
pub struct SnapshotData {
    pub bids: Vec<RestQuote>,
    pub asks: Vec<RestQuote>,
}

// RestSnapshot is a struct representation of the snapshot response from Woo X.
#[derive(Debug, Deserialize)]
pub struct RestSnapshot {
    pub timestamp: u64,
    pub data: SnapshotData,
}

// WsMessage is a struct representation of the delta response from the Woo X websocket.
#[derive(Debug, Deserialize)]
pub struct WsMessage {
    pub data: Option<OrderBookDelta>
}


// WsQuote is a struct representation of the quote response apart of the websocket
#[derive(Debug, Deserialize)]
pub struct OrderBookDelta {
    #[serde(rename = "prevTs")]
    pub prev_ts: u64,
    pub bids: Vec<WsQuote>,
    pub asks: Vec<WsQuote>,
}

