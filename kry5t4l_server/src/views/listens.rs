
use iced::{
    widget::{button, container, pick_list, row, scrollable, text, text_input, Column, Row, Space, column}, Alignment::{self, Center}, Background, Border, Color, Element, Font, Length::{self, Fill}, Theme};
use kry5t4l_share::modules::protocol::Protocol;

use crate::{modules::network::{add_listener, all_listener, remove_listener, Listener}, EMOJI_FONT};

#[derive(Debug, Clone)]
pub struct ListensState {
    listeners: Vec<Listener>,
    port_input: String,
    selected_protocol: Option<Protocol>,
    error_message: Option<String>,
    show_error_dialog: bool,
}


#[derive(Debug, Clone)]
pub enum ListensMessgae {
    AddListener,
    CloseDialog,
    ProtocolSelected(Protocol),
    PortInputChanged(String),
    RemoveListener(u8),
}

impl ListensState {

    pub fn new() -> Self {
        Self { 
            listeners: Vec::<Listener>::new(), 
            port_input: String::new(), 
            selected_protocol: None, 
            error_message: Some(String::new()),
            show_error_dialog: false,
        }
    }

    pub fn update(&mut self, message: ListensMessgae) {
        match message {
            ListensMessgae::AddListener => {
                if let Some(protocol) = &self.selected_protocol {
                    if let Ok(port) = self.port_input.parse::<u16>() {
                        if port > 0 && port <= 65535 {
                            match add_listener(protocol, port) {
                                Ok(_) => {
                                    self.listeners = all_listener();
                                    self.port_input.clear();
                                    self.error_message = None;
                                }
                                Err(e) => {
                                    self.error_message = Some(format!("添加监听器失败: {}", e));
                                    self.show_error_dialog = true;
                                }
                            }
                        } else {
                            self.error_message = Some("端口范围必须在 1-65535 之间".to_string());
                            self.show_error_dialog = true;
                        }
                    } else {
                        self.error_message = Some("请输入有效的端口号".to_string());
                        self.show_error_dialog = true;
                    }
                } else {
                    self.error_message = Some("请选择协议".to_string());
                    self.show_error_dialog = true;
                }
            }
            ListensMessgae::ProtocolSelected(protocol) => {
                self.selected_protocol = Some(protocol);
                self.error_message = None;
            }
            ListensMessgae::PortInputChanged(value) => {
                self.port_input = value;
                self.error_message = None;
            }
            ListensMessgae::RemoveListener(id) => {
                match remove_listener(id) {
                    Ok(_) => {
                        self.listeners = all_listener();
                    }
                    Err(e) => {
                        self.error_message = Some(format!("移除监听器失败: {}", e));
                        self.show_error_dialog = true;
                    }
                }
            }
            ListensMessgae::CloseDialog => {
                self.show_error_dialog = false;
            }
        }
    }
}

pub fn view(state: &ListensState) -> Element<ListensMessgae> {
    let protocol_options = vec![
        Protocol::TCP,
        Protocol::WS
    ];

    let add_controls = row![
        text("Protocol:").width(Length::Shrink),
        pick_list(
            protocol_options, 
            state.selected_protocol.clone(), 
            ListensMessgae::ProtocolSelected
        )
        .width(120)
        .placeholder("Choose :)"),
        Space::with_width(Length::Fixed(20.0)),
        text("Port:").width(Length::Shrink),
        text_input("3208", &state.port_input)
            .on_input(ListensMessgae::PortInputChanged)
            .on_submit(ListensMessgae::AddListener)
            .width(120),
        Space::with_width(Length::Fill),
        button(text("Add").center())
            .width(100)
            .on_press(ListensMessgae::AddListener)
    ]
    .spacing(10)
    .align_y(Center);

    let border = Border {
        color: Color::from_rgb(0.6, 0.6, 0.6),
        width: 1.0,
        radius: 0.0.into(),
    };

    let list_header = Row::new()
        .push(container(text("Port").size(12))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                text_color: Some(Color::WHITE),
                border,
                ..Default::default()
            })
            .padding([8, 6])
            .width(Length::FillPortion(1)))
        .push(container(text("Protocol").size(12))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                text_color: Some(Color::WHITE),
                border,
                ..Default::default()
            })
            .padding([8, 6])
            .width(Length::FillPortion(1)))
        .push(container(text("State").size(12))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                text_color: Some(Color::WHITE),
                border,
                ..Default::default()
            })
            .padding([8, 6])
            .width(Length::FillPortion(1)))
        .push(container(text("Operation").size(12))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                text_color: Some(Color::WHITE),
                border,
                ..Default::default()
            })
            .padding([8, 6])
            .width(Length::FillPortion(1)))
        .spacing(0);

    let mut listeners_column: Column<'_, ListensMessgae, _, _> = Column::new()
        .spacing(5);

        for listener in &state.listeners {
            let border = Border {
                color: Color::from_rgb(0.6, 0.6, 0.6),
                width: 1.0,
                radius: 0.0.into(),
            };

            let listener_row = Row::new()
                .push(container(text(listener.addr.port().to_string()).size(12))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(Color::WHITE)),
                        border,
                        ..Default::default()
                    })
                    .padding([12, 6])
                    .height(Length::Fixed(45.0))
                    .width(Length::FillPortion(1))
                    .align_y(Center))
                .push(container(text(format!("{:?}", listener.protocol)).size(12))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(Color::WHITE)),
                        border,
                        ..Default::default()
                    })
                    .padding([12, 6])
                    .height(Length::Fixed(45.0))
                    .width(Length::FillPortion(1))
                    .align_y(Center))
                .push(container(text("Running").size(12))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(Color::WHITE)),
                        text_color: Some(Color::from_rgb(0.2, 0.8, 0.2)),
                        border,
                        ..Default::default()
                    })
                    .padding([12, 6])
                    .height(Length::Fixed(45.0))
                    .width(Length::FillPortion(1))
                    .align_y(Center))
                .push(container(
                        button(text("🗑").font(Font::with_name("Segoe UI Emoji")).center())
                                .style(button::text)
                            .on_press(ListensMessgae::RemoveListener(listener.id))
                            .padding([2, 8])
                            .height(Length::Fixed(24.0)))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(Color::WHITE)),
                        border,
                        ..Default::default()
                    })
                    .padding([12, 6])
                    .height(Length::Fixed(45.0))
                    .width(Length::FillPortion(1))
                    .align_y(Center))
                .spacing(0);

            listeners_column = listeners_column.push(listener_row);
        }

    let listeners_section: container::Container<'_, ListensMessgae, _, _> = container(
        Column::new()
            .push(container(list_header))
            .push(listeners_column)
            .spacing(0)
    );
    
    let main_content = column![
        Space::with_height(2),
        add_controls,
        Space::with_height(2),
        listeners_section,
    ]
    .spacing(5);

    let scrollable_content = scrollable(main_content)
        .height(Fill)
        .width(Fill);

    if state.show_error_dialog {
        let err_str = state.error_message.clone().unwrap();
        iced::widget::stack!(
            container(scrollable_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(10)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        render_err_message(err_str)
        ).into()
    } else {
        iced::widget::stack!(
            container(scrollable_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(10)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        ).into()
    }

}

/// 渲染通知
fn render_err_message<'a>(error_message: String) -> Element<'a, ListensMessgae> {

    let (icon, bg_color, border_color, text_color) = 
        ("✗", Color::from_rgb(1.0, 0.9, 0.9), Color::from_rgb(0.8, 0.0, 0.0), Color::from_rgb(0.7, 0.0, 0.0));

    container(
        container(
            row![
                text(icon).font(EMOJI_FONT).size(16).style(move |_: &Theme| text::Style {
                    color: Some(text_color),
                }),
                text(error_message).size(14).style(move |_: &Theme| text::Style {
                    color: Some(text_color),
                }),
                iced::widget::horizontal_space(),
                button(text("✕").font(EMOJI_FONT).size(12))
                    .style(button::text)
                    .on_press(ListensMessgae::CloseDialog)
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