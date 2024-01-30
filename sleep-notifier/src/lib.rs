//! Receive notifications when your screen is turned on and off by windows

use std::{ffi::CString, sync::mpsc, thread};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{BOOL, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader, Power, SystemServices},
        UI::WindowsAndMessaging,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Off,
    On,
    Dimmed,
}

/// Window procedure, called upon DispatchMessageA
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_POWERBROADCAST => {
            if wparam.0 as u32 == WindowsAndMessaging::PBT_POWERSETTINGCHANGE {
                let msgdata = &*(lparam.0 as *const Power::POWERBROADCAST_SETTING);

                let tx = WindowsAndMessaging::GetWindowLongPtrA(
                    hwnd,
                    WindowsAndMessaging::WINDOW_LONG_PTR_INDEX(0),
                ) as *const mpsc::Sender<Event>;

                match (msgdata.PowerSetting, msgdata.Data) {
                    (SystemServices::GUID_CONSOLE_DISPLAY_STATE, [0]) => Some(Event::Off),
                    (SystemServices::GUID_CONSOLE_DISPLAY_STATE, [1]) => Some(Event::On),
                    (SystemServices::GUID_CONSOLE_DISPLAY_STATE, [2]) => Some(Event::Dimmed),
                    (SystemServices::GUID_CONSOLE_DISPLAY_STATE, _) => unreachable!(),
                    _ => None,
                }
                .map(|event| (&*tx).send(event).expect("Receiver has been destroyed"));
            }
        }
        _ => return WindowsAndMessaging::DefWindowProcA(hwnd, msg, wparam, lparam),
    }
    LRESULT(0)
}

/// Start a thread that listens to display on/off events.
pub fn start() -> mpsc::Receiver<Event> {
    // Create a channel for the messages to be passed through
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let hinstance =
            unsafe { LibraryLoader::GetModuleHandleA(None) }.expect("Could not get hinstance");

        // Register a window class
        let classname = CString::new("DisplayStatusClass").unwrap();
        let mut windowclass = WindowsAndMessaging::WNDCLASSEXA::default();
        windowclass.cbSize = std::mem::size_of::<WindowsAndMessaging::WNDCLASSEXA>() as u32;
        windowclass.cbWndExtra = std::mem::size_of::<&mpsc::Sender<Event>>() as i32;
        windowclass.lpfnWndProc = Some(wndproc);
        windowclass.hInstance = hinstance.into();
        windowclass.lpszClassName = PCSTR(classname.as_ptr() as *const u8);
        unsafe {
            WindowsAndMessaging::RegisterClassExA(&windowclass);
        }

        // Create a window
        let windowname = CString::new("DisplayStatus").unwrap();
        let hwnd = unsafe {
            WindowsAndMessaging::CreateWindowExA(
                WindowsAndMessaging::WINDOW_EX_STYLE(0),
                PCSTR(classname.as_ptr() as *const u8),
                PCSTR(windowname.as_ptr() as *const u8),
                WindowsAndMessaging::WINDOW_STYLE(0),
                WindowsAndMessaging::CW_USEDEFAULT,
                WindowsAndMessaging::CW_USEDEFAULT,
                0,
                0,
                None,
                None,
                hinstance,
                None,
            )
        };
        if hwnd.0 == 0 {
            panic!("Cound not create a window")
        }

        // Set the address of tx as userdata
        let tx = Box::leak(Box::new(tx));
        unsafe {
            WindowsAndMessaging::SetWindowLongPtrA(
                hwnd,
                WindowsAndMessaging::WINDOW_LONG_PTR_INDEX(0),
                tx as *const mpsc::Sender<Event> as isize,
            );
        }

        // Hide the window
        unsafe {
            WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_HIDE);
        };

        // Register to the notifications related to the display
        unsafe {
            Power::RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &SystemServices::GUID_CONSOLE_DISPLAY_STATE,
                WindowsAndMessaging::DEVICE_NOTIFY_WINDOW_HANDLE.0,
            )
        }
        .expect("Could not register to power setting events");

        // Run the event loop
        loop {
            let mut message = WindowsAndMessaging::MSG::default();
            let BOOL(b) = unsafe { WindowsAndMessaging::GetMessageA(&mut message, None, 0, 0) };
            if b <= 0 {
                panic!("Event loop has been interrupted")
            }
            unsafe {
                WindowsAndMessaging::DispatchMessageA(&message);
            }
        }
    });

    rx
}
