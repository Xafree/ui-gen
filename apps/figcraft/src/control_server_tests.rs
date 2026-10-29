#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use std::io::Read;
use std::net::Shutdown;
use std::sync::mpsc;
use std::time::Instant;

fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    client.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    (client, listener.accept().unwrap().0)
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The first line every connection must send.
fn authenticate(client: &mut TcpStream) {
    writeln!(client, "{}", json!({"auth": TOKEN})).unwrap();
}

fn queue() -> Queue {
    Arc::new(Mutex::new(Vec::new()))
}

fn context() -> CtxSlot {
    Arc::new(Mutex::new(None))
}

fn reply(reader: &mut BufReader<TcpStream>) -> Value {
    let mut line = String::new();
    assert!(reader.read_line(&mut line).unwrap() > 0);
    serde_json::from_str(&line).unwrap()
}

/// Build a `ui.inspect` control request with a throwaway reply channel.
fn request() -> ControlRequest {
    ControlRequest { id: json!(0), method: "ui.inspect".into(), params: json!({}), reply: mpsc::channel().0, state: Default::default() }
}

#[test]
fn rejects_an_oversized_unterminated_frame_without_dispatch() {
    let (mut client, server) = pair();
    let pending = queue();
    let q = pending.clone();
    let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
    authenticate(&mut client);
    // Generate bytes as they are written; no giant input string and no terminating newline.
    std::io::copy(&mut std::io::repeat(b' ').take((MAX_LINE + 1) as u64), &mut client).unwrap();
    let mut reader = BufReader::new(client);
    assert_eq!(reply(&mut reader), json!({"id": null, "ok": false, "error": "message too large"}));
    assert_eq!(reader.read(&mut [0]).unwrap(), 0);
    worker.join().unwrap().unwrap();
    assert!(pending.lock().unwrap().is_empty());
}

// covers: K2
#[test]
fn valid_frames_keep_their_order_ids_and_parameters() {
    let (mut client, server) = pair();
    let pending = queue();
    let q = pending.clone();
    let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
    authenticate(&mut client);
    client.write_all(b"\r\n{\"id\":\"test\",\"method\":\"engine.execute\",\"params\":{\"command\":\"shape.rectangle\"}}\r\n").unwrap();
    let mut reader = BufReader::new(client);
    let start = Instant::now();
    let r = loop {
        if let Some(r) = pending.lock().unwrap().pop() {
            break r;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(r.id, "test");
    assert_eq!(r.method, "engine.execute");
    assert_eq!(r.params, json!({"command": "shape.rectangle"}));
    r.reply.send(figcraft_ui_egui::ControlReply { id: r.id, result: Ok(json!({"id": 42})) }).unwrap();
    assert_eq!(reply(&mut reader), json!({"id": "test", "ok": true, "result": {"id": 42}}));
    reader.get_ref().shutdown(Shutdown::Write).unwrap();
    worker.join().unwrap().unwrap();
}

#[test]
fn a_line_that_is_not_json_ends_the_connection_before_anything_after_it_runs() {
    // What a browser sends when a web page posts to the port: the JSON line in its body never runs.
    let (mut client, server) = pair();
    let pending = queue();
    let q = pending.clone();
    let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
    let body = "{\"id\":1,\"method\":\"engine.execute\",\"params\":{\"command\":\"file.save\"}}\n";
    let http = format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    authenticate(&mut client);
    client.write_all(http.as_bytes()).unwrap();
    let mut reader = BufReader::new(client);
    assert_eq!(reply(&mut reader)["ok"], false);
    assert_eq!(reader.read(&mut [0]).unwrap(), 0, "the connection is closed");
    worker.join().unwrap().unwrap();
    assert!(pending.lock().unwrap().is_empty(), "nothing was dispatched");
}

#[test]
fn full_queue_rejects_before_dispatch_and_ui_queue_stays_bounded() {
    let (mut client, server) = pair();
    let pending = queue();
    pending.lock().unwrap().extend((0..MAX_QUEUED_REQUESTS).map(|_| request()));
    let mut ui_queue = (0..MAX_QUEUED_REQUESTS - 1).map(|_| request()).collect();
    drain_queue(&pending, &mut ui_queue);
    assert_eq!(ui_queue.len(), MAX_QUEUED_REQUESTS);
    assert_eq!(pending.lock().unwrap().len(), MAX_QUEUED_REQUESTS - 1);
    pending.lock().unwrap().push(request());
    drain_queue(&pending, &mut ui_queue);
    assert_eq!(pending.lock().unwrap().len(), MAX_QUEUED_REQUESTS);
    let q = pending.clone();
    let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
    authenticate(&mut client);
    client.write_all(b"{\"id\":1,\"method\":\"engine.execute\"}\n").unwrap();
    let mut reader = BufReader::new(client);
    assert_eq!(reply(&mut reader), json!({"id": 1, "ok": false, "error": "control request queue is full"}));
    assert_eq!(pending.lock().unwrap().len(), MAX_QUEUED_REQUESTS);
    ui_queue.clear();
    drain_queue(&pending, &mut ui_queue);
    assert_eq!(ui_queue.len(), MAX_QUEUED_REQUESTS);
    assert!(pending.lock().unwrap().is_empty());
    reader.get_ref().shutdown(Shutdown::Write).unwrap();
    worker.join().unwrap().unwrap();
}

/// Asserts a commit that writes the file and queues its reply wins the gate: the transport's
/// timeout branch consumes that reply and leaves the state `Replied`.
#[test]
fn a_committed_reply_wins_over_the_transport_timeout() {
    let dir = std::env::temp_dir().join(format!("figcraft-control-commit-{}", std::process::id()));
    let path = dir.join("shot.png");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let gate = RequestGate::new();
    let (tx, rx) = mpsc::channel();
    // The UI's final commit gets the gate first: the file and reply are both committed before the
    // transport's timeout branch can look at the gate.
    let committed = gate.commit(|| {
        std::fs::write(&path, b"png").unwrap();
        tx.send(ControlReply { id: json!(7), result: Ok(json!({"path": path.to_string_lossy()})) }).unwrap();
    });
    assert!(committed.is_some());
    assert!(path.is_file());
    let reply = after_timeout(&rx, &gate).expect("the queued commit is consumed");
    assert_eq!(reply.id, json!(7));
    assert!(reply.result.is_ok());
    assert_eq!(gate.state(), RequestState::Replied, "the timeout must not overwrite a commit");
    assert!(path.is_file(), "a winning commit keeps its file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Asserts a zero-duration wait reports `Timeout` and cancels the open gate.
#[test]
fn a_timeout_cancels_an_open_request() {
    let gate = RequestGate::new();
    let (_tx, rx) = mpsc::channel::<ControlReply>();
    assert!(matches!(wait_for_reply(&rx, &gate, Duration::ZERO), Err(WaitFailure::Timeout)));
    assert_eq!(gate.state(), RequestState::Cancelled, "nothing may write after the timeout");
}

/// Asserts a closed reply channel reports `Disconnected` and leaves the gate `Open`, not
/// cancelled.
#[test]
fn a_disconnected_reply_channel_is_not_reported_as_a_timeout() {
    let gate = RequestGate::new();
    let (tx, rx) = mpsc::channel::<ControlReply>();
    drop(tx);
    assert!(matches!(wait_for_reply(&rx, &gate, Duration::from_secs(1)), Err(WaitFailure::Disconnected)));
    assert_eq!(gate.state(), RequestState::Open, "a closed app is not a cancel decision");
}

#[test]
fn idle_and_partial_frame_connections_expire() {
    for bytes in [b"".as_slice(), b"{\"id\":"] {
        let (mut client, server) = pair();
        let (done, finished) = mpsc::channel();
        let pending = queue();
        let q = pending.clone();
        let worker = std::thread::spawn(move || done.send(serve(server, q, context(), Duration::from_millis(50), TOKEN)));
        client.write_all(bytes).unwrap();
        let err = finished.recv_timeout(Duration::from_secs(5)).unwrap().unwrap_err();
        assert!(matches!(err.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut));
        assert!(pending.lock().unwrap().is_empty());
        worker.join().unwrap().unwrap();
    }
}

/// A failed accept is followed by a wait before the next one, so a listener that keeps failing
/// does not spin, and the connections around the failures still come through.
#[test]
fn a_failed_accept_waits_before_the_next() {
    let failed = || Err(std::io::Error::other("no file descriptors left"));
    let incoming = vec![failed(), Ok(1), failed(), failed(), Ok(2)];
    let mut waits = 0;
    let connections: Vec<i32> = accepted(incoming.into_iter(), || waits += 1).collect();
    assert_eq!(connections, [1, 2]);
    assert_eq!(waits, 3, "one wait per failed accept");
}

#[test]
fn connection_limit_rejects_excess_clients_and_reuses_finished_slots() {
    let pending = queue();
    let ctx = context();
    let mut workers = Vec::new();
    let mut clients = Vec::new();
    for _ in 0..MAX_CONNECTIONS {
        let (client, server) = pair();
        spawn_connection(server, pending.clone(), ctx.clone(), &mut workers, Duration::from_secs(5), Arc::from(TOKEN));
        clients.push(client);
    }
    assert_eq!(workers.len(), MAX_CONNECTIONS);
    let (mut extra, server) = pair();
    spawn_connection(server, pending.clone(), ctx.clone(), &mut workers, Duration::from_secs(5), Arc::from(TOKEN));
    assert_eq!(extra.read(&mut [0]).unwrap(), 0);
    assert_eq!(workers.len(), MAX_CONNECTIONS);
    for client in clients {
        client.shutdown(Shutdown::Both).unwrap();
    }
    let start = Instant::now();
    while workers.iter().any(|worker| !worker.is_finished()) {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    let (mut client, server) = pair();
    spawn_connection(server, pending, ctx, &mut workers, Duration::from_secs(5), Arc::from(TOKEN));
    assert_eq!(workers.len(), 1);
    authenticate(&mut client);
    client.write_all(b"invalid\n").unwrap();
    assert_eq!(reply(&mut BufReader::new(client.try_clone().unwrap()))["ok"], false);
    // The server closes this connection after answering an invalid request, and shutting down
    // a socket whose peer has already closed is ENOTCONN on macOS and the BSDs where Linux
    // returns Ok. Closing the client is what the worker is waiting for, so the result is moot:
    // the other shutdowns in this file run while the peer is still open and stay checked.
    let _ = client.shutdown(Shutdown::Both);
    workers.pop().unwrap().join().unwrap();
}

#[test]
fn a_client_that_stops_reading_cannot_hold_a_worker() {
    let (mut client, server) = pair();
    let pending = queue();
    let q = pending.clone();
    let (done, finished) = mpsc::channel();
    let worker = std::thread::spawn(move || done.send(serve(server, q, context(), Duration::from_millis(100), TOKEN)));
    authenticate(&mut client);
    client.write_all(b"{\"id\":1,\"method\":\"document.inspect\"}\n").unwrap();
    let start = Instant::now();
    let r = loop {
        if let Some(r) = pending.lock().unwrap().pop() {
            break r;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    };
    r.reply.send(figcraft_ui_egui::ControlReply { id: r.id, result: Ok(json!("x".repeat(8 * 1024 * 1024))) }).unwrap();
    let err = finished.recv_timeout(Duration::from_secs(5)).unwrap().unwrap_err();
    assert!(matches!(err.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut));
    worker.join().unwrap().unwrap();
}

#[test]
fn a_limit_sized_frame_is_accepted_at_eof() {
    let (mut client, server) = pair();
    let pending = queue();
    let q = pending.clone();
    let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
    let message = b"{\"id\":1,\"method\":\"ui.inspect\"}";
    authenticate(&mut client);
    client.write_all(message).unwrap();
    std::io::copy(&mut std::io::repeat(b' ').take((MAX_LINE - message.len()) as u64), &mut client).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let start = Instant::now();
    let r = loop {
        if let Some(r) = pending.lock().unwrap().pop() {
            break r;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(r.id, 1);
    assert_eq!(r.method, "ui.inspect");
    r.reply.send(figcraft_ui_egui::ControlReply { id: r.id, result: Ok(json!({})) }).unwrap();
    assert_eq!(reply(&mut BufReader::new(client)), json!({"id": 1, "ok": true, "result": {}}));
    worker.join().unwrap().unwrap();
}

/// Asserts expired requests stop occupying the UI queue cap, a later request is admitted
/// and answered, and a window that finally draws discards its expired command before
/// dispatch.
#[test]
fn timed_out_requests_release_a_full_deferred_ui_queue() {
    use figcraft_ui_egui::{FigApp, NoServices, control, windows};

    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = FigApp::new(figcraft_engine::Session::new(), Box::new(NoServices));
    let window = windows::open(&mut app, &ctx, Some(0), None, None).unwrap();
    let pending = queue();
    let mut clients = Vec::new();
    let mut workers = Vec::new();
    for id in 0..MAX_QUEUED_REQUESTS {
        let (mut client, server) = pair();
        let q = pending.clone();
        workers.push(std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(1), TOKEN)));
        authenticate(&mut client);
        writeln!(client, "{}", json!({"id": id, "method": "engine.execute", "params": {"window": window, "command": "file.new"}})).unwrap();
        clients.push(BufReader::new(client));
    }
    let start = Instant::now();
    while pending.lock().unwrap().len() < MAX_QUEUED_REQUESTS {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    drain_queue(&pending, &mut app.control_queue);
    control::drain(&mut app, &ctx);
    assert_eq!(app.control_queue.len(), MAX_QUEUED_REQUESTS, "the other window has not drawn");
    for client in &mut clients {
        assert!(reply(client)["error"].as_str().unwrap().contains("timed out"));
        client.get_ref().shutdown(Shutdown::Write).unwrap();
    }
    assert!(app.control_queue.iter().all(ControlRequest::is_cancelled));
    let expired = app.control_queue.pop().unwrap();
    // Include expired requests in the socket queue, too: admission must reclaim their slots.
    pending.lock().unwrap().extend((0..MAX_QUEUED_REQUESTS).map(|_| {
        let r = request();
        r.state.cancel_if_open();
        r
    }));
    let (mut client, server) = pair();
    let q = pending.clone();
    workers.push(std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN)));
    authenticate(&mut client);
    client.write_all(b"{\"id\":100,\"method\":\"window.list\",\"params\":{\"window\":0}}\n").unwrap();
    let start = Instant::now();
    while !pending.lock().unwrap().iter().any(|r| r.id == 100) {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    drain_queue(&pending, &mut app.control_queue);
    assert_eq!(app.control_queue.len(), 1, "expired UI requests no longer occupy the cap");
    control::drain(&mut app, &ctx);
    let mut client = BufReader::new(client);
    assert_eq!(reply(&mut client)["ok"], true);
    client.get_ref().shutdown(Shutdown::Write).unwrap();
    assert!(app.control_queue.is_empty());
    // A window that finally draws must discard its expired command before dispatch, too.
    let documents = app.session.docs.len();
    app.windows.current = window;
    app.control_queue.push(expired);
    control::drain(&mut app, &ctx);
    assert!(app.control_queue.is_empty());
    assert_eq!(app.session.docs.len(), documents);
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
}

/// What the server wrote before it closed, and whether it closed (rather than timed out).
fn read_until_closed(mut client: TcpStream) -> Vec<u8> {
    let mut received = Vec::new();
    let start = Instant::now();
    // A reset by the peer also means the server closed the connection.
    let outcome = client.read_to_end(&mut received);
    if let Err(e) = &outcome {
        assert!(!matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut), "the server left the connection open");
    }
    assert!(start.elapsed() < Duration::from_secs(4), "the server left the connection open");
    received
}

fn assert_closed_without_dispatch(first_lines: &[u8]) {
    let (mut client, server) = pair();
    let pending = queue();
    let q = pending.clone();
    let worker = std::thread::spawn(move || serve(server, q, context(), Duration::from_secs(5), TOKEN));
    let command = b"{\"id\":1,\"method\":\"engine.execute\",\"params\":{\"command\":\"file.save\"}}\n";
    client.write_all(first_lines).unwrap();
    let _ = client.write_all(command);
    let received = String::from_utf8_lossy(&read_until_closed(client)).into_owned();
    let _ = worker.join().unwrap();
    assert!(pending.lock().unwrap().is_empty(), "nothing was dispatched");
    assert!(!received.contains("\"ok\":true"), "nothing was executed: {received}");
}

// covers: K2
#[test]
fn a_wrong_token_closes_the_connection_before_anything_runs() {
    assert_closed_without_dispatch(b"{\"auth\": \"not-the-token\"}\n");
    let almost = format!("{}\n", json!({"auth": &TOKEN[..TOKEN.len() - 1]}));
    assert_closed_without_dispatch(almost.as_bytes());
}

// covers: K2
#[test]
fn a_first_line_that_is_not_the_token_closes_the_connection_before_anything_runs() {
    // A client that skips the handshake and sends a command right away.
    assert_closed_without_dispatch(b"{\"id\":0,\"method\":\"ui.inspect\"}\n");
    // The token in the wrong place (inside a command) is not the handshake either.
    let smuggled = format!("{}\n", json!({"id": 0, "method": "ui.inspect", "auth": TOKEN, "params": {}}));
    assert_closed_without_dispatch(smuggled.as_bytes());
    // What a browser sends: the request line is not the handshake.
    assert_closed_without_dispatch(b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
}

// covers: K2
#[test]
fn the_token_is_random_hex_of_at_least_128_bits() {
    let (a, b) = (generate_token(), generate_token());
    assert!(a.len() >= 32 && a.chars().all(|c| c.is_ascii_hexdigit()), "{a}");
    assert_ne!(a, b, "two tokens must differ");
}

// covers: K2
#[test]
fn tokens_match_only_when_identical() {
    assert!(token_matches(TOKEN, TOKEN));
    assert!(!token_matches(TOKEN, ""));
    assert!(!token_matches("", TOKEN));
    assert!(!token_matches(TOKEN, &TOKEN[..TOKEN.len() - 1]));
    assert!(!token_matches(TOKEN, &format!("{TOKEN}0")));
    let flipped = format!("{}f", &TOKEN[..TOKEN.len() - 1]);
    assert!(!token_matches(TOKEN, &flipped));
    assert!(!token_matches("", ""), "an empty token never authenticates");
}

// covers: K2
#[test]
fn the_token_file_holds_the_token_and_is_private() {
    let dir = std::env::temp_dir().join(format!("figcraft-control-token-{}", std::process::id())).join("nested");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let token = generate_token();
    let path = write_token_file(&dir, 4242, &token).unwrap();
    assert!(path.starts_with(&dir));
    assert_eq!(path.file_name().unwrap(), "control-token-4242");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), token, "the exact token, no newline");
    // Starting again replaces the old token.
    let next = generate_token();
    assert_eq!(write_token_file(&dir, 4242, &next).unwrap(), path);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), next);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0, "nobody but the owner may read it");
    }
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

// covers: K3
#[test]
fn the_idle_timeout_is_not_reset_by_each_byte() {
    for authenticated in [false, true] {
        let (mut client, server) = pair();
        let (done, finished) = mpsc::channel();
        let pending = queue();
        let q = pending.clone();
        let started = Instant::now();
        let worker = std::thread::spawn(move || done.send(serve(server, q, context(), Duration::from_millis(300), TOKEN)));
        if authenticated {
            authenticate(&mut client);
        }
        // One byte every 50 ms and never a newline: each byte would restart a per-read timeout.
        let dribble = std::thread::spawn(move || {
            for _ in 0..60 {
                if client.write_all(b" ").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let outcome = finished.recv_timeout(Duration::from_millis(2000)).expect("the connection must be cut after the timeout, not kept alive by a slow trickle");
        assert!(started.elapsed() < Duration::from_millis(2000));
        let err = outcome.unwrap_err();
        assert!(matches!(err.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut), "{err:?}");
        assert!(pending.lock().unwrap().is_empty());
        worker.join().unwrap().unwrap();
        dribble.join().unwrap();
    }
}
