mod orderbook;
mod exchange_api_types;

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::net::TcpStream;

use serde_json::json;
use tungstenite::{connect, Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use url::Url;

use orderbook::LocalOrderBook;
use exchange_api_types::{OrderBookDelta, WsMessage, RestSnapshot};

const WOOX_WS_URL: &str = "wss://wss.woox.io/v3/public";
const REST_URL: &str = "https://api.woox.io/v3/public/orderbook";
const CLIENT_ID: &str = "client_id_x";
const SYMBOL: &str = "PERP_ETH_USDT";
const MAX_LEVEL: usize = 50;

const WOOX_SUBSCRIBE_CMD: &str = "SUBSCRIBE";
const WOOX_PING_CMD: &str = "PING";
const WOOX_PONG_CMD: &str = "PONG";


// MarketEvent represents an order book delta provided by the Woo X exchange.
struct MarketEvent {
    prev_ts: u64,
    delta: OrderBookDelta,
}

// read_exchange_events reads delta updates from the WebSocket and sends evnts over the Sender
fn read_exchange_events(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, tx: Sender<MarketEvent>) {
    loop {
        if let Ok(Message::Text(text)) = socket.read() {
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
                    if let Some(data) = parsed.data {
                        let event = MarketEvent {
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
        read_exchange_events(&mut socket, tx);
    });

    rx
}

// process_orderbook reads events from the receiver and updates the local order book
// with the websocket delta events. It takes a snapshot of the remote order book and 
// repeatedly adds deltas to update the local order book. It prints the order book after every update.
fn process_orderbook(symbol: &str, receiver: Receiver<MarketEvent>) { 
    println!("Buffering for 4 seconds");
    thread::sleep(Duration::from_millis(4000));
    
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