use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use std::fs::File;
use std::io::{self, BufRead};
use std::net::UdpSocket;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, thread};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: command <dat file> <udp addr>");
        return;
    }

    let filename = String::from(&args[1]);
    let target_addr = String::from(&args[2]);

    let ws_streams: Arc<Mutex<Vec<SplitSink<WebSocketStream<TcpStream>, Message>>>> =
        Arc::new(Mutex::new(Vec::new()));
    let thread_ws_streams = ws_streams.clone();
    tokio::spawn(async move {
        let listener = TcpListener::bind("localhost:8080")
            .await
            .expect("Can't listen");
        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.expect("Failed to accept");
            let (ws_sender, _) = ws_stream.split();
            thread_ws_streams.lock().unwrap().push(ws_sender);
        }
    });

    let socket = UdpSocket::bind("localhost:0").expect("Could not bind socket");

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let start_epoch = current_epoch();
        if let Ok(mut lines) = read_lines(filename) {
            let first_line = lines.next().unwrap().expect("File is not empty");
            let ShallowParsedData(data_start_epoch, _data) = shallow_parse_data(first_line);

            for line in lines {
                if let Ok(line) = line {
                    let ShallowParsedData(data_epoch, data) = shallow_parse_data(line);
                    let data_time = data_epoch - data_start_epoch;
                    loop {
                        let past_time = current_epoch() - start_epoch;
                        if past_time >= data_time {
                            break;
                        }
                    }
                    println!("{}", data_time);
                    tx.send(data).unwrap();
                }
            }
        }
    });

    for data in rx {
        socket
            .send_to(data.as_bytes(), &target_addr)
            .expect("failed to send message");
        for ws_stream in ws_streams.lock().unwrap().iter_mut() {
            let _ = ws_stream.send(Message::text(data.as_str())).await;
        }
    }
}

struct ShallowParsedData(u128, String);

fn shallow_parse_data(line: String) -> ShallowParsedData {
    let data: Vec<&str> = line.split(":").collect();
    let epoch = data[0].parse::<u128>().unwrap();
    ShallowParsedData(epoch, line)
}

fn current_epoch() -> u128 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    duration.as_millis()
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
