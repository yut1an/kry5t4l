use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use net2::TcpStreamExt;

use crate::modules::crypto::Rc4Cipher;
use crate::modules::protocol::{Protocol, Server, Client};

const TCP_CHUNK_SIZE: usize = 1024;

pub struct TcpServer {
    local_addr: SocketAddr,
    closed: Arc<AtomicBool>,
    connections: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,
}

impl Drop for TcpServer {
    fn drop(&mut self) {
        self.close();
        for i in self.connections.lock().unwrap().values() {
            println!("tcp [{}] dropped", i.peer_addr().unwrap());
        }
    }
}

impl Server for TcpServer {
    fn new<
        CBCB: 'static + Fn(super::Message) + Send + Copy,
        CB: 'static + Fn(super::Protocol, Vec<u8>, SocketAddr, CBCB) + Send,
    >(
        address: &str,
        cb_data: CB,
        cbcb: CBCB,
    ) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut local_addr: SocketAddr = address.parse().unwrap();
        let server = TcpListener::bind(local_addr)?;
        local_addr  = server.local_addr().unwrap();

        // 设置为非阻塞模式
        server.set_nonblocking(true)?;

        let connections = Arc::new(Mutex::new(HashMap::new()));

        let closed = Arc::new(AtomicBool::new(false));

        let closed_1 = closed.clone();
        let connections_1 = connections.clone();

        let cb_data = Arc::new(Mutex::new(cb_data));

        // 启动主线程，负责接受新的客户端连接
        std::thread::Builder::new()
            .name(format!("tcp main worker: {}", server.local_addr().unwrap()))
            .spawn(move || {
                // 循环处理
                for stream in server.incoming() {
                    // 防止 CPU 忙轮询
                    std::thread::sleep(std::time::Duration::from_micros(200));
                    let cb_data = cb_data.clone();

                    match stream {
                        Ok(s) => {
                            // 设置客户端 socket 为阻塞模式
                            s.set_nonblocking(false).unwrap();
                            // 设置 TCP keepalive（200ms 保活）
                            s.set_keepalive_ms(Some(200)).unwrap();

                            let peer_addr = s.peer_addr().unwrap();
                            let mut s_1 = s.try_clone().unwrap();
                            // 保存连接到 HashMap
                            connections_1.lock().unwrap().insert(peer_addr, s);

                            let connections_2 = connections_1.clone();

                            // 启动线程处理收报逻辑
                            std::thread::Builder::new()
                                .name(format!("tcp client worker : {}", s_1.peer_addr().unwrap()))
                                .spawn(move || {
                                    loop {
                                        let mut size_buf = [0u8; 4];

                                        match s_1.read_exact(&mut size_buf) {
                                            Ok(_) => {}
                                            Err(_) => break,
                                        };
                                        
                                        
                                        let total_size = u32::from_be_bytes(size_buf) as usize;

                                        let mut encrypted = Vec::with_capacity(total_size);
                                        let mut reamaining = total_size;

                                        while reamaining > 0 {
                                            let chunk_size = std::cmp::min(TCP_CHUNK_SIZE, reamaining);
                                            let mut chunk = vec![0u8; chunk_size];

                                            if s_1.read_exact(&mut chunk).is_err() {
                                                break;
                                            }

                                            encrypted.extend_from_slice(&chunk);
                                            reamaining -= chunk_size;

                                        }

                                        let decrypted = Rc4Cipher::decrypt(&encrypted);

                                        cb_data.lock().unwrap()(
                                            Protocol::TCP,
                                            decrypted,
                                            peer_addr,
                                            cbcb,
                                        );
                                    }

                                    println!("connection closed or enter tunnel : {}", peer_addr);

                                    // 删除对应关系
                                    connections_2.lock().unwrap().remove(&peer_addr);

                                })
                                .unwrap();
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                // 非阻塞模式下没连接可接收
                                if closed_1.load(Ordering::Relaxed) {
                                    break; // 如果服务器被标记关闭，就退出
                                }
                            } else {
                                continue;
                            }
                        }
                    }
                }

                let mut conns = connections_1.lock().unwrap();
                for i in conns.values_mut() {
                    let _ = i.shutdown(std::net::Shutdown::Both);
                }
                conns.clear();
                println!("server closed");
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
        match self.connections.lock().unwrap().get(peer_addr) {
            Some(mut k) => {

                let raw = Rc4Cipher::encrypt(buf);
                let total_len = raw.len() as u32;

                k.write_all(&total_len.to_be_bytes())?;

                for chunk in raw.chunks(TCP_CHUNK_SIZE) {
                    k.write_all(chunk)?;
                }
                k.flush()?;
                println!("B data sent");
                Ok(())
            }
            None => {
                println!("Client not found: {}", peer_addr);
                Err(std::io::Error::new(
                
                std::io::ErrorKind::NotFound,
                "not found client",
            ))
            }
        }
    }

    fn contains_addr(&mut self, peer_addr: &SocketAddr) -> bool {
        self.connections.lock().unwrap().contains_key(peer_addr)
    }

    fn close(&mut self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}


pub struct TcpConnection {
    s: Option<TcpStream>,
    closed: Arc<AtomicBool>,
}

impl Clone for TcpConnection {
    fn clone(&self) -> Self {
        Self { 
            s: Some(self.s.as_ref().unwrap().try_clone().unwrap()), 
            closed: self.closed.clone(),
        }
    }
}

impl Client for TcpConnection  {
    fn connect(address: &str) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let address: std::net::SocketAddr = match address.parse() {
            Ok(p) => p,
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData, 
                    format!("address format error :{}", e)
                ))
            }
        };

        let s = TcpStream::connect(address)?;
        Ok(Self {
            s: Some(s),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    fn recv(&mut self) -> std::io::Result<Vec<u8>> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "socket closed",
            ));
        }

        let s = match self.s.as_mut() {
            Some(p) => p,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "socket closed",
                ));
            }
        };
        
        let mut size_buf = [0u8; 4];
        s.read_exact(&mut size_buf)?;
        println!("size_buf: {:?}", &size_buf);
        let total_len = u32::from_be_bytes(size_buf) as usize;

        let mut encrypted = vec![0u8; total_len];
        let mut read = 0;
        while read < total_len {
            let to_read = std::cmp::min(TCP_CHUNK_SIZE, total_len - read);
            let n = s.read(&mut encrypted[read..read + to_read])?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof, 
                    "stream closed"
                ));
            }
            read += n;
        }
        println!("encrypted: {:?}", &encrypted);
        Ok(Rc4Cipher::decrypt(&encrypted))
    }

    fn send(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "socket closed",
            ));
        }

        let s = match self.s.as_mut() {
            Some(p) => p,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "socket closed",
                ));
            }
        };
        
        let raw = Rc4Cipher::encrypt(buf);
        let total_len = raw.len() as u32;
        s.write_all(&total_len.to_be_bytes())?;

        for chunk in raw.chunks(TCP_CHUNK_SIZE) {
            s.write(chunk)?;
        }

        Ok(())
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        let s = match self.s.as_ref() {
            Some(p) => p,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "socket closed",
                ));
            }
        };

        s.local_addr()
    }

    fn close(&mut self) {
       self.closed.store(true, Ordering::Relaxed);
       self.s = None;
    }
}

impl Drop for TcpConnection {
    fn drop(&mut self) {
        if let Some(s) = self.s.as_ref() {
            println!("tcp client [{}] dropped", s.peer_addr().unwrap());
            self.s = None;
        } else {
            println!("tcp client dropped");
        }
    }
}