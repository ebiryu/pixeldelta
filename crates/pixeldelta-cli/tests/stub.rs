//! A one-connection-at-a-time HTTP server the storage and notification tests
//! point at, so the requests they send can be read back without a network.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{channel, Receiver};

/// A request the stub received.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// A canned reply.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn ok(body: &[u8]) -> Self {
        Reply {
            status: 200,
            body: body.to_vec(),
        }
    }

    pub fn status(status: u16) -> Self {
        Reply {
            status,
            body: Vec::new(),
        }
    }
}

/// A server that answers with the given replies, in order.
pub struct Stub {
    addr: SocketAddr,
    received: Receiver<Request>,
}

impl Stub {
    pub fn start(replies: Vec<Reply>) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let addr = listener.local_addr().expect("the bound address");
        let (sender, received) = channel();

        std::thread::spawn(move || {
            for reply in replies {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let mut stream = stream;
                let request = read_request(&mut stream);
                let _ = sender.send(request);
                write_reply(&mut stream, &reply);
            }
        });

        Stub { addr, received }
    }

    /// Base URL to point a client at.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The requests received so far, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.received.try_iter().collect()
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> Request {
    let mut reader = BufReader::new(stream.try_clone().expect("the stream clones"));

    let mut line = String::new();
    reader.read_line(&mut line).expect("a request line");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();

    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).expect("a header line");
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }

    let length: usize = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("the body");

    Request {
        method,
        target,
        headers,
        body,
    }
}

fn write_reply(stream: &mut std::net::TcpStream, reply: &Reply) {
    let head = format!(
        "HTTP/1.1 {} {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        reply.status,
        reason(reply.status),
        reply.body.len(),
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&reply.body);
    let _ = stream.flush();
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Unknown",
    }
}
