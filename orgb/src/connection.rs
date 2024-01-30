use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::protocol::{Request, Response};

/// A wrapper around a TCP connection to an OpenRGB server.
pub struct Connection {
    con: TcpStream,
    rx: Receiver<Response>,
    devices_updated: Arc<AtomicBool>,
}

const NUM_CONNECTION_TRIES: i32 = 10;

impl Connection {
    /// Connect to an OpenRGB server and starts a thread that listens to incomming messages.
    ///
    /// If the server is not immediately available, it will attempt to connect 10 times before panicking.
    pub fn start<A: ToSocketAddrs>(addr: A) -> Connection {
        // Connect to the server
        log::info!("Connecting to OpenRGB server...");
        let mut num_tries = 0;
        let con = loop {
            match TcpStream::connect(&addr) {
                Ok(con) => break con,
                Err(_) => {
                    num_tries += 1;
                    if num_tries >= NUM_CONNECTION_TRIES {
                        panic!("Could not connect, aborting");
                    } else {
                        log::info!("Could not connect, retrying...");
                    }
                    thread::sleep(Duration::from_secs(1));
                }
            }
        };

        // A channel to receive responses and a flag to indicate device updates
        let (tx, rx) = mpsc::sync_channel(0);
        let devices_updated = Arc::new(AtomicBool::new(true));

        // Launch the thread that receives messages from the OpenRGB server
        let _recv_thread = {
            let devices_updated = Arc::clone(&devices_updated);
            let mut con = con.try_clone().expect("Could not clone the TcpStream");
            thread::spawn(move || loop {
                match Response::read_from(&mut con).expect("Could not read from the TcpStream") {
                    Response::DeviceListUpdated => {
                        log::info!("Device list has been updated");
                        devices_updated.store(true, Ordering::Relaxed)
                    }
                    other => tx.send(other).expect("Receiver has been destroyed"),
                }
            })
        };

        Connection {
            con,
            rx,
            devices_updated,
        }
    }

    /// Send a request to the OpenRGB server.
    pub fn send(&mut self, request: Request) {
        request
            .write_to(&mut self.con)
            .expect("Could not write to the TcpStream");
    }

    /// Wait for a response from the OpenRGB server.
    pub fn recv(&self) -> Response {
        self.rx.recv().expect("Sender has been destroyed")
    }

    /// Returns the flag that indicates when the list of devices has been updated, then resets the flag.
    ///
    /// If the flag is raised, it means that the controllers must be requested again.
    pub fn devices_updated_reset(&self) -> bool {
        self.devices_updated.swap(false, Ordering::Relaxed)
    }
}
