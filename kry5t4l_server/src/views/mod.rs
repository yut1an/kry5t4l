use std::sync::{Arc, Mutex};

use iced::{border::Radius, widget::{button, column, container, image, row, text, Space}, Background, Border, Color, Element, Length};

use crate::{views::{
    hosts::{HostsMessage, HostsState}, listens::{ListensMessgae, ListensState}
}};
use crossbeam_channel::{Sender, Receiver};


pub mod hosts;
pub mod listens;
pub mod shell;
pub mod explorer;
pub mod monitor;
pub mod clipboard;

lazy_static::lazy_static! {
    pub static ref G_APP_MESSAGE_SENDER: Arc<Mutex<Option<Sender<Kry5t4lMessage>>>> = 
        Arc::new(Mutex::new(None));
    pub static ref G_APP_MESSAGE_RECEIVER: Arc<Mutex<Option<Receiver<Kry5t4lMessage>>>> = 
        Arc::new(Mutex::new(None));
}

#[derive(Debug, Clone)]
pub struct Kry5t4lState {
    current_view: Kry5t4lView,
    pub hosts_state: HostsState,
    listens_state: ListensState,
    sidebar_collapsed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kry5t4lView {
    Hosts,
    Listens,
}

#[derive(Debug, Clone)]
pub enum Kry5t4lMessage {
    ToggleSidebar,
    SwitchView(Kry5t4lView),
    HostsMessage(HostsMessage),
    ListensMessgae(ListensMessgae),
}

impl Kry5t4lState {

    pub fn new() -> Self {
        Self {
            current_view: Kry5t4lView::Hosts,
            hosts_state: HostsState::new(),
            listens_state: ListensState::new(),
            sidebar_collapsed: false,
        }
    }

    pub fn update(&mut self, message: Kry5t4lMessage) -> iced::Task<Kry5t4lMessage> {
        match message {
            Kry5t4lMessage::SwitchView(kry5t4l_view) => {
                                        self.current_view = kry5t4l_view;
                                        iced::Task::none()
                                    }
            Kry5t4lMessage::HostsMessage(msg) => {
                                        self.hosts_state.update(msg);
                                        iced::Task::none()
                                    }
            Kry5t4lMessage::ToggleSidebar => {
                                        self.sidebar_collapsed = !self.sidebar_collapsed;
                                        iced::Task::none()
                                    }
            Kry5t4lMessage::ListensMessgae(msg) => {
                                        self.listens_state.update(msg);
                                        iced::Task::none()
                                    }
        }
    }
    
    pub fn view(&self) -> Element<Kry5t4lMessage> {
        let sidebar = sidebar(self.current_view,self.sidebar_collapsed);

        let content = match self.current_view {
            Kry5t4lView::Hosts => {
                hosts::view(&self.hosts_state).map(Kry5t4lMessage::HostsMessage)
            },
            Kry5t4lView::Listens => {
                listens::view(&self.listens_state).map(Kry5t4lMessage::ListensMessgae)
            }
        };


        row![sidebar, content]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

}

fn sidebar(current_view: Kry5t4lView, collapsed: bool) -> Element<'static, Kry5t4lMessage> {
    let sidebar_width = if collapsed { 60 } else { 120 };

    let create_icon = |icon_path: &str, size: u16| -> Element<Kry5t4lMessage> {
        let handle = image::Handle::from_path(icon_path);
        image(handle)
            .width(size)
            .height(size)
            .into()
    };

    let header = if collapsed {
        // 收缩状态
        container(
            button(
                container(create_icon("./kry5t4l_server/assets/right.png", 256))
                        .width(40)
                        .height(40)
                        .align_x(iced::alignment::Horizontal::Center)
                        .align_y(iced::alignment::Vertical::Center)
            )
                .style(button::text)
                .width(Length::Fill)
                .on_press(Kry5t4lMessage::ToggleSidebar)
        )
        .padding(10)
    } else {
        container(
            column![
                create_icon("./kry5t4l_server/assets/logo.jpg", 128),
                Space::with_width(Length::Fill),
                button(
                    container(
                        create_icon("./kry5t4l_server/assets/left.png", 32)
                    )
                    .width(30)
                    .height(30)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center)
                )
                .style(button::text)
                .on_press(Kry5t4lMessage::ToggleSidebar)
            ]
            .align_x(iced::alignment::Horizontal::Center)
        )
        .padding([15,20])
    };

    let nav_items = column![
        sidebar_item("./kry5t4l_server/assets/hosts.png", "Hosts", Kry5t4lView::Hosts, current_view, collapsed),
        sidebar_item("./kry5t4l_server/assets/listens.png", "Listens", Kry5t4lView::Listens, current_view, collapsed),
    ]
    .spacing(5);

    let footer = if !collapsed {
        container(
            column![
                text("v1.0.0").size(10).color(Color::from_rgb(0.6, 0.6, 0.6)),
                text("© 2025").size(10).color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .align_x(iced::alignment::Horizontal::Center)
        )
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .padding(20)
    } else {
        container(Space::with_height(0))
    };
    
    let sidebar_content = column![
        header,
        Space::with_height(20),
        nav_items,
        Space::with_height(Length::Fill),
        footer
    ];

    container(sidebar_content)
        .width(sidebar_width)
        .height(Length::Fill)
        .style(|_| {
            container::Style {
                background: Some(Background::Color(Color::from_rgb(0.80, 0.88, 0.98))),
                border: Border { 
                    color: Color::from_rgb(0.2, 0.25, 0.3),
                    width: 0.0,
                    radius: Radius { top_left: 0.0, top_right: 0.0, bottom_right: 0.0, bottom_left: 0.0 } 
                },
                ..Default::default()

            }
        })
        .into()

}

// 侧边栏导航项
fn sidebar_item<'a>(
    icon_path: &'a str, 
    label: &'a str, 
    tab: Kry5t4lView, 
    active_tab: Kry5t4lView, 
    collapsed: bool
) -> Element<'a, Kry5t4lMessage> {
    let is_active = tab == active_tab;

    let create_icon = |size: u16| -> Element<Kry5t4lMessage> {
        let handle = image::Handle::from_path(icon_path);
        image(handle)
            .width(size)
            .height(size)
            .into()
    };
    
    let item_content = if collapsed {
        // 收缩状态：只显示图标
        container(
            create_icon(128)
        )
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .width(40)
        .height(40)
    } else {
        // 展开状态：只显示文字
        container(
            text(label).size(14)
        )
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding([12, 20])
        .width(Length::Fill)
    };

    let styled_content = container(item_content)
        .style(move |_theme| {
            let (bg_color, text_color) = if is_active {
                (
                    Color::from_rgb(0.2, 0.4, 0.8),  // 活跃背景色
                    Color::WHITE                       // 活跃文字色
                )
            } else {
                (
                    Color::TRANSPARENT,                // 普通背景色
                    Color::from_rgb(0.7, 0.7, 0.7)   // 普通文字色
                )
            };

            container::Style {
                background: Some(Background::Color(bg_color)),
                border: Border {
                    color: if is_active {
                        Color::from_rgb(0.3, 0.5, 0.9)
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if is_active && !collapsed { 3.0 } else { 0.0 },
                    radius: Radius { top_left: 6.0, top_right: 6.0, bottom_right: 6.0, bottom_left: 6.0 },
                },
                text_color: Some(text_color),
                ..Default::default()
            }
        });

    button(styled_content)
        .style(button::text)
        .width( Length::Fill )
        .on_press(Kry5t4lMessage::SwitchView(tab))
        .into()
}
