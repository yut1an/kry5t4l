use chrono::TimeZone;
use iced::{
    widget::{button, column, container, row, scrollable, text, Column}, 
    Alignment, Background, Border, Color, Element, Length, Padding, Theme
};
use kry5t4l_share::modules::{protocol::{FileTransfer, Message}, CommandType};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, io::Read, net::SocketAddr, path::Path, sync::{Arc, Mutex}};


use crate::{modules::network::send_command_to, CHINESE_FONT, EMOJI_FONT};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub dir: bool,
    pub size: Option<String>,
    pub modified: Option<String>,
    #[serde(default)]
    pub son: Vec<FileEntry>,
    #[serde(skip)]
    pub expanded: bool,
    #[serde(skip)]
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
pub enum ExplorerMessage {
    ToggleExpand(String),
    NavigateTo(String),
    SelectItem(String),
    DoubleClickItem(String),
    GoBack,
    Upload,
    UploadResult(String, bool, String), // src_path, success, message
    ShowDownloadDialog,
    CloseDownloadDialog,
    CloseNotification,
    DownloadFile(String),
    SortBy(SortKey),
}

#[derive(Debug, Clone)]
pub struct Explorer {
    pub client_id: String,
    pub peer_addr: SocketAddr,
    pub title: String,
    pub root_entries: Vec<FileEntry>,
    pub current_path: String,
    pub selected_item: Option<String>,
    pub history: Vec<String>,
    pub sort_key: SortKey,
    pub sort_direction: SortDirection,
    pub is_loading: bool,
    pub show_download_dialog: bool,
}

impl Explorer {

    pub fn new(client_id: String, peer_addr: SocketAddr) -> Self {
        Self {
            client_id,
            peer_addr,
            title: "正在解析".to_string(),
            root_entries: Vec::new(),
            current_path: "C:\\".to_string(),
            selected_item: None,
            history: vec![],
            sort_key: SortKey::Name,
            sort_direction: SortDirection::Ascending,
            is_loading: true,
            show_download_dialog: false,
        }
    }

    pub fn update_from_json(&mut self, json_data: &str) {
        // 解析JSON数据
        let mut entries: Vec<FileEntry> = match serde_json::from_str(json_data) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to parse JSON: {}", e);
                vec![]
            }
        };

        // 修复路径信息
        fix_entries(&mut entries, "");

        self.root_entries = entries;
        self.is_loading = false;
        self.title = format!("Explorer - {}", self.peer_addr);

        // 展开当前路径
        expand_path(&mut self.root_entries, &self.current_path);
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn update(&mut self, message: ExplorerMessage) {
        match message {
            ExplorerMessage::ToggleExpand(path) => {
                                toggle_expand(&mut self.root_entries, &path);
                            }
            ExplorerMessage::NavigateTo(path) => {
                                self.history.push(self.current_path.clone());
                                self.current_path = path;
                                self.selected_item = None;
                                expand_path(&mut self.root_entries, &self.current_path);
                            }
            ExplorerMessage::SelectItem(path) => {
                                self.selected_item = Some(path);
                            }
            ExplorerMessage::DoubleClickItem(path) => {
                                if let Some(entry) = find_entry(&self.root_entries, &path) {
                                    if entry.dir {
                                        self.history.push(self.current_path.clone());
                                        self.current_path = path;
                                        self.selected_item = None;
                                        expand_path(&mut self.root_entries, &self.current_path);
                                    }
                                }
                            }
            ExplorerMessage::GoBack => {
                                if let Some(prev) = self.history.pop() {
                                    self.current_path = prev;
                                    self.selected_item = None;
                                }
                            }
            ExplorerMessage::Upload => {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    println!("用户选择的文件: {}", path.display());
                                    let mut file_data = Vec::new();
                                    let mut status = String::from("Success");
                                    match File::open(&path) {
                                        Ok(mut file) => {
                                            if let Err(e) = file.read_to_end(&mut file_data) {
                                                println!("Error: {}", e);
                                                file_data.clear();
                                                status = format!("Error: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            println!("Error: {}", e);
                                            status = format!("Error: {}", e);
                                        }
                                    }

                                    let src_path = path.display().to_string();

                                    let ft = FileTransfer {
                                        src_path: src_path.clone(),
                                        dst_path: self.current_path.clone(),
                                        file_size: file_data.len() as u64,
                                        file_data,
                                        status,
                                    };

                                    let mut buf = vec![];
                                    buf.push(CommandType::Upload.to_u8());
                                    let mut vec_u8 = ft.to_bytes();
                                    buf.append(&mut vec_u8);
                                    //println!("peer_addr: {} \n vec_u8: {:?}", &self.peer_addr, &buf);

                                    let _ = send_command_to(&self.peer_addr, &buf);

                                    let file_size = if let Ok(metadate) = std::fs::metadata(src_path.clone()) {
                                        metadate.len()
                                    } else {
                                        0
                                    };

                                    let upload_request = UploadRequest {
                                        client_id: self.client_id.clone(),
                                        file_name: path.file_name().unwrap().to_string_lossy().to_string(),
                                        file_path: src_path.clone(),
                                        file_size, 
                                        upload_time: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs(),
                                        target_directory: self.current_path.clone(),
                                    };

                                    add_upload_request(src_path.clone(), upload_request);

                                }
                            }
            ExplorerMessage::ShowDownloadDialog => {
                                self.show_download_dialog = true;
                            }
            ExplorerMessage::CloseDownloadDialog => {
                                self.show_download_dialog = false;
                            }
            ExplorerMessage::DownloadFile(file_path) => {
                                println!("Download file: {}", file_path);
                                self.show_download_dialog = false;

                                let ft = FileTransfer {
                                    src_path: "".to_string(),
                                    dst_path: file_path.clone(),
                                    file_size: 0 as u64,
                                    file_data: vec![],
                                    status: "Success".to_string(),
                                };
                
                                let mut buf = vec![];
                                buf.push(CommandType::Download.to_u8());
                                let mut vec_u8 = ft.to_bytes();
                                buf.append(&mut vec_u8);
                                println!("peer_addr: {} \n vec_u8: {:?}", &self.peer_addr, &buf);

                                let _ = send_command_to(&self.peer_addr, &buf);
                            }
            ExplorerMessage::SortBy(new_key) => {
                                if self.sort_key == new_key {
                                    self.sort_direction = match self.sort_direction {
                                        SortDirection::Ascending => SortDirection::Descending,
                                        SortDirection::Descending => SortDirection::Ascending,  
                                    };
                                } else {
                                    self.sort_key = new_key;
                                    self.sort_direction = SortDirection::Ascending;
                                }
                            }
            ExplorerMessage::UploadResult(src_path, success, message) => {
                                if let Some(new_file) = handle_upload_result(&self.client_id.clone(), &src_path, success, &message) {
                                    self.add_file_to_directory(&new_file.path, new_file.clone());
                                }
                    }
            ExplorerMessage::CloseNotification => {
                clear_notification();
            }
        }
    }

    pub fn view(&self, _window_id: iced::window::Id) -> Element<ExplorerMessage> {
        if self.is_loading {
            return container(
                column![
                    text("正在解析").size(24).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    }),
                    text("请等待文件系统信息加载...").size(16).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                    }),
                ]
                .spacing(20)
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into();
        }

        // 顶部工具栏
        let toolbar = row![
            button(text("← 返回").center())
                .style(button::primary)
                .on_press(ExplorerMessage::GoBack)
                .width(Length::FillPortion(1)),
            text(&self.current_path)
                .size(14)
                .style(|_: &Theme| text::Style {
                    color: Some(Color::from_rgb(0.3, 0.3, 0.3)),
                })
                .width(Length::FillPortion(7)),
            button(text("上传").center())
                .style(button::secondary)
                .on_press(ExplorerMessage::Upload)
                .width(Length::FillPortion(1)),
            button(text("下载").center())
                .style(button::secondary)
                .on_press(ExplorerMessage::ShowDownloadDialog)
                .width(Length::FillPortion(1)),
        ]
        .spacing(15)
        .align_y(Alignment::Center);

        // 左侧树（只显示文件夹）
        let left_tree = container(
            scrollable(render_folder_tree(&self.root_entries))
                .height(Length::Fill)
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.98, 0.98, 0.98))),
            border: Border {
                color: Color::from_rgb(0.9, 0.9, 0.9),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .padding(10)
        .width(Length::Fixed(300.0))
        .height(Length::Fill);

        // 右侧文件列表（表格形式）
        let right_list = container(
            scrollable(render_file_table(
                &self.root_entries,
                &self.current_path,
                &self.sort_key,
                &self.sort_direction,
                &self.selected_item,
            ))
            .height(Length::Fill)
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::WHITE)),
            border: Border {
                color: Color::from_rgb(0.9, 0.9, 0.9),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill);

        // 主布局
        let main_content = column![
            container(toolbar)
                .padding(15)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
                    border: Border {
                        color: Color::from_rgb(0.85, 0.85, 0.85),
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                })
                .width(Length::Fill),
            row![left_tree, right_list]
                .spacing(10)
                .padding(10)
                .height(Length::Fill),
        ];

        if let Some(notification) = get_notification_for_client(&self.client_id) {
            if self.show_download_dialog {
                iced::widget::stack!(
                    main_content,
                    render_download_dialog(&self.root_entries, &self.current_path),
                    render_notification(notification)
                ).into()
            } else {
                iced::widget::stack!(
                    main_content,
                    render_notification(notification)
                ).into()
            }
        } else {
            if self.show_download_dialog {
                iced::widget::stack!(
                    main_content,
                    render_download_dialog(&self.root_entries, &self.current_path)
                ).into()
            } else {
                iced::widget::stack!(
                    main_content
                ).into()
            }
        }
    }

    // 添加文件到指定目录
    pub fn add_file_to_directory(&mut self, target_path: &str, new_file: FileEntry) {
        // 找到目标目录并添加文件

        if let Some(folder) = find_entry_mut(&mut self.root_entries, target_path) {
            // 检查是否已存在同名文件，如果存在则替换
            if let Some(existing_index) = folder.son.iter().position(|f| f.name == new_file.name) {
                folder.son[existing_index] = new_file;
            } else {
                folder.son.push(new_file);
            }
            
            // 重新排序文件列表
            folder.son.sort_by(|a, b| {
                if a.dir != b.dir {
                    return b.dir.cmp(&a.dir); // 文件夹在前
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            });
        }
    }
}

/// 渲染通知
fn render_notification<'a>(notification: NotificationInfo) -> Element<'a, ExplorerMessage> {
    let (icon, bg_color, border_color, text_color) = if notification.is_success {
        ("✓", Color::from_rgb(0.9, 1.0, 0.9), Color::from_rgb(0.0, 0.8, 0.0), Color::from_rgb(0.0, 0.6, 0.0))
    } else {
        ("✗", Color::from_rgb(1.0, 0.9, 0.9), Color::from_rgb(0.8, 0.0, 0.0), Color::from_rgb(0.7, 0.0, 0.0))
    };

    container(
        container(
            row![
                text(icon).font(EMOJI_FONT).size(16).style(move |_: &Theme| text::Style {
                    color: Some(text_color),
                }),
                text(notification.message).size(14).style(move |_: &Theme| text::Style {
                    color: Some(text_color),
                }),
                iced::widget::horizontal_space(),
                button(text("✕").font(EMOJI_FONT).size(12))
                    .style(button::text)
                    .on_press(ExplorerMessage::CloseNotification)
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        )
        .padding(15)
        .width(Length::Fixed(400.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg_color)),
            border: Border {
                color: border_color,
                width: 2.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Top)
    .padding(20)
    .into()
}

/// 渲染左侧文件夹树（只显示文件夹）
fn render_folder_tree(entries: &[FileEntry]) -> Column<ExplorerMessage> {
    let mut col = column![];
    for entry in entries {
        if entry.dir {

            let icon = text("📁").font(EMOJI_FONT);
            let name = text(&entry.name).font(CHINESE_FONT);

            let folder_row = row![

                button(text(if entry.expanded { "▼" } else { "▶" }).font(EMOJI_FONT))
                    .style(button::text)
                    .on_press(ExplorerMessage::ToggleExpand(entry.path.clone())),
                
                // 将图标和文件名放在同一个 row 里
                button(row![icon, name])
                    .style(button::text)
                    .on_press(ExplorerMessage::NavigateTo(entry.path.clone())),
            ]
            .spacing(5)
            .align_y(Alignment::Center);
            
            col = col.push(folder_row);
            
            if entry.expanded {
                col = col.push(
                    container(render_folder_tree(&entry.son))
                        .padding(Padding::new(0.0).left(20.0))
                );
            }
        }
    }
    col.spacing(2)
}

/// 渲染右侧文件表格
fn render_file_table<'a>(
    entries: &'a [FileEntry],
    path: &'a str,
    sort_key: &'a SortKey,
    sort_direction: &'a SortDirection,
    selected_item: &'a Option<String>,
) -> Column<'a, ExplorerMessage> {
    let mut col = column![];

    // 表头
    let header = row![
        container(
            button("名称")
                .style(button::text)
                .on_press(ExplorerMessage::SortBy(SortKey::Name))
        ).width(Length::FillPortion(4)),
        container(
            button("修改时间")
                .style(button::text)
                .on_press(ExplorerMessage::SortBy(SortKey::Modified))
        ).width(Length::FillPortion(2)),
        container(
            button("大小")
                .style(button::text)
                .on_press(ExplorerMessage::SortBy(SortKey::Size))
        ).width(Length::FillPortion(1)),
    ]
    .spacing(10);
    
    col = col.push(
        container(header)
            .padding(8)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.92, 0.92, 0.92))),
                border: Border {
                    color: Color::from_rgb(0.8, 0.8, 0.8),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
    );

    if let Some(folder) = find_entry(entries, path) {
        let mut children = folder.son.clone();

        // 文件夹在前 + 排序
        children.sort_by(|a, b| {
            if a.dir != b.dir {
                return b.dir.cmp(&a.dir); // 文件夹在前
            }
            let ordering = match sort_key {
                SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortKey::Size => {
                    let sa = parse_size(a.size.as_deref());
                    let sb = parse_size(b.size.as_deref());
                    sa.cmp(&sb)
                }
                SortKey::Modified => a.modified.cmp(&b.modified),
            };

            if *sort_direction == SortDirection::Descending {
                ordering.reverse()
            } else {
                ordering
            }

        });

        for child in children {
            let size = child.size.clone().unwrap_or_default();
            let modified = child.modified.clone().unwrap_or_default();
            
            let is_selected = selected_item.as_ref() == Some(&child.path);
            
            let icon = if child.dir {
                text("📁").font(EMOJI_FONT)
            } else {
                text("📄").font(EMOJI_FONT)
            };

            let name = text(child.name.clone()).font(CHINESE_FONT);

            let icon_and_name = row![icon, name]
                .spacing(5)
                .align_y(Alignment::Center);

            let file_row = row![
                container(icon_and_name)
                    .width(Length::FillPortion(4))
                    .padding(8),
                container(text(modified))
                    .width(Length::FillPortion(2))
                    .padding(8),
                container(text(size))
                    .width(Length::FillPortion(1))
                    .padding(8),
            ]
            .spacing(10);

            let row_container = container(file_row)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(
                        if is_selected {
                            Color::from_rgb(0.85, 0.95, 1.0)
                        } else {
                            Color::TRANSPARENT
                        }
                    )),
                    border: Border {
                        color: if is_selected {
                            Color::from_rgb(0.5, 0.7, 1.0)
                        } else {
                            Color::from_rgb(0.9, 0.9, 0.9)
                        },
                        width: if is_selected { 2.0 } else { 1.0 },
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                });


            let clickable_row = button(row_container)
                .style(button::text)
                .on_press(ExplorerMessage::SelectItem(child.path.clone()))
                .on_press_maybe(Some(ExplorerMessage::DoubleClickItem(child.path.clone())))
                .width(Length::Fill);

            col = col.push(clickable_row);
        }
    }
    
    col.spacing(2)
}

/// 渲染下载对话框
fn render_download_dialog<'a>(entries: &'a [FileEntry], current_path: &'a str) -> Element<'a, ExplorerMessage> {
    // 获取当前路径下的所有文件（不包括文件夹）
    let files = if let Some(folder) = find_entry(entries, current_path) {
        folder.son.iter()
            .filter(|entry| !entry.dir)
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let dialog_content = column![
        // 对话框标题
        container(
            row![
                text("选择要下载的文件").size(18).style(|_: &Theme| text::Style {
                    color: Some(Color::from_rgb(0.2, 0.2, 0.2)),
                }),
                iced::widget::horizontal_space(),
                button(text("✕").font(EMOJI_FONT).size(16))
                    .style(button::text)
                    .on_press(ExplorerMessage::CloseDownloadDialog)
            ]
        )
        .padding(15)
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
            border: Border {
                color: Color::from_rgb(0.8, 0.8, 0.8),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        }),
        
        // 文件列表
        container(
            if files.is_empty() {
                container(
                    text("当前目录下没有文件").size(14).style(|_: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                    })
                )
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .height(200)
                .width(Length::Fill)
            } else {
                container(
                    scrollable({
                        let mut file_list = column![];

                        for file in files {
                            let file_row = container(
                                row![
                                    text("📄").font(EMOJI_FONT),
                                    text(&file.name).font(CHINESE_FONT),
                                    iced::widget::horizontal_space(),
                                    text(file.size.as_deref().unwrap_or("")).size(12).style(|_: &Theme| text::Style {
                                        color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                                    }),
                                ]
                                .spacing(10)
                                .align_y(Alignment::Center)
                            )
                            .padding(10)
                            .width(Length::Fill)
                            .style(|_: &Theme| container::Style {
                                background: Some(Background::Color(Color::TRANSPARENT)),
                                border: Border {
                                    color: Color::from_rgb(0.9, 0.9, 0.9),
                                    width: 1.0,
                                    radius: 4.0.into(),
                                },
                                ..Default::default()
                            });

                            let clickable_row = button(file_row)
                                .style(button::text)
                                .on_press(ExplorerMessage::DownloadFile(file.path.clone()))
                                .width(Length::Fill);

                            file_list = file_list.push(clickable_row);
                        }

                        file_list.spacing(5)
                    })
                    .height(300)
                )
            }
        )
        .padding(15),
        
        // 底部按钮
        container(
            row![
                iced::widget::horizontal_space(),
                button(text("取消").font(CHINESE_FONT).center())
                    .style(button::secondary)
                    .on_press(ExplorerMessage::CloseDownloadDialog)
                    .width(Length::Fixed(100.0)),
            ]
            .spacing(10)
        )
        .padding([10, 15])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
            border: Border {
                color: Color::from_rgb(0.8, 0.8, 0.8),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        }),
    ];

    // 模态背景 + 对话框
    container(
        container(dialog_content)
            .width(Length::Fixed(500.0))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::WHITE)),
                border: Border {
                    color: Color::from_rgb(0.6, 0.6, 0.6),
                    width: 2.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.3))),
        ..Default::default()
    })
    .into()
}

/// 解析 "12.3KB" → 12300
fn parse_size(size: Option<&str>) -> u64 {
    if let Some(s) = size {
        if let Some(num) = s.replace("KB", "").trim().parse::<f64>().ok() {
            return (num * 1024.0) as u64;
        }
    }
    0
}

/// 查找节点
fn find_entry<'a>(entries: &'a [FileEntry], path: &str) -> Option<&'a FileEntry> {
    for entry in entries {
        if entry.path == path {
            return Some(entry);
        }
        if let Some(found) = find_entry(&entry.son, path) {
            return Some(found);
        }
    }
    None
}

/// 查找节点
fn find_entry_mut<'a>(entries: &'a mut [FileEntry], path: &str) -> Option<&'a mut FileEntry> {
    for entry in entries {
        if entry.path == path {
            return Some(entry);
        }
        if let Some(found) = find_entry_mut(&mut entry.son, path) {
            return Some(found);
        }
    }
    None
}

/// 展开/收起
fn toggle_expand(entries: &mut [FileEntry], path: &str) -> bool {
    for entry in entries {
        if entry.path == path {
            entry.expanded = !entry.expanded;
            return true;
        }
        if toggle_expand(&mut entry.son, path) {
            return true;
        }
    }
    false
}

/// 展开指定路径
fn expand_path(entries: &mut [FileEntry], path: &str) -> bool {
    for entry in entries {
        if entry.path == path {
            entry.expanded = true;
            return true;
        }
        if expand_path(&mut entry.son, path) {
            entry.expanded = true;
            return true;
        }
    }
    false
}

/// 修复路径信息
fn fix_entries(entries: &mut [FileEntry], parent: &str) {
    for e in entries.iter_mut() {
        e.expanded = false;
        e.path = if parent.is_empty() {
            format!("{}\\", e.name)
        } else {
            format!("{}{}\\", parent, e.name)
        };
        fix_entries(&mut e.son, &e.path);
    }
}


use crossbeam_channel::{unbounded, Sender, Receiver};

lazy_static::lazy_static! {
// 全局Explorer消息发送器
    pub static ref G_EXPLORER_MESSAGE_SENDER: Arc<Mutex<Option<Sender<ExplorerUpdate>>>> = 
        Arc::new(Mutex::new(None));
    pub static ref G_EXPLORER_MESSAGE_RECEIVER: Arc<Mutex<Option<Receiver<ExplorerUpdate>>>> = 
        Arc::new(Mutex::new(None));

    pub static ref G_UPLOAD_TRACKER: Arc<Mutex<HashMap<String, UploadRequest>>> = 
        Arc::new(Mutex::new(HashMap::new()));

    pub static ref G_NOTIFICATION_STATE: Arc<Mutex<Option<NotificationInfo>>> =
        Arc::new(Mutex::new(None));

}

#[derive(Debug, Clone)]
pub struct UploadRequest {
    pub client_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_size: u64,
    pub upload_time: u64,
    pub target_directory: String, // 目标上传目录
}

#[derive(Debug, Clone)]
pub struct NotificationInfo {
    pub message: String,
    pub is_success: bool,
    pub client_id: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum ExplorerUpdate {
    FileSystemInfo {
        client_id: String,
        json_data: String,
    },
    UploadResult {
        client_id: String,
        src_path: String,
        success: bool,
        message: String,
    },
}

// 初始化Explorer消息通道
pub fn initialize_explorer_channel() {
    let (sender, receiver) = unbounded::<ExplorerUpdate>();
    *G_EXPLORER_MESSAGE_SENDER.lock().unwrap() = Some(sender);
    *G_EXPLORER_MESSAGE_RECEIVER.lock().unwrap() = Some(receiver);
    println!("Explorer消息通道初始化完成");
}


pub fn send_explorer_update(update: ExplorerUpdate) {
    if let Some(sender) = G_EXPLORER_MESSAGE_SENDER.lock().unwrap().as_ref() {
        let _ = sender.send(update);
    }
}

// 添加上传请求到跟踪器
pub fn add_upload_request(upload_id: String, request: UploadRequest) {
    G_UPLOAD_TRACKER.lock().unwrap().insert(upload_id, request);
}

// 处理上传结果并设置通知 message -> E:\\迅雷下载\Cargo(4).toml
pub fn handle_upload_result(client_id: &str, upload_id: &str, success: bool, message: &str) -> Option<FileEntry> {
    let mut tracker = G_UPLOAD_TRACKER.lock().unwrap();
    
    if let Some(request) = tracker.remove(upload_id) {
        // 设置通知状态
        let notification = NotificationInfo {
            message: if success {
                format!("文件 '{}' 上传成功:\n{}", request.file_name, message)
            } else {
                format!("文件 '{}' 上传失败:\n{}", request.file_name, message)
            },
            is_success: success,
            client_id: client_id.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        *G_NOTIFICATION_STATE.lock().unwrap() = Some(notification);
        
        println!("Upload result: {} - {}", if success { "Success" } else { "Failed" }, message);

        let path = Path::new(message);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let mut path_str = path.parent().unwrap().to_string_lossy().to_string();
        //if !path_str.ends_with('\\') {
            path_str.push('\\');
        //}
        
        // 如果上传成功，返回新的文件条目
        if success {
            let file_size_str = format_file_size(request.file_size);
            let upload_time_str = format_timestamp(request.upload_time);
            
            let new_file = FileEntry {
                name,
                dir: false,
                size: Some(file_size_str),
                modified: Some(upload_time_str),
                son: vec![],
                expanded: false,
                path: format!("{}", path_str),
            };
            
            return Some(new_file);
        }
    }
    None
}

// 格式化文件大小
fn format_file_size(size: u64) -> String {
    if size < 1024 {
        format!("1.0 KB")
    } else {
        format!("{:.1} KB", size as f64 / 1024.0)
    }
    // if size < 1024 {
    //     format!("1.0 KB")
    // } else if size < 1024 * 1024 {
    //     format!("{:.1} KB", size as f64 / 1024.0)
    // } else if size < 1024 * 1024 * 1024 {
    //     format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    // } else {
    //     format!("{:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    // }
}

// 格式化时间戳
fn format_timestamp(timestamp: u64) -> String {

    let dt = chrono::prelude::Utc.timestamp_opt(timestamp as i64, 0).unwrap();
    let formatted = dt.format("%Y/%m/%d %H:%M").to_string();
    formatted
}

// 清除通知
pub fn clear_notification() {
    *G_NOTIFICATION_STATE.lock().unwrap() = None;
}

// 获取当前通知（供Explorer实例使用）
pub fn get_notification_for_client(client_id: &str) -> Option<NotificationInfo> {
    let notification_state = G_NOTIFICATION_STATE.lock().unwrap();
    if let Some(ref notification) = *notification_state {
        if notification.client_id == client_id {
            return Some(notification.clone());
        }
    }
    None
}