use std::fs::File;
use std::io::{self, BufRead};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, thread};
use tungstenite::{accept, Message, WebSocket};

trait Broker {
    fn broadcast(&self, message: String);
}

struct WebSocketBorker {
    sockets: Arc<Mutex<Vec<WebSocket<TcpStream>>>>,
}

impl WebSocketBorker {
    fn new(addr: String) -> WebSocketBorker {
        let sockets = Arc::new(Mutex::new(Vec::new()));

        let server = TcpListener::bind(addr).unwrap();
        let sockets_ref = sockets.clone();
        thread::spawn(move || {
            for stream in server.incoming() {
                let socket = accept(stream.unwrap()).unwrap();
                sockets_ref.lock().unwrap().push(socket);
            }
        });

        WebSocketBorker { sockets }
    }
}

impl Broker for WebSocketBorker {
    fn broadcast(&self, message: String) {
        let sockets_ref = self.sockets.clone();
        thread::spawn(move || {
            let mut sockets = sockets_ref.lock().unwrap();
            for socket in sockets.iter_mut() {
                socket.write_message(Message::text(&message)).unwrap();
            }
        });
    }
}

struct UdpBroker {
    target_addr: String,
    socket: UdpSocket,
}

impl UdpBroker {
    fn new(addr: String) -> UdpBroker {
        UdpBroker {
            target_addr: addr,
            socket: UdpSocket::bind("localhost:0").expect("Could not bind socket"),
        }
    }
}

impl Broker for UdpBroker {
    fn broadcast(&self, message: String) {
        self.socket
            .send_to(message.as_bytes(), &self.target_addr)
            .expect("failed to send message");
    }
}

trait FloorMessanger {
    fn create_receiver(&self) -> Receiver<String>;
}

struct FileFloorMessanger {
    filename: String,
}

impl FileFloorMessanger {
    fn new(filename: String) -> FileFloorMessanger {
        FileFloorMessanger { filename }
    }
}

impl FloorMessanger for FileFloorMessanger {
    fn create_receiver(&self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        let lines = read_lines(&self.filename);
        thread::spawn(move || {
            let start_epoch = current_epoch();
            if let Ok(mut lines) = lines {
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
        rx
    }
}

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
