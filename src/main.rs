mod orderbook;

use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer};
use serde_json::json;
use tungstenite::{connect, Message};
use url::Url;

use orderbook::LocalOrderBook;

const WOOX_WS_URL: &str = "wss://wss.woox.io/v3/public";
const REST_URL: &str = "https://api.woox.io/v3/public/orderbook";
const CLIENT_ID: &str = "client_id_x";
const SYMBOL: &str = "PERP_ETH_USDT";
const MAX_LEVEL: usize = 50;

const WOOX_SUBSCRIBE_CMD: &str = "SUBSCRIBE";
const WOOX_PING_CMD: &str = "PING";
const WOOX_PONG_CMD: &str = "PONG";

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


// WsQuote is a struct representation of the quote response apart of the websocket
#[derive(Debug, Deserialize)]
pub struct OrderBookDelta {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "prevTs")]
    pub prev_ts: u64,
    pub bids: Vec<WsQuote>,
    pub asks: Vec<WsQuote>,
}


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
struct WsMessage {
    ts: Option<u64>,
    data: Option<OrderBookDelta>
}

// MarketEvent is a struct representation of 
struct MarketEvent {
    ts: u64,
    prev_ts: u64,
    delta: OrderBookDelta,
}

// connect_stream attempts to connect to the Woo X websocket and returns a receiver
// to consume the stream of market events for the specified symbol. 
fn connect_stream(symbol: &str) -> Receiver<MarketEvent> {
    let (tx, rx) = mpsc::channel();
    let symbol = symbol.to_string();

    thread::spawn(move || {
        let parsed_url = Url::parse(WOOX_WS_URL).unwrap();
        let (mut socket, _) = connect(parsed_url.as_str())
            .expect("Failed to connect to websocker");

        println!("Connected to websocket");

        let topic = format!("orderbookupdate@{}@{}", symbol, MAX_LEVEL);
        let sub_msg = json!({
            "id": CLIENT_ID,
            "cmd": WOOX_SUBSCRIBE_CMD,
            "params": [topic]
        });

        socket.send(Message::Text(sub_msg.to_string())).unwrap();

        loop {
            if let Ok(message) = socket.read() {
                if let Message::Text(text) = message {
                    if text.contains(WOOX_PING_CMD) {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                        let pong = json!(
                            {
                                "cmd": WOOX_PONG_CMD,
                                "ts": now
                            }).to_string();

                        socket.send(Message::Text(pong)).unwrap();
                        continue;
                    }

                    if text.contains("success") { continue; }

                    match serde_json::from_str::<WsMessage>(&text) {
                        Ok(parsed) => {
                            if let (Some(ts), Some(data)) = (parsed.ts, parsed.data) {
                                let event = MarketEvent {
                                    ts,
                                    prev_ts: data.prev_ts,
                                    delta: data,
                                };
                                
                                if tx.send(event).is_err() { break; }
                            }
                        }
                        Err(e) => println!("Parse err: {} , data: {}", e, text),
                    }
                }
            }
        }
    });
    rx
}

// process_orderbook reads events from the receiver and updates the local order book
// with the websocket delta events. It takes a snapshot of the remote order book and 
// repeatedly adds deltas to update the local order book. It prints the order book after every update.
fn process_orderbook(symbol: &str, receiver: Receiver<MarketEvent>) { 
    println!("Buffering for 3 seconds");
    thread::sleep(Duration::from_millis(3000));
    
    println!("Fetching snapshot");
    let url = format!("{}?symbol={}&maxLevel={}", REST_URL, symbol, MAX_LEVEL);
    
    let snapshot: RestSnapshot = reqwest::blocking::get(url)
        .expect("HTTP request failed")
        .json()
        .expect("Failed to parse snapshot json");

    println!("Snapshot received at ts: {}", snapshot.timestamp);

    let mut book = LocalOrderBook::new();
    book.apply_snapshot(snapshot.data);
    
    println!("Attempting to sync book with ws");

    let mut synced = false;

    for event in receiver {
        if !synced {
            if event.prev_ts < snapshot.timestamp {
                let diff = snapshot.timestamp - event.prev_ts;
                println!("Stream is {}ms behind snapshot", diff);
                continue; 
            }

            if event.prev_ts == snapshot.timestamp {
                println!("Local book is now synced");
                synced = true;
                book.apply_delta(event.delta);
                book.print_top_5();
            } 
            else if event.prev_ts > snapshot.timestamp {
                 println!("Local book out of sync, probably rerun with a bigger buffer time");
                 return; 
            }
        } else {
            book.apply_delta(event.delta);
            book.print_top_5();
        }
    }
}

fn main() {
    let data_stream = connect_stream(SYMBOL);
    process_orderbook(SYMBOL, data_stream);
}