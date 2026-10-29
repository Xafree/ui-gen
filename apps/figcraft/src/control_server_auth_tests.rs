#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use std::io::Read;
use std::net::Shutdown;
use std::sync::mpsc;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ACK: &str = r#"{"id":null,"ok":true,"result":"authenticated"}"#;

fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    client.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    (client, listener.accept().unwrap().0)
}

fn queue() -> Queue {
    Arc::new(Mutex::new(Vec::new()))
}

fn context() -> CtxSlot {
    Arc::new(Mutex::new(None))
}

fn auth_line() -> String {
    format!("{}\n", json!({"auth": TOKEN}))
}

#[test]
fn a_good_token_is_answered_with_one_acknowledgement_line() {
    let (client, server) = pair();
    let worker = std::thread::spawn(move || serve(server, queue(), context(), Duration::from_secs(5), TOKEN));
    let mut client = client;
    client.write_all(auth_line().as_bytes()).unwrap();
    let mut reader = BufReader::new(client);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line.trim_end(), ACK);
    reader.get_ref().shutdown(Shutdown::Write).unwrap();
    let mut rest = String::new();
    assert_eq!(reader.read_line(&mut rest).unwrap(), 0, "nothing else follows the acknowledgement: {rest}");
    worker.join().unwrap().unwrap();
}

#[test]
fn a_request_sent_in_the_same_write_as_the_token_is_handled() {
    let (mut client, server) = pair();
    let pending = queue();
    let q = pending.clone();
    let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
    let both = format!("{}{}\n", auth_line(), json!({"id": 7, "method": "ui.inspect", "params": {}}));
    client.write_all(both.as_bytes()).unwrap();
    let start = Instant::now();
    let request = loop {
        if let Some(r) = pending.lock().unwrap().pop() {
            break r;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "the pipelined request was never queued");
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!((request.id.clone(), request.method.as_str()), (json!(7), "ui.inspect"));
    request.reply.send(figcraft_ui_egui::ControlReply { id: request.id, result: Ok(json!({"done": true})) }).unwrap();
    let mut reader = BufReader::new(client);
    let mut lines = Vec::new();
    for _ in 0..2 {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        lines.push(line.trim_end().to_string());
    }
    assert_eq!(lines[0], ACK);
    assert_eq!(serde_json::from_str::<Value>(&lines[1]).unwrap(), json!({"id": 7, "ok": true, "result": {"done": true}}));
    reader.get_ref().shutdown(Shutdown::Write).unwrap();
    worker.join().unwrap().unwrap();
}

#[test]
fn a_first_line_over_one_kib_is_refused_even_when_it_holds_the_token() {
    for padding in [MAX_AUTH_LINE + 1, 64 * 1024] {
        let (mut client, server) = pair();
        let pending = queue();
        let q = pending.clone();
        let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
        let first = format!("{}{}\n", json!({"auth": TOKEN}), " ".repeat(padding));
        let _ = client.write_all(first.as_bytes());
        let _ = client.write_all(b"{\"id\":1,\"method\":\"ui.inspect\"}\n");
        let mut received = Vec::new();
        let _ = client.read_to_end(&mut received);
        let received = String::from_utf8_lossy(&received);
        assert!(!received.contains("\"ok\":true"), "an oversized first line must not authenticate: {received}");
        worker.join().unwrap().unwrap();
        assert!(pending.lock().unwrap().is_empty(), "nothing was dispatched");
    }
}

#[test]
fn a_silent_connection_is_cut_after_the_authentication_delay_not_the_request_timeout() {
    assert!(AUTH_TIMEOUT <= Duration::from_secs(5));
    assert!(AUTH_TIMEOUT < TIMEOUT);
    let (_client, server) = pair();
    let (done, finished) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || done.send(serve(server, queue(), context(), TIMEOUT, TOKEN)));
    let err = finished
        .recv_timeout(AUTH_TIMEOUT + Duration::from_secs(3))
        .expect("a silent client holds its slot past the authentication delay")
        .unwrap_err();
    assert!(matches!(err.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut), "{err:?}");
    assert!(started.elapsed() < TIMEOUT / 2);
    worker.join().unwrap().unwrap();
}

#[test]
fn an_authenticated_connection_keeps_the_longer_timeout() {
    let (mut client, server) = pair();
    let (done, finished) = mpsc::channel();
    let worker = std::thread::spawn(move || done.send(serve(server, queue(), context(), AUTH_TIMEOUT * 2, TOKEN)));
    client.write_all(auth_line().as_bytes()).unwrap();
    let mut ack = String::new();
    BufReader::new(client.try_clone().unwrap()).read_line(&mut ack).unwrap();
    assert!(finished.recv_timeout(AUTH_TIMEOUT + Duration::from_millis(500)).is_err(), "the request timeout applies once authenticated");
    drop(client);
    worker.join().unwrap().unwrap();
}
