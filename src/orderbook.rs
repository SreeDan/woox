use std::collections::BTreeMap;
use ordered_float::OrderedFloat;

use crate::exchange_api_types::SnapshotData;

// LocalOrderBook contains the current bids and asks for a symbol.
// OrderBookDeltas can be applied to update the order book in real time.
pub struct LocalOrderBook {
    bids: BTreeMap<OrderedFloat<f64>, f64>,
    asks: BTreeMap<OrderedFloat<f64>, f64>,
}

impl LocalOrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new()
        }
    }

    // apply_snapshot clears the orderbook and syncs the state to the given snapshot
    pub fn apply_snapshot(&mut self, data: SnapshotData) {
        self.bids.clear();
        self.asks.clear();

        for quote in data.bids {
            self.bids.insert(OrderedFloat(quote.price), quote.quantity);
        }

        for quote in data.asks {
            self.asks.insert(OrderedFloat(quote.price), quote.quantity);
        }
    }

    // apply_delta applies the order book delta to the local order book.
    // It will remove bids and asks with quantities set to 0.
    pub fn apply_delta(&mut self, delta: crate::OrderBookDelta) {
        for quote in delta.bids {
            if quote.quantity == 0.0 {
                self.bids.remove(&OrderedFloat(quote.price));
            } else {
                self.bids.insert(OrderedFloat(quote.price), quote.quantity);
            }
        }
        
        for quote in delta.asks {
            if quote.quantity == 0.0 {
                self.asks.remove(&OrderedFloat(quote.price));
            } else {
                self.asks.insert(OrderedFloat(quote.price), quote.quantity);
            }
        }
    }

    // print_top_5 will print the top 5 bids and asks in the order book.
    pub fn print_top_5(&self) {
        // Clear console
        print!("{}[2J{}", 27 as char, 27 as char);
        print!("{}[1;1H", 27 as char);
        

        let bids: Vec<_> = self.bids.iter().rev().take(5).collect();
        let asks: Vec<_> = self.asks.iter().take(5).collect();

        for i in 0..5 {
            if i < bids.len() {
                println!("BID Price: {:.2} \t BID Size: {:.4}", bids[i].0, bids[i].1);
            } else {
                println!("BID Price: - \t BID Size: -");
            }
        }
        println!();

        for i in 0..5 {
            if i < asks.len() {
                println!("ASK Price: {:.2} \t ASK Size: {:.4}", asks[i].0, asks[i].1);
            } else {
                println!("ASK Price: - \t ASK Size: -");
            }
        }
    }
}