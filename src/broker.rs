use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use tungstenite::{accept, Message, WebSocket};

pub trait Broker {
    fn broadcast(&self, message: String);
}

pub struct WebSocketBorker {
    sockets: Arc<Mutex<Vec<WebSocket<TcpStream>>>>,
}

impl WebSocketBorker {
    pub fn new(addr: String) -> WebSocketBorker {
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

pub struct UdpBroker {
    target_addrs: Vec<String>,
    socket: UdpSocket,
}

impl UdpBroker {
    pub fn new(target_addrs: Vec<String>) -> UdpBroker {
        let socket = UdpSocket::bind("localhost:0").expect("Could not bind socket");
        UdpBroker {
            target_addrs,
            socket,
        }
    }
}

impl Broker for UdpBroker {
    fn broadcast(&self, message: String) {
        for addr in &self.target_addrs {
            self.socket.send_to(message.as_bytes(), addr).unwrap();
        }
    }
}
