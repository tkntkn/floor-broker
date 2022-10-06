use native_tls::{Identity, TlsAcceptor, TlsStream};
// use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use std::fs::File;
use std::io::ErrorKind::{ConnectionAborted, WouldBlock};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use tungstenite::error::Error::Io;
use tungstenite::{accept, Message, WebSocket};

pub trait Broker {
    fn broadcast(&self, message: String);
}

pub trait WebSocketBroker<Stream: Read + Write> {
    fn get_locked_sockets(&self) -> MutexGuard<'_, Vec<WebSocket<Stream>>>;
    fn get_identifier(&self, socket: &Stream) -> String;
}

impl<Stream: Read + Write, T: WebSocketBroker<Stream>> Broker for T {
    fn broadcast(&self, message: String) {
        self.get_locked_sockets().retain_mut(|socket| {
            match socket.read_message() {
                Ok(message) if message.is_close() => {
                    println!("Socket closed: {}.", self.get_identifier(socket.get_ref()));
                    return false;
                }
                Ok(message) => panic!("[003] unknown message: {message}"),
                Err(Io(e)) if e.kind() == WouldBlock => (),
                Err(e) => panic!("[001] encountered unknown error: {e}"),
            }
            match socket.write_message(Message::text(&message)) {
                Ok(()) => true,
                Err(Io(e)) if e.kind() == ConnectionAborted => {
                    println!(
                        "Connection aborted: {}.",
                        self.get_identifier(socket.get_ref())
                    );
                    return false;
                }
                Err(e) => panic!("[002] encountered unknown error: {e}"),
            };
            socket.can_write()
        })
    }
}

pub struct TcpWebSocketBroker {
    sockets: Arc<Mutex<Vec<WebSocket<TcpStream>>>>,
}

impl TcpWebSocketBroker {
    pub fn new(addr: String) -> TcpWebSocketBroker {
        let sockets = Arc::new(Mutex::new(Vec::new()));
        let sockets_ref = sockets.clone();

        let server = TcpListener::bind(addr).unwrap();
        thread::spawn(move || {
            for stream in server.incoming() {
                let socket = accept(stream.unwrap()).unwrap();
                socket.get_ref().set_nonblocking(true).unwrap();
                println!("Connected: {}.", socket.get_ref().peer_addr().unwrap());
                sockets_ref.lock().unwrap().push(socket);
            }
        });

        TcpWebSocketBroker { sockets }
    }
}

impl WebSocketBroker<TcpStream> for TcpWebSocketBroker {
    fn get_locked_sockets(&self) -> MutexGuard<'_, Vec<WebSocket<TcpStream>>> {
        self.sockets.lock().unwrap()
    }

    fn get_identifier(&self, socket: &TcpStream) -> String {
        socket.peer_addr().unwrap().to_string()
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
                socket.get_ref().get_ref().set_nonblocking(true).unwrap();
                sockets_ref.lock().unwrap().push(socket);
            }
        });

        WebSocketSecureBroker { sockets }
    }
}

impl WebSocketBroker<TlsStream<TcpStream>> for WebSocketSecureBroker {
    fn get_locked_sockets(&self) -> MutexGuard<'_, Vec<WebSocket<TlsStream<TcpStream>>>> {
        self.sockets.lock().unwrap()
    }

    fn get_identifier(&self, socket: &TlsStream<TcpStream>) -> String {
        socket.get_ref().peer_addr().unwrap().to_string()
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
