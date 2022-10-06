mod broker;
mod floor_messanger;

use crate::broker::*;
use crate::floor_messanger::*;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 {
        println!("Usage: command (replay <dat file> | serial <config file>) <websocket addr> [...<udp addrs>]");
        return;
    }

    let mode = &args[1];
    let filename = &args[2];
    let webscoket_addr = &args[3];
    let upd_addrs = args[4..]
        .into_iter()
        .map(|e| e.to_string())
        .collect::<Vec<String>>();

    let brokers: Vec<Box<dyn Broker>> = vec![
        Box::new(TcpWebSocketBroker::new(webscoket_addr.to_string())),
        // Box::new(WebSocketSecureBroker::new(webscoket_addr.to_string())),
        Box::new(UdpBroker::new(upd_addrs)),
    ];

    let messanger: Box<dyn FloorMessanger> = if mode.as_str() == "replay" {
        Box::new(ReplayFloorMessanger::new(filename.to_string()))
    } else {
        Box::new(SerialFloorMessanger::new(filename.to_string()))
    };

    for data in messanger.create_receiver() {
        for broker in &brokers {
            broker.broadcast(String::from(&data));
        }
    }
}
