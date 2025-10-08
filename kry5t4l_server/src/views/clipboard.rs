use std::sync::{Arc, Mutex};
use crossbeam_channel::{Sender, Receiver};
use once_cell::sync::Lazy;



#[derive(Debug, Clone)]
pub struct ClipboardUpdate {
    pub client_id: String,
    pub content: String,
}

pub static G_CLIPBOARD_MESSAGE_SENDER: Lazy<Arc<Mutex<Option<Sender<ClipboardUpdate>>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub static G_CLIPBOARD_MESSAGE_RECEIVER: Lazy<Arc<Mutex<Option<Receiver<ClipboardUpdate>>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub fn initialize_clipboard_channel() {
    let (tx, rx) = crossbeam_channel::unbounded::<ClipboardUpdate>();
    *G_CLIPBOARD_MESSAGE_SENDER.lock().unwrap() = Some(tx);
    *G_CLIPBOARD_MESSAGE_RECEIVER.lock().unwrap() = Some(rx);
}

pub fn send_clipboard_update(update: ClipboardUpdate) {
    if let Some(sender) = G_CLIPBOARD_MESSAGE_SENDER.lock().unwrap().as_ref() {
        let _ = sender.send(update);
    }
}