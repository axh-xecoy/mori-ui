use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
    pub extra_headers: String,
}
impl Response {
    pub fn json(value: Value) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body: value.to_string(),
            extra_headers: String::new(),
        }
    }
    pub fn sse(events: &[Value]) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream",
            body: events
                .iter()
                .map(|v| format!("data: {v}\r\n\r\n"))
                .collect::<String>()
                + "data: [DONE]\r\n\r\n",
            extra_headers: String::new(),
        }
    }
}
pub fn server(
    responses: Vec<Response>,
) -> (
    String,
    mpsc::Receiver<(String, Value)>,
    std::thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        for response in responses {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut bytes = vec![];
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
            }
            let header = String::from_utf8(bytes).unwrap();
            let len = header
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            let mut body = vec![0; len];
            socket.read_exact(&mut body).unwrap();
            let value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            tx.send((header, value)).unwrap();
            write!(socket,"HTTP/1.1 {} Response\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",response.status,response.content_type,response.body.len(),response.extra_headers).unwrap();
            // Deliberately break UTF-8 characters and SSE delimiters across network reads.
            for chunk in response.body.as_bytes().chunks(7) {
                socket.write_all(chunk).unwrap();
            }
        }
    });
    (url, rx, handle)
}
