use std::{
    collections::{HashMap, VecDeque},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{atomic::AtomicBool, Arc, Condvar, Mutex},
    time::Duration,
};

use http::{
    header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE},
    Request, Response, StatusCode,
};