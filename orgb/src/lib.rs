//! 💡 Talk with an OpenRGB server 💡
//!
//! [Network protocol documentation](https://gitlab.com/OpenRGBDevelopers/OpenRGB-Wiki/-/blob/stable/Developer-Documentation/OpenRGB-SDK-Documentation.md)

mod connection;
mod protocol;

pub use connection::Connection;
pub use protocol::*;
