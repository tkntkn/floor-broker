use native_tls::{Identity, TlsAcceptor, TlsStream};
// use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use std::fs::File;
use std::io::Read;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use tungstenite::{accept, Message, WebSocket};

pub trait Broker {
    fn broadcast(&self, message: String);
}

pub struct WebSocketBroker {
    sockets: Arc<Mutex<Vec<WebSocket<TcpStream>>>>,
}

impl WebSocketBroker {
    pub fn new(addr: String) -> WebSocketBroker {
        let sockets = Arc::new(Mutex::new(Vec::new()));
        let sockets_ref = sockets.clone();

        let server = TcpListener::bind(addr).unwrap();
        thread::spawn(move || {
            for stream in server.incoming() {
                let socket = accept(stream.unwrap()).unwrap();
                sockets_ref.lock().unwrap().push(socket);

                // let sockets_ref = sockets_ref.clone();
                // thread::spawn(move || loop {
                //     thread::sleep(Duration::from_millis(1));
                //     let msg = socket_ref.lock().unwrap().read_message().unwrap();
                //     if msg.is_close() {
                //         dbg!("connection closed");
                //         let mut sockets = sockets_ref.lock().unwrap();
                //         sockets.retain(|s| {
                //             s.lock().unwrap().get_ref().peer_addr().unwrap()
                //                 == socket_ref.lock().unwrap().get_ref().peer_addr().unwrap()
                //         });
                //         break;
                //     }
                // });
            }
        });

        WebSocketBroker { sockets }
    }
}

impl Broker for WebSocketBroker {
    fn broadcast(&self, message: String) {
        let sockets_ref = self.sockets.clone();
        thread::spawn(move || {
            let mut sockets = sockets_ref.lock().unwrap();
            for socket in sockets.iter_mut() {
                socket
                    // .lock()
                    // .unwrap()
                    .write_message(Message::text(&message))
                    .unwrap();
            }
        });
    }
}

pub struct WebSocketSecureBroker {
    sockets: Arc<Mutex<Vec<WebSocket<TlsStream<TcpStream>>>>>,
}

impl WebSocketSecureBroker {
    pub fn new(addr: String) -> WebSocketSecureBroker {
        let mut file = File::open("./cert/server.pfx").unwrap();
        let mut identity = vec![];
        file.read_to_end(&mut identity).unwrap();
        let identity = Identity::from_pkcs12(&identity, "").unwrap();

        let acceptor = TlsAcceptor::new(identity).unwrap();
        let acceptor = Arc::new(acceptor);
        let acceptor_ref = acceptor.clone();

        let sockets = Arc::new(Mutex::new(Vec::new()));
        let sockets_ref = sockets.clone();

        let server = TcpListener::bind(addr).unwrap();
        thread::spawn(move || {
            for stream in server.incoming() {
                let socket = accept(acceptor_ref.accept(stream.unwrap()).unwrap()).unwrap();
                sockets_ref.lock().unwrap().push(socket);

                // let sockets_ref = sockets_ref.clone();
                // thread::spawn(move || loop {
                //     thread::sleep(Duration::from_millis(1));
                //     let msg = socket_ref.lock().unwrap().read_message().unwrap();
                //     if msg.is_close() {
                //         dbg!("connection closed");
                //         let mut sockets = sockets_ref.lock().unwrap();
                //         sockets.retain(|s| {
                //             s.lock().unwrap().get_ref().peer_addr().unwrap()
                //                 == socket_ref.lock().unwrap().get_ref().peer_addr().unwrap()
                //         });
                //         break;
                //     }
                // });
            }
        });

        WebSocketSecureBroker { sockets }
    }
}

impl Broker for WebSocketSecureBroker {
    fn broadcast(&self, message: String) {
        let sockets_ref = self.sockets.clone();
        thread::spawn(move || {
            let mut sockets = sockets_ref.lock().unwrap();
            for socket in sockets.iter_mut() {
                socket
                    // .lock()
                    // .unwrap()
                    .write_message(Message::text(&message))
                    .unwrap();
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
