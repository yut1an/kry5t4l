use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, thread::{self, JoinHandle}, time::{Duration, Instant}};
use xcap::Monitor;
use kry5t4l_share::modules::{protocol::Message, screen::DiffBlock, CommandType};
use lz4_flex::{self, block};

pub struct ScreenCaptureManager {
    pub capture: ScreenCapture,
    pub first_frame_sent: bool,
    pub is_capturing: Arc<AtomicBool>,
    pub capture_thread: Option<JoinHandle<()>>,
}

impl ScreenCaptureManager {
    pub fn new() -> Self {
        let monitors = Monitor::all().unwrap();
        let monitor = monitors.into_iter().next().unwrap();

        let width = monitor.width().unwrap();
        let height = monitor.height().unwrap();

        Self { 
            capture: ScreenCapture::new(width, height), 
            first_frame_sent: false, 
            is_capturing: Arc::new(AtomicBool::new(false)),
            capture_thread: None 
        }
    }

    pub fn start_capture(&mut self, cmd_type: CommandType, clientid: String, sender: std::sync::mpsc::Sender<Vec<u8>>) {
        if self.is_capturing.load(Ordering::Relaxed) {
            println!("已经在捕获中");
            return;
        };

        self.is_capturing.store(true, Ordering::Relaxed);
        self.first_frame_sent = false;

        let is_capturing = Arc::clone(&self.is_capturing);
        let width = self.capture.width;
        let height = self.capture.height;

        println!("开始屏幕捕获线程: {}x{}", width, height);

        let handle = thread::spawn(move || {
            let mut capture = ScreenCapture::new(width, height);
            let mut first_frame_sent = false;

            while is_capturing.load(Ordering::Relaxed) {
                match Self::capture_frame(&mut capture, &mut first_frame_sent) {
                    Ok(data) => {
                        if let Some(packet) = Message::to_bytes(
                            cmd_type.to_u8(), 
                            &clientid, 
                            &data).ok() {
                                if sender.send(packet).is_err() {
                                    eprintln!("channel closed");
                                }
                            }
                    }
                    Err(e) => {
                        if e.to_string() != "没有变化" {
                            eprintln!("捕获帧失败: {}", e);
                        }
                    }
                }
            }

            println!("屏幕捕获线程结束");
        });

        self.capture_thread = Some(handle);
    }

    pub fn stop_capture(&mut self) {
        if !self.is_capturing.load(Ordering::Relaxed) {
            println!("未在捕获中");
            return;
        }

        println!("停止屏幕捕获");
        self.is_capturing.store(false, Ordering::Relaxed);

        if let Some(handle) = self.capture_thread.take() {
            if let Err(e) = handle.join() {
                eprintln!("等待捕获线程结束失败: {:?}", e);
            }
        }

        self.first_frame_sent = false;

    }

    fn capture_frame(capture: &mut ScreenCapture, first_frame_sent: &mut bool) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let monitors = Monitor::all().unwrap();
        let monitor = monitors.into_iter().next().unwrap();

        let image = monitor.capture_image().unwrap();
        let new_screen_data = image.into_raw();

        if !*first_frame_sent {
            let encoded = Self::encode_full_frame(
                capture.width, 
                capture.height, 
                &new_screen_data
            );
            capture.current_screen = new_screen_data;
            *first_frame_sent = true;
            println!("发送完整帧，原始: {} bytes, 压缩后: {} bytes", 
                capture.current_screen.len(), encoded.len());
            Ok(encoded)
        } else {
            let diff_blocks = capture.capture_and_diff(new_screen_data);

            if diff_blocks.is_empty() {
                return Err("没有变化".into());
            }

            let encoded = Self::encode_diff_frame(
                capture.width, 
                capture.height, 
                &diff_blocks
            );
            println!("发送差分帧，{} 个块，压缩后: {} bytes", 
                diff_blocks.len(), encoded.len());
            Ok(encoded)
        }

    }

    // 编码完整帧
    fn encode_full_frame(width: u32, height: u32, screen_data: &[u8]) -> Vec<u8> {
        let mut packet = Vec::new();
        
        // frame_type: 0 = 完整帧
        packet.push(0);
        
        // 宽度和高度
        packet.extend_from_slice(&width.to_le_bytes());
        packet.extend_from_slice(&height.to_le_bytes());
        
        // LZ4 压缩完整屏幕数据
        let compressed = lz4_flex::compress_prepend_size(screen_data);
        packet.extend_from_slice(&compressed);
        
        packet
    }

   fn encode_diff_frame(width: u32, height: u32, diff_blocks: &[DiffBlock]) -> Vec<u8> {
        let mut packet = Vec::new();

        // frame_type: 1 = 差分帧
        packet.push(1);
        
        // 宽度和高度
        packet.extend_from_slice(&width.to_le_bytes());
        packet.extend_from_slice(&height.to_le_bytes());
        
        // 差分块数量
        packet.extend_from_slice(&(diff_blocks.len() as u32).to_le_bytes());
        
        // 每个差分块
        for block in diff_blocks {
            packet.extend_from_slice(&block.x.to_le_bytes());
            packet.extend_from_slice(&block.y.to_le_bytes());
            packet.extend_from_slice(&block.width.to_le_bytes());
            packet.extend_from_slice(&block.height.to_le_bytes());
            
            // LZ4 压缩块数据
            let compressed_block = lz4_flex::compress_prepend_size(&block.data);
            packet.extend_from_slice(&(compressed_block.len() as u32).to_le_bytes());
            packet.extend_from_slice(&compressed_block);
        }
        
        packet
    }
}

#[derive(Debug)]
pub struct ScreenCapture {
    pub current_screen: Vec<u8>,
    pub previous_screen: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub block_size: u32,
}

impl ScreenCapture {
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = (width * height * 4) as usize; // RGBA
        Self { 
            current_screen: vec![0; pixel_count], 
            previous_screen: vec![0; pixel_count], 
            width, 
            height, 
            block_size: 32
        }
    }

    fn is_block_different(&self, x: u32, y: u32, width: u32, height: u32) -> bool {
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;

                if px < self.width && py < self.height {
                    let idx = ((py * self.width + px) * 4) as usize;
                    if idx + 3 < self.current_screen.len() && idx + 3 < self.previous_screen.len() {
                        for i in 0..4 {
                            if self.current_screen[idx + i] != self.previous_screen[idx + i] {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn extract_block_data(&self, x: u32, y: u32, width: u32, height: u32) -> Vec<u8> {
        let mut block_data = Vec::new();

        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;

                if px < self.width && py < self.height {
                    let idx = ((py * self.width + px) * 4) as usize;
                    if idx + 3 < self.current_screen.len() {
                        block_data.extend_from_slice(&self.current_screen[idx..idx + 4]);
                    }
                }
            }
        }

        block_data
    }


    pub fn capture_and_diff(&mut self, new_screen_data: Vec<u8>) -> Vec<DiffBlock> {
        let mut diff_blocks = Vec::new();

        self.previous_screen = self.current_screen.clone();
        self.current_screen = new_screen_data;

        let blocks_x = (self.width + self.block_size - 1) / self.block_size;
        let blocks_y = (self.height + self.block_size - 1) / self.block_size;

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let x = bx * self.block_size;
                let y = by * self.block_size;
                let block_width = (self.block_size).min(self.width - x);
                let block_height = (self.block_size).min(self.height - y);

                if self.is_block_different(x, y, block_width, block_height) {
                    let block_data = self.extract_block_data(x, y, block_width, block_height);
                    diff_blocks.push(DiffBlock { 
                        x, 
                        y, 
                        width: block_width, 
                        height: block_height, 
                        data: block_data 
                    });
                }
            }
        }

        diff_blocks
    }

}
