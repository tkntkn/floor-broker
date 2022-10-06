use native_tls::{Identity, TlsAcceptor, TlsStream};
use std::fs::File;
use std::io::ErrorKind::{ConnectionAborted, WouldBlock};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use tungstenite::error::Error::Io;
use tungstenite::{accept, Message, WebSocket};

pub trait Broker {
    fn broadcast(&self, message: String);
}

enum FlexStream {
    Tcp(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl FlexStream {
    fn get_tcp_ref(&self) -> &TcpStream {
        match self {
            FlexStream::Tcp(s) => s,
            FlexStream::Tls(s) => s.get_ref(),
        }
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.get_tcp_ref().set_nonblocking(nonblocking)
    }

    fn get_identifier(&self) -> String {
        self.get_tcp_ref().peer_addr().unwrap().to_string()
    }
}

impl Read for FlexStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            FlexStream::Tcp(s) => s.read(buf),
            FlexStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for FlexStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            FlexStream::Tcp(s) => s.write(buf),
            FlexStream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            FlexStream::Tcp(s) => s.flush(),
            FlexStream::Tls(s) => s.flush(),
        }
    }
}

pub struct WebSocketBroker {
    sockets: Arc<Mutex<Vec<WebSocket<FlexStream>>>>,
}

impl WebSocketBroker {
    pub fn new(addr: String, secure_addr: String) -> WebSocketBroker {
        let sockets = Arc::new(Mutex::new(Vec::new()));

        let sockets_ref = sockets.clone();
        let server = TcpListener::bind(addr).unwrap();
        thread::spawn(move || {
            for stream in server.incoming() {
                let stream = stream.unwrap();
                let socket = accept(FlexStream::Tcp(stream)).unwrap();
                socket.get_ref().set_nonblocking(true).unwrap();
                println!("Connected: {}.", socket.get_ref().get_identifier());
                sockets_ref.lock().unwrap().push(socket);
            }
        });

        let mut file = File::open("./cert/server.pfx").unwrap();
        let mut identity = vec![];
        file.read_to_end(&mut identity).unwrap();
        let identity = Identity::from_pkcs12(&identity, "").unwrap();
        let acceptor = TlsAcceptor::new(identity).unwrap();

        let sockets_ref = sockets.clone();
        let server = TcpListener::bind(secure_addr).unwrap();
        thread::spawn(move || {
            for stream in server.incoming() {
                let stream = stream.unwrap();
                let socket = accept(FlexStream::Tls(acceptor.accept(stream).unwrap())).unwrap();
                socket.get_ref().set_nonblocking(true).unwrap();
                println!("Connected: {}.", socket.get_ref().get_identifier());
                sockets_ref.lock().unwrap().push(socket);
            }
        });

        WebSocketBroker { sockets }
    }
}

impl Broker for WebSocketBroker {
    fn broadcast(&self, message: String) {
        self.sockets.lock().unwrap().retain_mut(|socket| {
            match socket.read_message() {
                Ok(message) if message.is_close() => {
                    println!("Socket closed: {}.", socket.get_ref().get_identifier());
                    return false;
                }
                Ok(message) => panic!("[003] unknown message: {message}"),
                Err(Io(e)) if e.kind() == WouldBlock => (),
                Err(e) => panic!("[001] encountered unknown error: {e}"),
            }
            match socket.write_message(Message::text(&message)) {
                Ok(()) => true,
                Err(Io(e)) if e.kind() == ConnectionAborted => {
                    println!("Connection aborted: {}.", socket.get_ref().get_identifier());
                    return false;
                }
                Err(e) => panic!("[002] encountered unknown error: {e}"),
            };
            socket.can_write()
        })
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
