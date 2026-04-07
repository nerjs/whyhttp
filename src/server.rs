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
    use std::{net::TcpStream, time::Duration};

    #[test]
    fn starts_server_and_stops_on_shutdown_signal() {
        let worker = SharedWorker::default();
        let (shutdown, addr) = run_server(worker);

        assert!(TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok());

        let _ = shutdown.send(());

        std::thread::sleep(Duration::from_millis(6));
        assert!(TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_err());
    }
}
