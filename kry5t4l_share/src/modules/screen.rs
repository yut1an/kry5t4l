


/// 差分块信息
#[derive(Debug, Clone)]
pub struct DiffBlock {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // 差分块的像素数据 (例如 RGBA)
}

#[derive(Debug, Clone)]
pub struct ScreenFrame {
    pub frame_id: u64,
    pub timestamp: u64,
    pub is_full_frame: bool,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // 压缩后的图像数据或差分数据
    pub diff_blocks: Vec<DiffBlock>, // 差分块信息
}
