//! How the attached backend reads the app's answer to its token, and which token it may send where.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use figcraft_mcp::Attached;

fn app_answering(answer: Option<&'static str>, delay: Duration) -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut first = String::new();
        BufReader::new(stream.try_clone().unwrap()).read_line(&mut first).unwrap();
        std::thread::sleep(delay);
        if let Some(answer) = answer {
            writeln!(stream, "{answer}").unwrap();
        }
        let mut rest = Vec::new();
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = stream.read_to_end(&mut rest);
    });
    (port, app)
}

#[test]
fn an_acknowledgement_that_comes_late_is_still_waited_for() {
    let (port, app) = app_answering(Some(r#"{"id":null,"ok":true,"result":"authenticated"}"#), Duration::from_millis(600));
    let backend = Attached::port(port, "token").unwrap();
    drop(backend);
    app.join().unwrap();
}

#[test]
fn a_refusal_is_reported_as_an_authentication_failure() {
    let (port, app) = app_answering(Some(r#"{"id":null,"ok":false,"error":"nope"}"#), Duration::ZERO);
    let error = Attached::port(port, "token").err().unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("authentication failed"), "{error}");
    drop(app);
}

#[test]
fn a_full_app_is_not_reported_as_a_wrong_token() {
    let (port, app) = app_answering(Some(r#"{"id":null,"ok":false,"error":"too many connections"}"#), Duration::ZERO);
    let error = Attached::port(port, "token").err().unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::ConnectionRefused);
    assert!(error.to_string().contains("too many connections"), "{error}");
    drop(app);
}

#[test]
fn a_connection_closed_without_an_answer_says_so() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut first = String::new();
        BufReader::new(stream.try_clone().unwrap()).read_line(&mut first).unwrap();
        drop(stream);
    });
    let error = Attached::port(port, "token").err().unwrap();
    assert_ne!(error.kind(), std::io::ErrorKind::PermissionDenied, "a closed connection is not a wrong token: {error}");
    assert!(error.to_string().contains("closed the connection"), "{error}");
    assert!(error.to_string().contains("too many connections or not authenticated"), "{error}");
    app.join().unwrap();
}

#[test]
fn an_app_that_never_answers_ends_the_wait_after_a_few_seconds() {
    let (port, app) = app_answering(None, Duration::ZERO);
    let started = Instant::now();
    let error = Attached::port(port, "token").err().unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
    assert!(started.elapsed() < Duration::from_secs(9), "{:?}", started.elapsed());
    drop(app);
}
