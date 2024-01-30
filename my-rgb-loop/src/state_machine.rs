use orgb::{Connection, ControllerData, ControllerType, Request, Rgb};
use palette::{IntoColor, LinSrgb, Oklab, Srgb};
use sleep_notifier::{self, Event};
use std::f32::consts::TAU;
use std::sync::mpsc;

enum State {
    Normal { ticks: u32 },
    Wake { ticks: u32, ticks_max: u32 },
    Sleep,
}

pub struct StateMachine {
    // Display status update receiver
    display_event_rx: mpsc::Receiver<Event>,
    // Index of the dram light controller
    dram_idx: Option<u32>,
    // Current state
    state: State,
}

impl StateMachine {
    pub fn new() -> StateMachine {
        StateMachine {
            display_event_rx: sleep_notifier::start(),
            dram_idx: None,
            state: State::Normal { ticks: 0 },
        }
    }

    /// Signal to the state machine that the controller have been updated
    pub fn controllers_updated(&mut self, controllers: &[ControllerData]) {
        // Find the index of the dram light controller
        self.dram_idx = controllers
            .iter()
            .position(|c| c.ty == ControllerType::Dram)
            .map(|p| p as u32);
    }

    /// Step the state machine
    pub fn update(&mut self, serv: &mut Connection) {
        // Get the current events
        let event = match self.display_event_rx.try_recv() {
            Ok(e) => {
                log::info!("Display status updated to {e:?}");
                Some(e)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => panic!("Sender has been disconnected"),
        };

        // Update the current state
        match &mut self.state {
            State::Normal { ticks } => {
                *ticks += 1;
                if let Some(Event::Off | Event::Dimmed) = event {
                    self.state = State::Sleep // Transition to sleep
                }
            }
            State::Sleep => {
                if let Some(Event::On) = event {
                    self.state = State::Wake {
                        ticks: 0,
                        ticks_max: 5,
                    } // Transition to wake
                }
            }
            State::Wake { ticks, ticks_max } => {
                *ticks += 1;
                if ticks == ticks_max {
                    self.state = State::Normal { ticks: 0 } // Transition to normal
                }
            }
        }

        // Update the lights of the dram
        if let Some(controller_idx) = self.dram_idx {
            let dram_colors = match self.state {
                State::Sleep => dram_color_asleep(),
                State::Normal { ticks: counter } => dram_color_normal(counter),
                State::Wake { ticks, ticks_max } => dram_color_wake(ticks, ticks_max),
            };
            serv.send(Request::UpdateLeds {
                controller_idx,
                colors: &dram_colors.map(|oklab| {
                    let srgb: Srgb = oklab.into_color();
                    let srgb: LinSrgb<u8> = srgb.into_linear().into_format();
                    Rgb(srgb.red, srgb.green, srgb.blue)
                }),
            });
        }
    }
}

// Color picker: https://observablehq.com/@shan/oklab-color-wheel

fn dram_color_normal(ticks: u32) -> [Oklab; 5] {
    let time_phase = (ticks % 150) as f32 / 150.0 * TAU;
    let color_1 = Oklab::new(0.900, -0.304, 0.151);
    let color_2 = Oklab::new(0.900, 0.094, 0.327);
    let mut result = [Oklab::default(); 5];
    for (i, c) in result.iter_mut().enumerate() {
        let space_phase = i as f32 / 5.0 * TAU;
        let t = (time_phase + space_phase).sin() * 0.5 + 0.5;
        *c = color_1 * t + color_2 * (1.0 - t);
    }
    result
}

fn dram_color_asleep() -> [Oklab; 5] {
    let orange = Oklab::new(0.5, 0.24, 0.29);
    [orange; 5]
}

fn dram_color_wake(ticks: u32, ticks_max: u32) -> [Oklab; 5] {
    let orange = Oklab::new(0.5, 0.24, 0.29);
    let mut result = dram_color_normal(0);
    let t = ticks as f32 / ticks_max as f32;
    for c in result.iter_mut() {
        *c = *c * t + orange * (1.0 - t);
    }
    result
}
