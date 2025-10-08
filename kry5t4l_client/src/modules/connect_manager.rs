use std::{net::Ipv4Addr, sync::{atomic::AtomicU64, Arc}, time::Duration};
use std::sync::atomic::Ordering::Relaxed;
use sysinfo;
use os_info;
use xcap::Monitor;
use whoami;
use lazy_static::*;
use kry5t4l_share::{
    self, 
    modules::{
        connection_manager::ClientConnector,
        protocol::{get_cur_timestamp_secs, Heartbeat, HostOSInfo, Message, Serializable, HEART_BEAT_TIME}, 
        CommandType
    }
};



lazy_static! {
    static ref G_OUT_BYTES : Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    static ref G_IN_BYTES : Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

}


pub fn get_host_info() -> HostOSInfo {
    let host_name = sysinfo::System::host_name().unwrap();

    let networks = sysinfo::Networks::new_with_refreshed_list();

    let mut ips = String::new();
    for (_name, network) in &networks {
        for ip in network.ip_networks() {
            if let std::net::IpAddr::V4(v4) = ip.addr {
                if v4 != Ipv4Addr::LOCALHOST {
                    ips.push_str(&format!("{}", v4));
                }
            }
        }
    }

    let info = os_info::get();
    let os_version = format!("{} {} ({})", info.edition().unwrap(), info.bitness(), info.architecture().unwrap());

    HostOSInfo {
        ip: ips,
        host_name,
        os_version,
        user_name: whoami::username(),
        monitor: Monitor::all().unwrap().len(),
    }
}

pub fn start_sender_thread(client: ClientConnector, receiver: std::sync::mpsc::Receiver<Vec<u8>>) {
    let mut client_1 = client.clone();
    std::thread::spawn(move || {
        for mut buf in receiver {
            G_OUT_BYTES.fetch_add(buf.len() as u64, Relaxed);
            //println!("buf: [{:?}]", buf);
            if let Err(e) = client_1.send(&mut buf) {
                println!("sender failed: {}", e);
                client_1.close();
                break;
            }
            //println!("send [{}] bytes", buf.len());
        }
    });
}

pub fn start_heartbeat_thread(clientid: String, sender: std::sync::mpsc::Sender<Vec<u8>>) {
    std::thread::spawn(move || {
        loop {
            let in_rate = G_IN_BYTES.load(Relaxed);
            let out_rate = G_OUT_BYTES.load(Relaxed);

            G_IN_BYTES.store(0, Relaxed);
            G_OUT_BYTES.store(0, Relaxed);

            let heartbeat = Heartbeat {
                time: get_cur_timestamp_secs(),
                in_rate,
                out_rate,
            };

            //println!("inrate : {} , outrate : {}", in_rate, out_rate);

            let buf = match Message::to_bytes(
                CommandType::Heartbeat.to_u8(),
                &clientid,
                &heartbeat.to_bytes()
            ) {
                Ok(p) => p,
                Err(e) => {
                    println!("make Heartbeat faild : {}", e);
                    break;
                }
            };

            if sender.send(buf).is_err() {
                println!("heartbeat channel closed");
                break;
            }

            std::thread::sleep(Duration::from_secs(HEART_BEAT_TIME));
        }
    });
}