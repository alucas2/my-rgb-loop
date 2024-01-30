//! ## Setup
//!
//! - Schedule the OpenRGB server to run at startup with the `--server` argument.
//! - Schedule this program to run at startup.
//!
//! To schedule a program to run at startup, create a shortcut to it in the Startup directory, which can
//! be opened by typing `shell:startup` in the Run utility (Windows+R).
//!
//! ## Customize the lighting scheme
//!
//! Edit the file `state_machine.rs` to create a lighting scheme.
//!
//! ## Troubleshooting
//!
//! This programs outputs to *log.txt*.
//!
//! If no lighting devices are detected, try re-running OpenRGB in admin mode.

// Hide the console window
#![windows_subsystem = "windows"]

mod state_machine;
use crate::state_machine::StateMachine;

use orgb::{Connection, Request, Response};
use std::thread;
use std::time::Duration;

fn main() {
    let _ = simplelog::WriteLogger::init(
        log::LevelFilter::Info,
        simplelog::Config::default(),
        std::fs::File::create("log.txt").expect("Could not create log.txt"),
    );

    log_panics::init();

    let mut serv = Connection::start("127.0.0.1:6742");
    serv.send(Request::SetClientName("My RGB loop yay"));

    // Resuest a protocol version
    log::info!("Requesting protocol version 0...");
    serv.send(Request::ProtocolVersion(0));
    match serv.recv() {
        Response::ProtocolVersion(v) => log::info!("Received protocol version: {v}"),
        other => panic!("Unexpected response: {other:?}"),
    }

    let mut state_machine = StateMachine::new();

    loop {
        // Controllers have been updated, they need to be requested again
        if serv.devices_updated_reset() {
            // Request the number of controllers
            serv.send(Request::ControllerCount);
            let controller_count = match serv.recv() {
                Response::ControllerCount(c) => c,
                other => panic!("Unexpected response: {other:?}"),
            };

            // Collect all the controllers data
            let mut new_controllers = Vec::new();
            for controller_idx in 0..controller_count {
                serv.send(Request::ControllerData { controller_idx });
                match serv.recv() {
                    Response::ControllerData(c) => new_controllers.push(c),
                    other => panic!("Unexpected response: {other:?}"),
                }
            }
            log::info!("Available controllers: {new_controllers:#?}");
            state_machine.controllers_updated(&new_controllers);
        }

        // Step the state machine and update the colors
        state_machine.update(&mut serv);

        // Wait a bit
        thread::sleep(Duration::from_millis(100))
    }
}
