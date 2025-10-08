pub mod tcp;
pub mod ws;
//pub mod http;

use std::{fmt::Error, net::SocketAddr, result};

pub type Result<T> = result::Result<T, Error>;

pub trait Serializable {
    fn to_bytes(&self) -> Vec<u8>;
    fn from_bytes(data: &[u8]) -> Option<Self>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    TCP,
    WS,
    Unknow,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TCP => "TCP",
            Self::WS => "WS",
            Self::Unknow => "Unknow",
        })
    }
}

impl Protocol {
    pub fn to_u8(&self) -> u8 {
        match self {
            Protocol::TCP => 0x00,
            Protocol::WS => 0x01,
            Protocol::Unknow => 0xff,
        }
    }

    pub fn from(protocl: u8) -> Self {
        match protocl {
            0x00 => Protocol::TCP,
            0x01 => Protocol::WS,
            _ => Protocol::Unknow,
        }
    }
}

pub fn get_cur_timestamp_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap_or(0)
}

pub fn get_cur_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .try_into()
        .unwrap_or(0)
}

struct BasePacket {
    clientid: String,
    data: Vec<u8>,
}

impl Serializable for BasePacket {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(self.clientid.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&self.clientid.as_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let clientid_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

        let clientid_end = (4 + clientid_len).try_into().unwrap();

        if data.len() < clientid_end {
            return None;
        }

        let clientid = String::from_utf8(data[4..clientid_end].to_vec()).ok()?;
        let data = data[clientid_end..].to_vec();
        
        Some(BasePacket { 
            clientid, 
            data
        })
    }
}


pub struct Message {
    command_type :u8,
    peer_addr: SocketAddr,
    protocol: Protocol,
    client_id: String,
    data: Vec<u8>,
    data_length: usize,
}

impl Message {
    pub fn new(peer_addr: SocketAddr, protocol: Protocol, buf: &[u8]) -> Result<Self> {

        let command_type = buf[0];

        let base = BasePacket::from_bytes(&buf[1..]).unwrap();

        Ok(Self { 
            command_type, 
            peer_addr, 
            protocol, 
            client_id: base.clientid, 
            data: base.data, 
            data_length: buf.len(), 
        })
        
    }

    pub fn to_bytes(command_type: u8, clientid: &String, data: &[u8]) -> Result<Vec<u8>> {
        let mut ret = vec![];
        ret.push(command_type);

        let base = BasePacket {
            clientid: clientid.clone(),
            data: data.to_vec(),
        };

        ret.append(&mut base.to_bytes());

        Ok(ret)
    }

    pub fn command_type(&self) -> u8 {
        self.command_type
    }

    pub fn protocl(&self) -> Protocol {
        self.protocol.clone()
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    pub fn clientid(&self) -> String {
        self.client_id.clone()
    }

    pub fn content(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn length(&self) -> usize {
        self.data_length
    }

}


pub trait Client {
    fn connect(address: &str) -> std::io::Result<Self>
    where
        Self: Sized;
    // fn tunnel(remote_addr: &str, server_local_port: u16) -> std::io::Result<Self>
    // where
    //     Self: Sized;
    fn recv(&mut self) -> std::io::Result<Vec<u8>>;
    fn send(&mut self, buf: &mut [u8]) -> std::io::Result<()>;
    fn local_addr(&self) -> std::io::Result<SocketAddr>;
    fn close(&mut self);
}

pub trait Server {
    fn new<
        CBCB: 'static + Fn(Message) + Send + Copy,
        CB: 'static + Fn(Protocol, Vec<u8>, SocketAddr, CBCB) + Send,
    >(
        address: &str,
        cb_data: CB,
        cbcb: CBCB,
    ) -> std::io::Result<Self>
    where
        Self: Sized;

    fn local_addr(&self) -> std::io::Result<SocketAddr>;
    fn sendto(&mut self, peer_addr: &SocketAddr, buf: &[u8]) -> std::io::Result<()>;
    fn contains_addr(&mut self, peer_addr: &SocketAddr) -> bool;
    fn close(&mut self);
}

#[derive(Debug, Clone)]
pub struct HostOSInfo {
    pub ip: String,
    pub host_name: String,
    pub os_version: String,
    pub user_name: String,
    pub monitor: usize,
}

impl Serializable for HostOSInfo {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // ip: length (u32) + bytes
        bytes.extend_from_slice(&(self.ip.len() as u32).to_be_bytes());
        bytes.extend_from_slice(self.ip.as_bytes());

        // host_name: length (u32) + bytes
        bytes.extend_from_slice(&(self.host_name.len() as u32).to_be_bytes());
        bytes.extend_from_slice(self.host_name.as_bytes());

        // os_version: length (u32) + bytes
        bytes.extend_from_slice(&(self.os_version.len() as u32).to_be_bytes());
        bytes.extend_from_slice(self.os_version.as_bytes());

        // user_name: length (u32) + bytes
        bytes.extend_from_slice(&(self.user_name.len() as u32).to_be_bytes());
        bytes.extend_from_slice(self.user_name.as_bytes());

        // monitor: as u64 (fixed size for usize)
        bytes.extend_from_slice(&(self.monitor as u64).to_be_bytes());

        bytes
    }
    
    fn from_bytes(data: &[u8]) -> Option<Self> {
        let mut offset = 0;

        // Deserialize ip
        if data.len() < offset + 4 {
            return None;
        }
        let ip_len = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;
        if data.len() < offset + ip_len {
            return None;
        }
        let ip = match String::from_utf8(data[offset..offset + ip_len].to_vec()) {
            Ok(s) => s,
            Err(_) => return None,
        };
        offset += ip_len;

        // Deserialize host_name
        if data.len() < offset + 4 {
            return None;
        }
        let host_name_len = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;
        if data.len() < offset + host_name_len {
            return None;
        }
        let host_name = match String::from_utf8(data[offset..offset + host_name_len].to_vec()) {
            Ok(s) => s,
            Err(_) => return None,
        };
        offset += host_name_len;

        // Deserialize os_version
        if data.len() < offset + 4 {
            return None;
        }
        let os_version_len = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;
        if data.len() < offset + os_version_len {
            return None;
        }
        let os_version = match String::from_utf8(data[offset..offset + os_version_len].to_vec()) {
            Ok(s) => s,
            Err(_) => return None,
        };
        offset += os_version_len;

        // Deserialize user_name
        if data.len() < offset + 4 {
            return None;
        }
        let user_name_len = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;
        if data.len() < offset + user_name_len {
            return None;
        }
        let user_name = match String::from_utf8(data[offset..offset + user_name_len].to_vec()) {
            Ok(s) => s,
            Err(_) => return None,
        };
        offset += user_name_len;

        // Deserialize monitor (u64)
        if data.len() < offset + 8 {
            return None;
        }
        let monitor = u64::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
        ]) as usize;
        offset += 8;

        // Ensure we've consumed all data (optional, but good for validation)
        if offset != data.len() {
            return None;
        }

        Some(HostOSInfo {
            ip,
            host_name,
            os_version,
            user_name,
            monitor,
        })
    }
}

pub const HEART_BEAT_TIME: u64 = 5;

#[derive(Debug, Clone)]
pub struct Heartbeat {
    pub time: u64,
    pub in_rate: u64,
    pub out_rate: u64,
}

impl Serializable for Heartbeat {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&self.time.to_be_bytes());
        bytes.extend_from_slice(&self.in_rate.to_be_bytes());
        bytes.extend_from_slice(&self.out_rate.to_be_bytes());

        bytes
    }

    fn from_bytes(data: &[u8]) -> Option<Self>
    where
        Self: Sized {
        let mut offset = 0;

        // Deserialize time
        if data.len() < offset + 8 {
            return None;
        }
        let time = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        // Deserialize in_rate
        if data.len() < offset + 8 {
            return None;
        }
        let in_rate = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        // Deserialize out_rate
        if data.len() < offset + 8 {
            return None;
        }
        let out_rate = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        // Ensure we've consumed all data
        if offset != data.len() {
            return None;
        }

        Some(Heartbeat {
            time,
            in_rate,
            out_rate,
        })
    }
}

pub struct FileTransfer {
    pub src_path: String,
    pub dst_path: String,
    pub file_size: u64,
    pub file_data: Vec<u8>,
    pub status: String,
}

impl FileTransfer {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // src_path
        bytes.extend_from_slice(&(self.src_path.len() as u32).to_be_bytes());
        bytes.extend_from_slice(self.src_path.as_bytes());

        // dst_path
        bytes.extend_from_slice(&(self.dst_path.len() as u32).to_be_bytes());
        bytes.extend_from_slice(self.dst_path.as_bytes());

        // file_size
        bytes.extend_from_slice(&self.file_size.to_be_bytes());

        // file_date (Vec<u8>)
        bytes.extend_from_slice(&(self.file_data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&self.file_data);

        // status
        bytes.extend_from_slice(&(self.status.len() as u32).to_be_bytes());
        bytes.extend_from_slice(self.status.as_bytes());


        bytes
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let mut offset = 0;

        // src_path
        if data.len() < offset + 4 {
            return None;
        }
        let src_len =
            u32::from_be_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
        offset += 4;
        if data.len() < offset + src_len {
            return None;
        }
        let src_path = String::from_utf8(data[offset..offset + src_len].to_vec()).ok()?;
        offset += src_len;

        // dst_path
        if data.len() < offset + 4 {
            return None;
        }
        let dst_len =
            u32::from_be_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
        offset += 4;
        if data.len() < offset + dst_len {
            return None;
        }
        let dst_path = String::from_utf8(data[offset..offset + dst_len].to_vec()).ok()?;
        offset += dst_len;

        // file_size
        if data.len() < offset + 8 {
            return None;
        }
        let file_size =
            u64::from_be_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;

        // file_date
        if data.len() < offset + 4 {
            return None;
        }
        let data_len =
            u32::from_be_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
        offset += 4;
        if data.len() < offset + data_len {
            return None;
        }
        let file_data = data[offset..offset + data_len].to_vec();
        offset += data_len;

        // status
        let status_len = u32::from_be_bytes(data.get(offset..offset+4)?.try_into().ok()?) as usize;
        offset += 4;
        let status = String::from_utf8(data.get(offset..offset+status_len)?.to_vec()).ok()?;
        offset += status_len;

        // 检查是否正好用完数据
        if offset != data.len() {
            return None;
        }

        Some(FileTransfer {
            src_path,
            dst_path,
            file_size,
            file_data,
            status,
        })
    }
}