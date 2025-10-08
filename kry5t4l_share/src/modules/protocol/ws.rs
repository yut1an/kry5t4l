use std::{
    collections::HashMap, net::{SocketAddr, TcpStream}, sync::{atomic::AtomicBool, Arc, Mutex}
};

use websocket::{
    sync::{Reader, Writer}, OwnedMessage
};

use crate::modules::{crypto::Rc4Cipher, protocol::{Client, Protocol, Server}};

pub struct WSServer {
    local_addr: SocketAddr,
    closed: Arc<AtomicBool>,
    connections: Arc<Mutex<HashMap<SocketAddr, Writer<TcpStream>>>>,
}

impl Drop for WSServer {
    fn drop(&mut self) {
        self.close();
    }
}

impl Server for WSServer {
    fn new<
        CBCB: 'static + Fn(super::Message) + Send + Copy,
        CB: 'static + Fn(super::Protocol, Vec<u8>, SocketAddr, CBCB) + Send,
    >(
        address: &str,
        cb_data: CB,
        cbcb: CBCB,
    ) -> std::io::Result<Self>
    where
        Self: Sized 
    {
        let mut server = websocket::sync::Server::bind(address)?;
        
        // 设置非阻塞模式
        server.set_nonblocking(true).unwrap();

        let connections: Arc<Mutex<HashMap<SocketAddr, Writer<TcpStream>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let closed = Arc::new(AtomicBool::new(false));

        let local_addr = server.local_addr().unwrap();

        let connections_1 = connections.clone();
        let closed_1 = closed.clone();
        
        let cb_data = Arc::new(Mutex::new(cb_data));

        std::thread::Builder::new()
            .name(format!("ws main worker : {}", local_addr.clone()))
            .spawn(move || {
                loop {
                    let clinet = match server.accept() {
                        Ok(p) => p.accept().unwrap(),
                        Err(_) => {
                            if closed_1.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }

                            std::thread::sleep(std::time::Duration::from_millis(200));
                            continue;

                        }
                    };

                    let connections_2 = connections_1.clone();
                    let cb_data = cb_data.clone();
                    std::thread::Builder::new()
                        .name(format!("ws client worker : {}", local_addr.clone()))
                        .spawn(move || {
                            // 设置阻塞模式
                            clinet.set_nonblocking(false).unwrap();
                            let remote_addr = clinet.peer_addr().unwrap();

                            println!("ws accept from : {}", remote_addr);

                            let (mut receiver, sender) = clinet.split().unwrap();

                            {
                                let mut conns = connections_2.lock().unwrap();
                                conns.insert(remote_addr, sender);
                            }

                            for message in receiver.incoming_messages() {
                                let message = match message {
                                    Ok(p) => p,
                                    Err(e) => {
                                        println!("ws connection incomming msg error : {}", e);
                                        break;
                                    }
                                };

                                match message {
                                    OwnedMessage::Close(_) => {
                                        println!("ws connection closed : {}", remote_addr);
                                        break;
                                    }
                                    OwnedMessage::Binary(buf) => {
                                        let decrypted = Rc4Cipher::decrypt(&buf);

                                        cb_data.lock().unwrap()(
                                            Protocol::WS,
                                            decrypted,
                                            remote_addr,
                                            cbcb,
                                        )
                                    }
                                    _ => {}
                                }
                            }
                            connections_2.lock().unwrap().remove(&remote_addr);
                            println!("ws client worker finished: {}", remote_addr);
                        })
                        .unwrap();
                }

            })
            .unwrap();

        Ok(Self { 
            local_addr, 
            closed, 
            connections,
        })
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(self.local_addr)
    }

    fn sendto(&mut self, peer_addr: &SocketAddr, buf: &[u8]) -> std::io::Result<()> {
        match self.connections.lock().unwrap().get_mut(peer_addr) {
            Some(k) => {
                let raw = Rc4Cipher::encrypt(buf);
                let msg = OwnedMessage::Binary(raw);
                match k.send_message(&msg) {
                    Ok(_) => {}
                    Err(e) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Interrupted, 
                            format!("ws send msg error : {}", e)
                        ));
                    }
                }
                Ok(())
            }
            None => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound, 
                    "not found client"
                ))
            }
        }
    }

    fn contains_addr(&mut self, peer_addr: &SocketAddr) -> bool {
        self.connections.lock().unwrap().contains_key(peer_addr)
    }

    fn close(&mut self) {
        self.closed.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}


pub struct WSConnection {
    reader: Option<Arc<Mutex<Reader<TcpStream>>>>,
    writer: Option<Arc<Mutex<Writer<TcpStream>>>>,
    local_addr: SocketAddr,
    closed: Arc<AtomicBool>,
}

impl Client for WSConnection  {
    fn connect(address: &str) -> std::io::Result<Self>
    where
        Self: Sized 
    {
        let s = match websocket::ClientBuilder::new(&format!("ws://{}", address))
            .unwrap()
            .connect_insecure() 
        {
            Ok(p) => p,
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted, 
                    format!("ws connect error : {}", e)
                ));
            }
        };

        let local_addr = s.local_addr().unwrap();

        let (reader, writer) = s.split().unwrap();

        Ok(Self { 
            reader: Some(Arc::new(Mutex::new(reader))), 
            writer: Some(Arc::new(Mutex::new(writer))), 
            local_addr, 
            closed:  Arc::new(AtomicBool::new(false)),
        })

    }

    fn recv(&mut self) -> std::io::Result<Vec<u8>> {
        if self.closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData, 
                "socket closed",
            ));
        }

        let s = match self.reader.as_mut() {
            Some(p) => p,
            None => {
               return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "socket closed",
                ));
            }
        };

        let mut s_lock = s.lock().unwrap();

        match s_lock.recv_message() {
            Ok(msg) => match msg {
                OwnedMessage::Binary(buf) => {
                    let decrypted = Rc4Cipher::decrypt(&buf);
                    Ok(decrypted)
                }
                OwnedMessage::Close(_) => {
                    drop(s_lock);
                    self.close();
                   Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "ws closed".to_string(),
                    ))
                }
                _ => Ok(vec![]),
            }
            Err(e) => {
               Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    format!("ws receive error : {}", e),
                ))
            }
        }

    }

    fn send(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        if self.closed.load(std::sync::atomic::Ordering::Relaxed) {
           return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "socket closed",
            ));
        }

        let s = match self.writer.as_mut() {
            Some(p) => p,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "socket closed",
                ));
            }
        };

        let buf = Rc4Cipher::encrypt(buf);
        let msg = OwnedMessage::Binary(buf.to_vec());
        if let Err(e) = s.lock().unwrap().send_message(&msg) {
           return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                format!("ws send msg error : {}", e),
            ));
        }

        Ok(())

    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(self.local_addr)
    }

    fn close(&mut self) {
        self.closed.store(true, std::sync::atomic::Ordering::Relaxed);
        self.reader = None;
        self.writer = None;
    }
}

impl Clone for WSConnection {
    fn clone(&self) -> Self {
        Self { 
            reader: self.reader.clone(), 
            writer: self.writer.clone(), 
            local_addr: self.local_addr, 
            closed: self.closed.clone(), 
        }
    }
}

impl Drop for WSConnection  {
    fn drop(&mut self) {
        self.reader = None;
        self.writer = None;
    }
}