mod broker;
mod floor_messanger;

use crate::broker::*;
use crate::floor_messanger::*;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: command <dat file> <udp addr>");
        return;
    }

    let filename = String::from(&args[1]);
    let target_addr = String::from(&args[2]);

    let brokers: Vec<Box<dyn Broker>> = vec![
        Box::new(WebSocketBorker::new("localhost:8080".to_string())),
        Box::new(UdpBroker::new(target_addr)),
    ];

    let messanger = FileFloorMessanger::new(filename);
    for data in messanger.create_receiver() {
        for broker in &brokers {
            broker.broadcast(String::from(&data));
        }
    }
}
