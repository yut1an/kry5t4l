use iced::{
    widget::{button, column, container, image, row, text}, 
    Alignment, Background, Border, Color, Element, Length, Task, Theme, Size, Point
};
use kry5t4l_share::modules::{screen::{DiffBlock, ScreenFrame}, CommandType};
use std::{collections::HashMap, mem, net::SocketAddr, sync::{Arc, Mutex}, time::{Duration, Instant}};
use once_cell::sync::Lazy;
use lz4_flex;
use std::collections::VecDeque;

use crate::modules::network::send_command_to;

// 全局Monitor消息发送器
static G_MONITOR_MESSAGE_SENDER: Lazy<Arc<Mutex<Option<crossbeam_channel::Sender<MonitorUpdate>>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone)]
pub enum MonitorUpdate {
    ScreenData {
        client_id: String,
        screen_data: ScreenFrame,
    },
    ScreenInfo {
        client_id: String,
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    StartCapture,
    StopCapture,
    ProcessFrame,
}

#[derive(Debug, Clone)]
pub struct MonitorWindow {
    pub client_id: String,
    pub peer_addr: SocketAddr,
    pub title: String,
    pub is_capturing: bool,
    pub current_frame: Option<ScreenFrame>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub window_scale: f32,
    pub frame_rate: u8,
    pub last_frame_time: Option<Instant>,
    pub frame_buffer: Vec<u8>, // 当前完整帧缓冲 (RGBA)
    //pub frame_counter: u64,
    pub fps_counter: u64,
    pub fps_start_time: Option<Instant>,
    pub current_fps: u32,
    pub image_handle: Option<image::Handle>, // iced图像句柄
    pub received_first_frame: bool, // 是否已收到第一帧
    // 新增：帧队列
    frame_queue: VecDeque<ScreenFrame>,
    max_queue_size: usize,  // 最大队列长度，防止内存溢出
    
    // 新增：处理控制
    target_fps: u8,  // 服务端目标帧率
    last_process_time: Option<Instant>,
    queue_size: usize,  // 用于显示当前队列大小
}

impl MonitorWindow {
    pub fn new(client_id: String, peer_addr: SocketAddr) -> Self {
        
        Self {
            client_id,
            peer_addr,
            title: format!("Monitor - {}", peer_addr),
            is_capturing: false,
            current_frame: None,
            screen_width: 1920,
            screen_height: 1080,
            window_scale: 0.4, // 默认缩放比例
            frame_rate: 30,
            last_frame_time: None,
            frame_buffer: Vec::new(),
            //frame_counter: 0,
            fps_counter: 0,
            fps_start_time: None,
            current_fps: 0,
            image_handle: None,
            received_first_frame: false,
            frame_queue: VecDeque::new(),
            max_queue_size: 90,  // 保留最多90帧（3秒缓冲 @ 30fps）
            target_fps: 30,
            last_process_time: None,
             queue_size: 0,
        }
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn update(&mut self, message: MonitorMessage) {
        match message {
            MonitorMessage::StartCapture => {
                self.is_capturing = true;
                self.frame_queue.clear();
                self.queue_size = 0;

                println!("开始屏幕捕获: {}", self.client_id);

                self.send_capture_command(true);
            }
            MonitorMessage::StopCapture => {
                self.is_capturing = false;
                self.received_first_frame = false;
                self.image_handle = None;
                self.frame_buffer.clear();
                self.frame_queue.clear();
                self.fps_counter = 0;
                self.fps_start_time = None;
                self.current_fps = 0;
                self.queue_size = 0;
                println!("停止屏幕捕获: {}", self.client_id);

                self.send_capture_command(false);
            }
            MonitorMessage::ProcessFrame => {
                self.process_next_frame();
            }
        }
    }
            
    fn send_capture_command(&self, start: bool) {

        let mut buf = vec![];
        buf.push(CommandType::Screenshot.to_u8());

        if start {
            buf.push(1);
        } else {
            buf.push(0);
        }

        let _ = send_command_to(&self.peer_addr, &buf);
    }

    pub fn update_screen_info(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        
        // 计算合适的缩放比例以适应窗口
        let max_width = 1200.0;
        let max_height = 800.0;
        let scale_x = max_width / width as f32;
        let scale_y = max_height / height as f32;
        self.window_scale = scale_x.min(scale_y).min(1.0);
        
        println!("屏幕尺寸更新: {}x{}, 缩放比例: {:.2}", width, height, self.window_scale);
        
    }

    fn apply_diff_blocks(&mut self, diff_blocks: &[DiffBlock]) {
        for block in diff_blocks {
            // LZ4 解压，获取原始 RGBA 块数据
            let raw_rgba_block = match lz4_flex::decompress_size_prepended(&block.data) {
                Ok(data) => data,
                Err(e) => {
                    println!("LZ4 解压失败: {}", e);
                    continue;
                }
            };

            let expected_size = (block.width * block.height * 4) as usize;
            if raw_rgba_block.len() != expected_size {
                 println!("警告: 差分块解压后大小不匹配, 期望: {}, 实际: {}", expected_size, raw_rgba_block.len());
                 continue;
            }

            // 应用原始 RGBA 数据到主缓冲区
            let start_y = block.y as usize;
            let end_y = (block.y + block.height) as usize;
            let start_x = block.x as usize * 4; // RGBA
            let block_width_bytes = block.width as usize * 4;

            let mut data_offset = 0;
            for y in start_y..end_y {
                if y < self.screen_height as usize {
                    let row_start = y * self.screen_width as usize * 4 + start_x;
                    let row_end = row_start + block_width_bytes;
                    
                    if row_end <= self.frame_buffer.len() && 
                       data_offset + block_width_bytes <= raw_rgba_block.len() {
                        self.frame_buffer[row_start..row_end]
                            .copy_from_slice(&raw_rgba_block[data_offset..data_offset + block_width_bytes]);
                    }
                    data_offset += block_width_bytes;
                }
            }
        }
    }

    // 修改：接收帧时只入队，不立即处理
    pub fn update_frame(&mut self, frame: ScreenFrame) {
        // 丢弃过老的帧，保持队列大小
        if self.frame_queue.len() >= self.max_queue_size {
            // 智能丢帧：优先保留完整帧
            if !frame.is_full_frame {
                // 如果新帧是差分帧，尝试丢弃旧的差分帧
                let mut removed = false;
                for i in 0..self.frame_queue.len() {
                    if !self.frame_queue[i].is_full_frame {
                        self.frame_queue.remove(i);
                        removed = true;
                        break;
                    }
                }
                if !removed {
                    self.frame_queue.pop_front();
                }
            } else {
                self.frame_queue.pop_front();
            }
            println!("警告: 帧队列已满，丢弃旧帧");
        }
        
        self.frame_queue.push_back(frame);
        self.queue_size = self.frame_queue.len();
    }


    // 新增：按固定帧率处理队列中的帧
    pub fn process_next_frame(&mut self) -> bool {
        if !self.is_capturing {
            return false;
        }
        
        let now = Instant::now();
        
        // 帧率控制：检查是否到达处理时机
        if let Some(last_time) = self.last_process_time {
            let target_duration = Duration::from_millis(1000 / self.frame_rate as u64);
            if now.duration_since(last_time) < target_duration {
                return false;  // 还没到处理时间
            }
        }
        
        // 从队列取出一帧处理
        if let Some(frame) = self.frame_queue.pop_front() {
            self.queue_size = self.frame_queue.len();
            self.last_process_time = Some(now);
            self.process_frame_internal(frame);
            
            // FPS 统计
            self.fps_counter += 1;
            if let Some(start_time) = self.fps_start_time {
                if now.duration_since(start_time) >= Duration::from_secs(1) {
                    self.current_fps = self.fps_counter as u32;
                    self.fps_counter = 0;
                    self.fps_start_time = Some(now);
                }
            } else {
                self.fps_start_time = Some(now);
            }
            
            true  // 处理了一帧
        } else {
            false  // 队列为空
        }
    }

    // 内部方法：实际处理帧数据（从原 update_frame 中提取）
    fn process_frame_internal(&mut self, frame: ScreenFrame) {
        if !self.received_first_frame || frame.is_full_frame {
            match lz4_flex::decompress_size_prepended(&frame.data) {
                Ok(raw_rgba) => {
                    if raw_rgba.len() == (self.screen_width * self.screen_height * 4) as usize {
                        self.frame_buffer = raw_rgba;
                        self.received_first_frame = true;
                        self.update_image_handle();
                        // println!("收到完整帧 (解压后大小: {} bytes\t分辨率: {}x{})", 
                        //     self.frame_buffer.len(), 
                        //     self.screen_width, 
                        //     self.screen_height
                        // );
                    } else {
                        println!("警告: 帧数据大小不匹配，期望: {}, 实际: {}", 
                            (self.screen_width * self.screen_height * 4) as usize, 
                            frame.data.len()
                        );
                    }
                },
                Err(e) => {
                    println!("错误: 完整帧 LZ4 解压失败: {}", e);
                }
            }
        } else {
            // 差分帧
            if !frame.diff_blocks.is_empty() {
                self.apply_diff_blocks(&frame.diff_blocks);
                self.update_image_handle();
            }
        }
        
        self.current_frame = Some(frame);
    }

    // 更新iced图像句柄
    fn update_image_handle(&mut self) {
        if self.frame_buffer.is_empty() || self.screen_width == 0 || self.screen_height == 0 {
            return;
        }

        let mut frame_buffer_owned = mem::take(&mut self.frame_buffer);

        let result = self.convert_rgba_to_jpeg_without_clone(&mut frame_buffer_owned);

        //将所有权还给 self.frame_buffer
        self.frame_buffer = frame_buffer_owned;

        // 将RGBA缓冲区转换为image::Handle
        if let Some(jpeg_data) = result {
            self.image_handle = Some(image::Handle::from_bytes(jpeg_data));
        }
    }

    // 将RGBA数据转换为PNG格式
    fn convert_rgba_to_jpeg_without_clone(&self, raw_rgba_data: &mut Vec<u8>) -> Option<Vec<u8>> {
        use std::io::Cursor;
        
        // 创建一个内存缓冲区用于存储PNG数据
        let mut png_data  = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        
        // 使用image库来编码PNG
        let img = match ::image::RgbaImage::from_raw(
            self.screen_width, 
            self.screen_height, 
            raw_rgba_data.to_vec()
        ) {
            Some(img) => img,
            None => {
                println!("无法创建RgbaImage");
                return None;
            }
        };

        match img.write_to(&mut cursor, ::image::ImageFormat::Png) {
            Ok(_) => Some(png_data),
            Err(e) => {
                println!("PNG编码失败: {}", e);
                None
            }
        }
    }

    pub fn view(&self, _window_id: iced::window::Id) -> Element<MonitorMessage> {
        let control_panel = row![
            button(text(if self.is_capturing { "停止捕获" } else { "开始捕获" }).center())
                .style(if self.is_capturing { button::danger } else { button::success })
                .on_press(if self.is_capturing { 
                    MonitorMessage::StopCapture 
                } else { 
                    MonitorMessage::StartCapture 
                })
                .width(Length::Fixed(120.0)),
            text(format!("分辨率: {}x{} | 缩放: {:.0}% | 帧数: {} | 缓冲: {}/{} ({:.1}s)", 
                self.screen_width, 
                self.screen_height,
                self.window_scale * 100.0,
                self.current_fps,
                self.queue_size,
                self.max_queue_size,
                self.queue_size as f32 / self.target_fps as f32  // 显示秒数
            )).size(12),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let screen_display = if !self.is_capturing {
            // 未开始捕获：显示提示文字
            container(
                column![
                    text("点击\"开始捕获\"按钮开始监控").size(18).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                    }),
                    text(format!("目标客户端: {}", self.peer_addr)).size(12).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    }),
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
                ..Default::default()
            })
            .into()
        } else if !self.received_first_frame {
            // 已开始捕获但未收到第一帧：显示连接中
            container(
                column![
                    text("正在连接客户端...").size(18).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.8, 0.6, 0.2)),
                    }),
                    text(format!("目标客户端: {}", self.peer_addr)).size(12).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    }),
                    text("等待屏幕数据...").size(12).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    }),
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
                ..Default::default()
            })
            .into()
        } else if self.image_handle.is_some() {
            self.render_screen()
        } else {
            // 显示等待界面
            container(
                column![
                    text("等待屏幕数据...").size(18).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                    }),
                    text(format!("目标客户端: {}", self.peer_addr)).size(12).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    }),
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
                ..Default::default()
            })
            .into()
        };

        column![
            container(control_panel)
                .padding(10)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
                    border: Border {
                        color: Color::from_rgb(0.8, 0.8, 0.8),
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                })
                .width(Length::Fill),
            screen_display
        ]
        .into()
    }

    fn render_screen(&self) -> Element<MonitorMessage> {
        // 创建屏幕显示区域
        let scaled_width = (self.screen_width as f32 * self.window_scale) as u16;
        let scaled_height = (self.screen_height as f32 * self.window_scale) as u16;

        // 显示屏幕内容的占位区域
        let screen_container = container(
            container(
                image::viewer(self.image_handle.as_ref().unwrap().clone())
                    .width(Length::Fixed(scaled_width as f32))
                    .height(Length::Fixed(scaled_height as f32))
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.3))),
                border: Border {
                    color: Color::from_rgb(0.4, 0.4, 0.5),
                    width: 2.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
        )
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.05, 0.05, 0.05))),
            ..Default::default()
        });

        screen_container.into()
    }
}

// 全局Monitor消息接收器
pub static G_MONITOR_MESSAGE_RECEIVER: Lazy<Arc<Mutex<Option<crossbeam_channel::Receiver<MonitorUpdate>>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

// 初始化Monitor消息通道
pub fn initialize_monitor_channel() {
    let (sender, receiver) = crossbeam_channel::unbounded::<MonitorUpdate>();
    *G_MONITOR_MESSAGE_RECEIVER.lock().unwrap() = Some(receiver);
    *G_MONITOR_MESSAGE_SENDER.lock().unwrap() = Some(sender);
    println!("Monitor消息通道初始化完成");
}

pub fn send_monitor_update(update: MonitorUpdate) {
    if let Some(sender) = G_MONITOR_MESSAGE_SENDER.lock().unwrap().as_ref() {
        let _ = sender.send(update);
    }
}