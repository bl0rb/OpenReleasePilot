//! The HTTP client (ureq) as both apps use it: one place for the agent settings.
//!
//! Text answers are read with `Body::read_to_string`: at most 10 MB, invalid UTF-8 replaced, as
//! ureq 2's `into_string` did; downloads and JSON go through `into_reader`, without that limit.

use std::time::Duration;

/// An agent whose `timeout` covers the whole request, answer body included. With `status_as_error`
/// an HTTP status from 400 on is an `Error::StatusCode`; without, it is a response like any other,
/// whose body the caller can still read. Otherwise kept as ureq 2 behaved: at most 30 s for the
/// connection, 5 redirects (without the Authorization header to another host), no proxy from the
/// environment.
pub fn agent(timeout: Duration, status_as_error: bool) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(30)))
        .max_redirects(5)
        .proxy(None)
        .http_status_as_error(status_as_error)
        .build()
        .into()
}

/// A URL from the user's config as ureq 2 sent it: surrounding whitespace trimmed, an
/// internationalized host in punycode, spaces and non-ASCII in path and query percent-encoded.
/// ureq 3 parses with `http::Uri`, which rejects all of that. Unparsable URLs are handed back
/// trimmed, so ureq reports its own error.
pub fn url(raw: &str) -> String {
    let raw = raw.trim();
    url::Url::parse(raw).map(String::from).unwrap_or_else(|_| raw.to_owned())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// A server for one request: answers `status` with `body` and hands back what it received.
    fn serve(status: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let (url, server) = serve_all(vec![(status, "", body)]);
        (url, std::thread::spawn(move || server.join().unwrap().remove(0)))
    }

    /// A server for one request per answer (`status`, extra header lines, body), one connection
    /// each; hands back the requests it received.
    fn serve_all(answers: Vec<(&'static str, &'static str, &'static str)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, headers, body) in answers {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream);
                let mut request = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    request.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut content = vec![0; length];
                reader.read_exact(&mut content).unwrap();
                request.push_str(&String::from_utf8(content).unwrap());
                let answer = format!("HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                reader.into_inner().write_all(answer.as_bytes()).unwrap();
                requests.push(request);
            }
            requests
        });
        (url, server)
    }

    #[test]
    fn error_statuses_keep_their_body() {
        let (url, server) = serve("403 Forbidden", r#"{"message":"SSO required"}"#);
        let request = ureq::http::Request::builder().method("GET").uri(format!("{url}/x")).header("User-Agent", "OpenReleasePilot");
        let response = crate::github::run(request, None).unwrap();
        assert_eq!(response.status().as_u16(), 403);
        assert_eq!(response.into_body().read_to_string().unwrap(), r#"{"message":"SSO required"}"#);
        let request = server.join().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /x http/1.1\r\n"));
        assert!(request.contains("user-agent: openreleasepilot\r\n"));
        assert!(!request.contains("content-length") && !request.contains("transfer-encoding"), "no body, no body headers: {request}");
    }

    #[test]
    fn writes_send_their_body_with_its_length() {
        let (url, server) = serve("201 Created", "{}");
        let request = ureq::http::Request::builder().method("POST").uri(format!("{url}/y")).header("Content-Type", "application/json");
        let response = crate::github::run(request, Some(r#"{"a":1}"#.into())).unwrap();
        assert_eq!(response.status().as_u16(), 201);
        let request = server.join().unwrap();
        assert!(request.to_ascii_lowercase().contains("content-length: 7\r\n"));
        assert!(request.ends_with(r#"{"a":1}"#));
    }

    #[test]
    fn bodiless_delete_sends_no_body_headers() {
        let (url, server) = serve("204 No Content", "");
        let request = ureq::http::Request::builder().method("DELETE").uri(format!("{url}/z"));
        assert_eq!(crate::github::run(request, None).unwrap().status().as_u16(), 204);
        let request = server.join().unwrap().to_ascii_lowercase();
        assert!(!request.contains("content-length") && !request.contains("transfer-encoding"), "{request}");
    }

    #[test]
    fn status_as_error_reports_the_code() {
        let (url, server) = serve("404 Not Found", "gone");
        match super::agent(std::time::Duration::from_secs(5), true).get(&url).call() {
            Err(ureq::Error::StatusCode(404)) => {}
            other => panic!("expected StatusCode(404), got {other:?}"),
        }
        server.join().unwrap();
    }

    #[test]
    fn writes_hand_back_a_redirect() {
        // GitHub answers writes to a renamed repository with 307; ureq 2 returned it as the answer.
        let (url, server) = serve_all(vec![
            ("307 Temporary Redirect", "Location: /moved\r\n", r#"{"message":"Temporary Redirect"}"#),
            ("308 Permanent Redirect", "Location: https://example.invalid/\r\n", ""),
        ]);
        let request = ureq::http::Request::builder().method("PUT").uri(format!("{url}/merge")).header("Content-Type", "application/json");
        let response = crate::github::run(request, Some("{}".into())).unwrap();
        assert_eq!(response.status().as_u16(), 307);
        assert_eq!(response.into_body().read_to_string().unwrap(), r#"{"message":"Temporary Redirect"}"#);
        let request = ureq::http::Request::builder().method("DELETE").uri(format!("{url}/branch"));
        assert_eq!(crate::github::run(request, None).unwrap().status().as_u16(), 308);
        assert_eq!(server.join().unwrap().len(), 2, "no redirect followed");
    }

    #[test]
    fn redirect_without_location_is_the_answer() {
        let (url, server) = serve_all(vec![("302 Found", "", "first"), ("302 Found", "", "again")]);
        let request = ureq::http::Request::builder().method("GET").uri(format!("{url}/x"));
        let response = crate::github::run(request, None).unwrap();
        assert_eq!(response.status().as_u16(), 302);
        assert_eq!(response.into_body().read_to_string().unwrap(), "again");
        server.join().unwrap();
    }

    #[test]
    fn config_urls_are_normalized_as_ureq_2_did() {
        assert_eq!(super::url("https://bäckerei-müller.de"), "https://xn--bckerei-mller-bfb28a.de/");
        assert_eq!(super::url(" https://example.com/über uns "), "https://example.com/%C3%BCber%20uns");
        assert_eq!(super::url("https://example.com/a/"), "https://example.com/a/");
        assert_eq!(super::url(" kein url "), "kein url");
        for raw in ["https://bäckerei-müller.de", "https://example.com/a b ", "https://example.com "] {
            assert!(super::url(raw).parse::<ureq::http::Uri>().is_ok(), "{raw}");
        }
    }
}
