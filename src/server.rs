use std::{
    net::{SocketAddr, TcpListener},
    sync::mpsc::{Sender, TryRecvError, channel},
    thread,
    time::Duration,
};

use tiny_http::Server;

use crate::{io::Request, print::logln, worker::SharedWorker};

/// Starts an HTTP server backed by the shared worker.
///
/// Returns a shutdown sender and the bound socket address.
/// The server runs in a dedicated thread and polls both incoming requests
/// and the shutdown channel.
pub fn run_server(shared_worker: SharedWorker) -> (Sender<()>, SocketAddr) {
    let (shutdown_tx, shutdown_rx) = channel();
    let listener = TcpListener::bind(("0.0.0.0", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port();
    thread::spawn(move || {
        let server = Server::from_listener(listener, None).unwrap();
        logln!(0, "LOG", "Server is listening on port {port}");
        loop {
            match shutdown_rx.try_recv() {
                Ok(()) | Err(TryRecvError::Disconnected) => break,
                Err(TryRecvError::Empty) => {}
            }

            match server.try_recv() {
                Ok(Some(req)) => handle(shared_worker.clone(), req),
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(_) => break,
            }
        }
        logln!(0, "LOG", "Server on port {port} has been stopped");
    });

    (shutdown_tx, addr)
}

fn handle(shared_worker: SharedWorker, mut incoming: tiny_http::Request) {
    let request = Request::try_from(&mut incoming).unwrap_or_default();
    logln!(1, "LOG", "Received request {}", incoming.url());
    let mut worker = shared_worker.lock().unwrap();
    let response = worker.handle(request);

    if let Err(error) = incoming.respond(response.into()) {
        logln!(1, "ERROR", "Failed response sending: {error}");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        matchers::Matcher,
        reports::{Report, ReportReason},
        worker::Worker,
    };
    use reqwest::Client;
    use std::{
        net::TcpStream,
        sync::{Arc, Mutex},
    };

    #[test]
    fn starts_server_and_stops_on_shutdown_signal() {
        let worker = SharedWorker::default();
        let (shutdown, addr) = run_server(worker);

        assert!(TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok());

        let _ = shutdown.send(());

        std::thread::sleep(Duration::from_millis(6));
        assert!(TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_err());
    }

    #[tokio::test]
    async fn async_responds_with_default_response_when_no_expectations_match() {
        let worker = SharedWorker::default();
        let (_shutdown, addr) = run_server(worker);
        let url = format!("http://{addr}");

        let response = reqwest::get(url).await.unwrap();

        assert_eq!(response.status().as_u16(), 200);
    }

    #[test]
    fn blocking_responds_with_default_response_when_no_expectations_match() {
        let worker = SharedWorker::default();
        let (_shutdown, addr) = run_server(worker);
        let url = format!("http://{addr}");

        let response = reqwest::blocking::get(url).unwrap();

        assert_eq!(response.status().as_u16(), 200);
    }

    #[tokio::test]
    async fn responds_with_configured_status_from_matched_expectation() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some/path".to_string()));
        worker.set_response_status(&id, 205);

        let (_shutdown, addr) = run_server(Arc::new(Mutex::new(worker)));

        let url = format!("http://{addr}/some/path");
        let response = reqwest::get(url).await.unwrap();
        assert_eq!(response.status().as_u16(), 205);
    }

    #[tokio::test]
    async fn responds_with_configured_headers_from_matched_expectation() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some/path".to_string()));
        worker.set_response_header(&id, "some-key", "some-value");

        let (_shutdown, addr) = run_server(Arc::new(Mutex::new(worker)));

        let url = format!("http://{addr}/some/path");
        let response = reqwest::get(url).await.unwrap();

        let value = response
            .headers()
            .get("some-key")
            .map(|s| s.to_str().unwrap());
        assert_eq!(value, Some("some-value"));
    }

    #[tokio::test]
    async fn responds_with_configured_body_from_matched_expectation() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some/path".to_string()));
        worker.set_response_body(&id, "qwerty body");

        let (_shutdown, addr) = run_server(Arc::new(Mutex::new(worker)));

        let url = format!("http://{addr}/some/path");
        let response = reqwest::get(url).await.unwrap().text().await.unwrap();

        assert_eq!(response, "qwerty body");
    }

    #[tokio::test]
    async fn passes_request_method_to_worker() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some/path".to_string()));
        worker.add_validating(&id, Matcher::Method("POST".to_string()));
        let shared_worker = Arc::new(Mutex::new(worker));
        let (_shutdown, addr) = run_server(shared_worker.clone());

        let url = format!("http://{addr}/some/path");
        let _ = Client::builder()
            .build()
            .unwrap()
            .post(url)
            .send()
            .await
            .unwrap();

        let worker = shared_worker.lock().unwrap();
        assert_eq!(worker.reports(), None);
    }

    #[tokio::test]
    async fn passes_request_headers_to_worker() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some/path".to_string()));
        worker.add_validating(
            &id,
            Matcher::HeaderEq("x-key".to_string(), "x-value".to_string()),
        );
        let shared_worker = Arc::new(Mutex::new(worker));
        let (_shutdown, addr) = run_server(shared_worker.clone());

        let url = format!("http://{addr}/some/path");
        let _ = Client::builder()
            .build()
            .unwrap()
            .post(url)
            .header("x-key", "x-value")
            .send()
            .await
            .unwrap();

        let worker = shared_worker.lock().unwrap();
        assert_eq!(worker.reports(), None);
    }

    #[tokio::test]
    async fn passes_request_body_to_worker() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some/path".to_string()));
        worker.add_validating(&id, Matcher::BodyEq("some body".to_string()));
        let shared_worker = Arc::new(Mutex::new(worker));
        let (_shutdown, addr) = run_server(shared_worker.clone());

        let url = format!("http://{addr}/some/path");
        let _ = Client::builder()
            .build()
            .unwrap()
            .post(url)
            .body("some body")
            .send()
            .await
            .unwrap();

        let worker = shared_worker.lock().unwrap();
        assert_eq!(worker.reports(), None);
    }

    #[tokio::test]
    async fn records_unmatched_runtime_request_in_reports() {
        let worker = Worker::default();
        let shared_worker = Arc::new(Mutex::new(worker));
        let (_shutdown, addr) = run_server(shared_worker.clone());

        let url = format!("http://{addr}/some/path");
        let _ = reqwest::get(url).await.unwrap();

        let worker = shared_worker.lock().unwrap();
        let reports = worker.reports();

        assert!(matches!(
            reports.as_deref(),
            Some([Report {
                request,
                reasons
            }])
            if request.path == "/some/path"
                && matches!(reasons.as_slice(), [ReportReason::UnmatchedRequest])
        ));
    }
}
