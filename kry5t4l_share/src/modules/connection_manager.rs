use std::{net::SocketAddr, ops::{Deref, DerefMut}};

use crate::modules::protocol::{
    ws::{WSConnection, WSServer}, tcp::{TcpConnection, TcpServer}, Client, Message, Protocol, Server
};


pub struct ClientConnector {
    protocol_type: Protocol,
    tcp_client: Option<TcpConnection>,
    ws_client: Option<WSConnection>,
}

impl Deref for ClientConnector {
    type Target = dyn Client;

    fn deref(&self) -> &Self::Target {
        match self.protocol_type {
            Protocol::TCP => self.tcp_client.as_ref().unwrap(),
            Protocol::WS => self.ws_client.as_ref().unwrap(),
            Protocol::Unknow => panic!("unknow protocol"),
        }
    }
}

impl DerefMut for ClientConnector {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.protocol_type {
            Protocol::TCP => self.tcp_client.as_mut().unwrap(),
            Protocol::WS => self.ws_client.as_mut().unwrap(),
            Protocol::Unknow => panic!("unknow protocol"),
        }
    }
}

impl Clone for ClientConnector {
    fn clone(&self) -> Self {
        Self { 
            protocol_type: self.protocol_type.clone(), 
            tcp_client: self.tcp_client.clone(),
            ws_client: self.ws_client.clone(),
        }
    }
}

impl ClientConnector {
    pub fn connect(protocol_type: &Protocol, address: &str) -> std::io::Result<Self> {
        match protocol_type {
            Protocol::TCP => {
                let client = TcpConnection::connect(address)?;
                Ok(Self { 
                    protocol_type: protocol_type.clone(), 
                    tcp_client: Some(client),
                    ws_client: None,
                })
            }
            Protocol::WS => {
                let client = WSConnection::connect(address)?;
                Ok(Self { 
                    protocol_type: protocol_type.clone(), 
                    tcp_client: None, 
                    ws_client: Some(client),
                })
            }
            Protocol::Unknow => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invaild protocol type",
                ))
            }
        }
    }
}


pub struct ServerConnector {
    tcp_server: Option<TcpServer>,
    ws_server: Option<WSServer>,
    protocol: Protocol,
}

impl ServerConnector {
    fn cb_connection<CB: 'static + Fn(Message) + Send + Copy>(
        protocol: Protocol,
        data: Vec<u8>,
        peer_addr: SocketAddr,
        cb: CB,
    ) {
        let msg = Message::new(peer_addr, protocol, &data).unwrap();
        cb(msg);
    }

    pub fn new<CB: 'static + Fn(Message) + Send + Copy>(
        protocol: Protocol,
        port: u16,
        cb_msg: CB,
    ) -> std::io::Result<Self> {
        match protocol {
            Protocol::TCP => {
                match TcpServer::new(
                    format!("0.0.0.0:{}", port).as_str(), 
                    ServerConnector::cb_connection, 
                    cb_msg
                ) {
                    Ok(tcp_server) => Ok(Self { 
                        tcp_server: Some(tcp_server),
                        ws_server: None,
                        protocol
                    }),
                    Err(e) => Err(e),
                }
            }
            Protocol::WS => {
                match WSServer::new(
                    format!("0.0.0.0:{}", port).as_str(), 
                    ServerConnector::cb_connection, 
                    cb_msg
                ) {
                        Ok(ws_server) => Ok(Self { 
                            tcp_server: None, 
                            ws_server: Some(ws_server), 
                            protocol
                        }),
                        Err(e) => Err(e),
                    }
            }
            Protocol::Unknow => panic!("unknow protocol"),
        }
    }

    pub fn sendto(&mut self, peer_addr: &SocketAddr, buf: &[u8]) -> std::io::Result<()> {
        match self.protocol {
            Protocol::TCP => self.tcp_server.as_mut().unwrap().sendto(peer_addr, buf),
            Protocol::WS => self.ws_server.as_mut().unwrap().sendto(peer_addr, buf),
            Protocol::Unknow => panic!("unknow protocol"),
        }
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        match self.protocol {
            Protocol::TCP => self.tcp_server.as_ref().unwrap().local_addr(),
            Protocol::WS => self.ws_server.as_ref().unwrap().local_addr(),
            Protocol::Unknow => panic!("unknow protocol"),
        }
    }

    pub fn protocl(&self) -> Protocol {
        self.protocol.clone()
    }

    pub fn contains_addr(&mut self, peer_addr: &SocketAddr) -> bool {
        match self.protocol {
            Protocol::TCP => self.tcp_server.as_mut().unwrap().contains_addr(peer_addr),
            Protocol::WS => self.ws_server.as_mut().unwrap().contains_addr(peer_addr),
            Protocol::Unknow => panic!("unknow protocol"),
        }
    }

    pub fn close(&mut self) {
        match self.protocol {
            Protocol::TCP => self.tcp_server.as_mut().unwrap().close(),
            Protocol::WS => self.ws_server.as_mut().unwrap().close(),
            Protocol::Unknow => panic!("unknow protocol"),
        }
    }

}