use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
};

fn exchange(scheme: &str, timeout: Duration, serve: impl FnOnce(TcpStream) + Send + 'static) -> ProbeResult {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(5);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < until => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("loopback fixture could not accept a connection: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        serve(stream);
    });
    let client = client_builder().build().unwrap();
    let result = inspect(client.get(format!("{scheme}://{address}/v1/models")).timeout(timeout));
    server.join().unwrap();
    result
}

fn read_http_request(stream: &mut TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
            break;
        }
    }
}

fn http_response(status: u16, header: &'static str, body: &'static str) -> ProbeResult {
    exchange("http", Duration::from_secs(3), move |mut socket| {
        read_http_request(&mut socket);
        write!(socket, "HTTP/1.1 {status} Fixture\r\n{header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()).unwrap();
    })
}

#[test]
fn distinguishes_http_outcomes_without_returning_headers_or_error_bodies() {
    for (status, outcome) in [
        (301, ProbeOutcome::Redirect),
        (401, ProbeOutcome::Rejected),
        (403, ProbeOutcome::Forbidden),
        (404, ProbeOutcome::Unsupported),
        (405, ProbeOutcome::Unsupported),
        (422, ProbeOutcome::HttpError),
        (429, ProbeOutcome::RateLimited),
        (503, ProbeOutcome::ServerError),
        (529, ProbeOutcome::ServerError),
    ] {
        let result = http_response(status, "Retry-After: 17\r\nLocation: https://offsite.invalid/private\r\n",
            r#"{"error":"sensitive test fixture"}"#);
        assert_eq!(result.stage, ProbeStage::Http);
        assert_eq!(result.outcome, outcome);
        assert_eq!(result.status, Some(status));
        assert_eq!(result.models, None);
        assert_eq!(result.retry_after_seconds, matches!(status, 429 | 503).then_some(17));
        let report = serde_json::to_string(&result).unwrap();
        assert!(!report.contains("sensitive") && !report.contains("offsite.invalid"));
    }
}

#[test]
fn distinguishes_html_json_schema_and_valid_lists() {
    for (body, outcome, models) in [
        ("<html>Gateway login</html>", ProbeOutcome::InvalidJson, None),
        (r#"{"result":[{"id":"model"}]}"#, ProbeOutcome::UnexpectedResponse, None),
        (r#"{"data":[{"id":" "}]}"#, ProbeOutcome::UnexpectedResponse, None),
        (r#"{"data":[{"id":42}]}"#, ProbeOutcome::UnexpectedResponse, None),
        (r#"{"data":[{"id":"fixture-model"}],"has_more":true}"#, ProbeOutcome::Connected, Some(1)),
        (r#"{"data":[],"object":"list"}"#, ProbeOutcome::Connected, Some(0)),
    ] {
        let result = http_response(200, "", body);
        assert_eq!(result.stage, ProbeStage::Models);
        assert_eq!(result.outcome, outcome);
        assert_eq!(result.models, models);
        assert!(!serde_json::to_string(&result).unwrap().contains("fixture-model"));
    }
}

#[test]
fn classifies_a_dns_failure_through_the_http_client() {
    struct Unresolved;
    impl Resolve for Unresolved {
        fn resolve(&self, _: Name) -> Resolving {
            Box::pin(async { Err(Box::new(DnsFailure) as Box<dyn StdError + Send + Sync>) })
        }
    }
    let client = client_builder().dns_resolver(Arc::new(Unresolved)).build().unwrap();
    let result = inspect(client.get("http://dns-fixture.invalid/v1/models"));
    assert_eq!(result.outcome, ProbeOutcome::DnsError);
    assert_eq!(result.stage, ProbeStage::Dns);
    assert!(result.status.is_none());
}

#[test]
fn classifies_a_tls_handshake_error_at_a_plain_http_listener() {
    let result = exchange("https", Duration::from_secs(3), |mut socket| {
        let mut header = [0; 5];
        socket.read_exact(&mut header).unwrap();
        assert_eq!(header[0], 22, "the request should start with a TLS handshake record");
        let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
        let mut client_hello = vec![0; length];
        socket.read_exact(&mut client_hello).unwrap();
        socket.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    assert_eq!(result.outcome, ProbeOutcome::TlsError);
    assert_eq!(result.stage, ProbeStage::Tls);
    assert!(result.status.is_none());

    let cert = io::Error::new(io::ErrorKind::InvalidData, rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer));
    assert!(causes(&cert).any(|cause| cause.is::<rustls::Error>()));
}

#[test]
fn reports_a_timeout_before_the_server_sends_headers() {
    let result = exchange("http", Duration::from_millis(350), |mut socket| {
        read_http_request(&mut socket);
        thread::sleep(Duration::from_millis(600));
    });
    assert_eq!(result.outcome, ProbeOutcome::Timeout);
    assert_eq!(result.stage, ProbeStage::Request);
    assert!(result.status.is_none());
}

#[test]
fn enforces_the_request_deadline_while_a_body_is_still_arriving() {
    let result = exchange("http", Duration::from_millis(1100), |mut socket| {
        read_http_request(&mut socket);
        let json = br#"{"data":[{"id":"slow-fixture-model"}]}"#;
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", json.len()).unwrap();
        for chunk in json.chunks(4) {
            if socket.write_all(chunk).is_err() { break; }
            thread::sleep(Duration::from_millis(185));
        }
    });
    assert_eq!(result.outcome, ProbeOutcome::Timeout);
    assert_eq!(result.stage, ProbeStage::Body);
    assert_eq!(result.status, Some(200));
}

#[test]
fn detects_interrupted_and_oversize_bodies() {
    for (length, outcome) in [(100, ProbeOutcome::BodyError), (MAX_RESPONSE_BYTES + 1, ProbeOutcome::ResponseTooLarge)] {
        let result = exchange("http", Duration::from_secs(3), move |mut socket| {
            read_http_request(&mut socket);
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\nshort").unwrap();
        });
        assert_eq!(result.stage, ProbeStage::Body);
        assert_eq!(result.outcome, outcome);
    }
    let chunked = exchange("http", Duration::from_secs(3), |mut socket| {
        read_http_request(&mut socket);
        write!(socket, "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n", MAX_RESPONSE_BYTES + 1).unwrap();
        let _ = socket.write_all(&vec![b' '; (MAX_RESPONSE_BYTES + 1) as usize]);
        let _ = socket.write_all(b"\r\n0\r\n\r\n");
    });
    assert_eq!(chunked.outcome, ProbeOutcome::ResponseTooLarge);
}

#[test]
fn parses_retry_after_as_seconds_or_http_date() {
    let now = httpdate::parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
    assert_eq!(retry_seconds("42", now), Some(42));
    assert_eq!(retry_seconds("Sun, 06 Nov 1994 08:50:00 GMT", now), Some(23));
    assert_eq!(retry_seconds("Sun, 06 Nov 1994 08:49:30 GMT", now), Some(0));
    assert_eq!(retry_seconds("4294967296", now), None);
    assert_eq!(retry_seconds("token=fixture", now), None);
    assert_eq!(retry_seconds("", now), None);
}
