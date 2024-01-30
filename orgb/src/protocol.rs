#![allow(non_upper_case_globals)] // Make rust-analyzer stfu

use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, IntoPrimitive, TryFromPrimitive, PartialEq, Eq)]
#[repr(u32)]
pub enum ControllerType {
    Motherboard = 0,
    Dram = 1,
    Gpu = 2,
    Cooler = 3,
    LedStrip = 4,
    Keyboard = 5,
    Mouse = 6,
    Mousemat = 7,
    Headset = 8,
    HeadsetStand = 9,
}

#[derive(Debug, Clone, Copy, IntoPrimitive, TryFromPrimitive, PartialEq, Eq)]
#[repr(u32)]
pub enum ZoneType {
    Single = 0,
    Linear = 1,
    Matrix = 2,
}

#[derive(Debug, Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The LED structure contains information about an LED.
///
/// The Value has no defined functionality in the RGBController API and is provided for implementation-specific use.
/// You can use this field to associate implementation-specific data with an LED.
#[derive(Debug)]
pub struct Led {
    pub name: String,
    pub value: u32,
}

/// The matrix map is used to provide positioning information about LEDs in a 2D grid.
/// The values of the map are LED index values in the zone (so offset by Start Index from the RGBController's LEDs
/// vector). If a spot in the matrix is unused and does not map to an LED, it should be set to 0xFFFFFFFF.
#[derive(Debug)]
pub struct ZoneMatrix {
    pub height: u32,
    pub width: u32,
    pub data: Vec<u32>,
}

/// The Zone structure contains information about a zone. A zone is a logical grouping of LEDs defined by the
/// RGBController implementation. LEDs in a zone must be contiguous in the RGBController's LEDs/Colors vectors.
#[derive(Debug)]
pub struct Zone {
    pub name: String,
    pub ty: ZoneType,
    pub leds_min: u32,
    pub leds_max: u32,
    pub leds_count: u32,
    pub matrix: Option<ZoneMatrix>,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ModeFlags: u32 {
        /// Mode has speed parameter
        const SPEED = 1 << 0;
        /// Mode has left/right direction parameter
        const LEFT_RIGHT = 1 << 1;
        /// Mode has up/down direction parameter
        const UP_DOWN = 1 << 2;
        /// Mode has horizontal/vertical direction parameter
        const HORIZONTAL_VERTICAL = 1 << 3;
        /// Mode has brightness parameter
        const BRIGHTNESS = 1 << 4;
        /// Mode has per-LED color settings
        const PER_LED_SETTINGS = 1 << 5;
        /// Mode has mode specific color settings
        const SPECIFIC_SETTINGS = 1 << 6;
        /// Mode has random color option
        const RANDOM_COLOR = 1 << 7;
    }
}

#[derive(Debug, Clone, Copy, IntoPrimitive, TryFromPrimitive, PartialEq, Eq)]
#[repr(u32)]
pub enum ColorMode {
    /// None - this mode does not have configurable colors
    None = 0,
    /// Per-LED - this mode uses the RGBController's colors vector to set each LED to its specified color
    PerLed = 1,
    /// Mode Specific - this mode has one or more configurable colors, but not individual LED control
    ModeSpecific = 2,
    /// Random - this mode can be switched to a random or cycling color palette
    Random = 3,
}

/// Modes represent internal effects and have a name field that describes the effect.
///
/// The mode value is field is provided to hold an implementation-defined mode value. This is usually the mode's value
/// in the hardware protocol.
///
/// The mode flags field is a bitfield that contains information about what features a mode has.
///
/// The mode minimum and maximum speed fields should be set to the implementation-specific minimum and maximum speed
/// values for the given mode if the mode supports speed control. The mode speed value field will be set between the
/// minimum and maximum value, inclusively. The minimum speed may be a greater numerical value than the maximum speed
/// if your device's speed adjustment is inverted (usually because the device takes a delay period rather than a speed
/// value)
///
/// The mode minimum and maximum number of colors fields should be used if the mode supports mode-specific color
/// settings. These determine the size range of the mode's Colors vector. If the mode has a fixed number of colors, the
/// minimum and maximum should be equal.  Mode-specific colors are used when a mode has one or more configurable colors
/// but these colors do not apply directly to individual LEDs. Example would be a breathing mode that cycles between
/// one or more colors each breath pulse. A mode may have multiple color options available, for instance a breathing
/// mode that can either use one or more defined colors or just cycle through random colors. The available color modes
/// for a given mode are set with the flags.
#[derive(Debug)]
pub struct Mode {
    pub name: String,
    pub value: u32,
    pub flags: ModeFlags,
    pub speed_min: u32,
    pub speed_max: u32,
    pub colors_min: u32,
    pub colors_max: u32,
    pub speed: u32,
    pub direction: u32,
    pub color_mode: ColorMode,
    pub colors: Vec<Rgb>,
}

#[derive(Debug)]
pub struct ControllerData {
    pub ty: ControllerType,
    pub name: String,
    pub description: String,
    pub version: String,
    pub serial: String,
    pub location: String,
    pub modes: Vec<Mode>,
    pub active_mode: u32,
    pub zones: Vec<Zone>,
    pub leds: Vec<Led>,
    pub colors: Vec<Rgb>,
}

#[derive(Debug, Clone)]
struct PacketHeader {
    _dev_idx: u32,
    pkt_id: u32,
    pkt_size: u32,
}

#[derive(Debug)]
pub enum Response {
    ControllerCount(u32),
    ControllerData(ControllerData),
    ProtocolVersion(u32),
    DeviceListUpdated,
}

impl Response {
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Response, std::io::Error> {
        // Parse header
        let mut header_bytes = [0u8; 16];
        reader.read_exact(&mut header_bytes)?;
        let (_, header) =
            parse::packet_header(&header_bytes).expect("Could not parse packet header");

        // Parse data
        let mut data_bytes = vec![0u8; header.pkt_size as usize];
        reader.read_exact(&mut data_bytes)?;
        let (rest, response) =
            parse::response(header, &data_bytes).expect("Could not parse packet data");

        // Check that there is no unparsed data
        assert_eq!(rest.len(), 0);

        Ok(response)
    }
}

#[derive(Debug, Clone)]
pub enum Request<'a> {
    ControllerCount,
    ControllerData {
        controller_idx: u32,
    },
    ProtocolVersion(u32),
    SetClientName(&'a str),
    ResizeZone {
        controller_idx: u32,
        zone_idx: u32,
        new_size: u32,
    },
    UpdateLeds {
        controller_idx: u32,
        colors: &'a [Rgb],
    },
    UpdateZoneLeds {
        controller_idx: u32,
        zone_idx: u32,
        colors: &'a [Rgb],
    },
    UpdateSingleLed {
        controller_idx: u32,
        led_idx: u32,
        color: Rgb,
    },
    SetCustomMode {
        controller_idx: u32,
    },
    UpdateMode {
        controller_idx: u32,
        mode_idx: u32,
        mode: &'a Mode,
    },
    SaveMode {
        controller_idx: u32,
        mode_idx: u32,
        mode: &'a Mode,
    },
}

impl Request<'_> {
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        let mut output = Vec::new();
        let output = &mut output;
        output.extend_from_slice(b"ORGB");

        match *self {
            Request::ControllerCount => {
                unparse::u32(0, output); // dev_idx
                unparse::u32(0, output); // pkt_id
                unparse::u32(0, output); // pkt_size
            }
            Request::ProtocolVersion(v) => {
                unparse::u32(0, output); // dev_idx
                unparse::u32(40, output); // pkt_id
                unparse::u32(4, output); // pkt_size
                unparse::u32(v, output);
            }
            Request::ControllerData { controller_idx } => {
                unparse::u32(controller_idx, output); // dev_idx
                unparse::u32(1, output); // pkt_id
                unparse::u32(0, output); // pkt_size
            }
            Request::SetClientName(name) => {
                let len = name.as_bytes().len() + 1;
                unparse::u32(0, output); // dev_idx
                unparse::u32(50, output); // pkt_id
                unparse::u32(len as u32, output); // pkt_size
                output.extend(name.as_bytes());
                output.extend(b"\0");
            }
            Request::UpdateLeds {
                controller_idx,
                colors,
            } => {
                let len = 4 + 2 + 4 * colors.len();
                unparse::u32(controller_idx, output); // dev_idx
                unparse::u32(1050, output); // pkt_id
                unparse::u32(len as u32, output); // pkt_size
                unparse::u32(len as u32, output);
                unparse::u16(colors.len() as u16, output);
                for c in colors {
                    unparse::color(*c, output);
                }
            }
            Request::ResizeZone { .. } => todo!(),
            Request::SaveMode { .. } => todo!(),
            Request::SetCustomMode { .. } => todo!(),
            Request::UpdateMode { .. } => todo!(),
            Request::UpdateZoneLeds { .. } => todo!(),
            Request::UpdateSingleLed { .. } => todo!(),
        }

        writer.write(&output).map(|_| ())
    }
}

mod parse {
    use super::*;

    use nom::{
        bytes::complete::{tag, take},
        combinator::map,
        multi::count,
        number::{complete, Endianness},
        IResult,
    };

    fn u16(input: &[u8]) -> IResult<&[u8], u16> {
        complete::u16(Endianness::Native)(input)
    }

    fn u32(input: &[u8]) -> IResult<&[u8], u32> {
        complete::u32(Endianness::Native)(input)
    }

    fn controller_type(input: &[u8]) -> IResult<&[u8], ControllerType> {
        let (input, x) = u32(input)?;
        let x = ControllerType::try_from(x).expect("Unknown controller type");
        Ok((input, x))
    }

    fn zone_type(input: &[u8]) -> IResult<&[u8], ZoneType> {
        let (input, x) = u32(input)?;
        let x = ZoneType::try_from(x).expect("Unknown zone type");
        Ok((input, x))
    }

    fn color_mode(input: &[u8]) -> IResult<&[u8], ColorMode> {
        let (input, x) = u32(input)?;
        let x = ColorMode::try_from(x).expect("Unknown color mode");
        Ok((input, x))
    }

    fn null_terminated_string(len_with_terminator: u16, input: &[u8]) -> IResult<&[u8], &str> {
        let (input, string) = take(len_with_terminator - 1)(input)?;
        let string = std::str::from_utf8(string).expect("Received a string that is not utf-8");
        let (input, _) = tag(b"\0")(input)?;
        Ok((input, string))
    }

    fn color(input: &[u8]) -> IResult<&[u8], Rgb> {
        let (input, color_int) = u32(input)?;
        let color_bytes = color_int.to_ne_bytes();
        Ok((input, Rgb(color_bytes[0], color_bytes[1], color_bytes[2])))
    }

    fn led(input: &[u8]) -> IResult<&[u8], Led> {
        let (input, name_len) = u16(input)?;
        let (input, name) = null_terminated_string(name_len, input)?;
        let (input, value) = u32(input)?;
        Ok((
            input,
            Led {
                name: name.into(),
                value,
            },
        ))
    }

    fn zone_matrix(input: &[u8]) -> IResult<&[u8], ZoneMatrix> {
        let (input, height) = u32(input)?;
        let (input, width) = u32(input)?;
        let (input, data) = count(u32, (height * width) as usize)(input)?;
        Ok((
            input,
            ZoneMatrix {
                height,
                width,
                data,
            },
        ))
    }

    fn zone(input: &[u8]) -> IResult<&[u8], Zone> {
        let (input, name_len) = u16(input)?;
        let (input, name) = null_terminated_string(name_len, input)?;
        let (input, ty) = zone_type(input)?;
        let (input, leds_min) = u32(input)?;
        let (input, leds_max) = u32(input)?;
        let (input, leds_count) = u32(input)?;
        let (input, matrix_len) = u16(input)?;
        let (input, matrix) = if matrix_len == 0 {
            (input, None)
        } else {
            let (input, matrix) = zone_matrix(input)?;
            (input, Some(matrix))
        };
        Ok((
            input,
            Zone {
                name: name.into(),
                ty,
                leds_min,
                leds_max,
                leds_count,
                matrix,
            },
        ))
    }

    fn mode(input: &[u8]) -> IResult<&[u8], Mode> {
        let (input, name_len) = u16(input)?;
        let (input, name) = null_terminated_string(name_len, input)?;
        let (input, value) = u32(input)?;
        let (input, flags) = u32(input)?;
        let (input, speed_min) = u32(input)?;
        let (input, speed_max) = u32(input)?;
        let (input, colors_min) = u32(input)?;
        let (input, colors_max) = u32(input)?;
        let (input, speed) = u32(input)?;
        let (input, direction) = u32(input)?;
        let (input, color_mode) = color_mode(input)?;
        let (input, num_colors) = u16(input)?;
        let (input, colors) = count(color, num_colors as usize)(input)?;
        Ok((
            input,
            Mode {
                name: name.into(),
                value,
                flags: ModeFlags::from_bits_retain(flags),
                speed_min,
                speed_max,
                colors_min,
                color_mode,
                colors_max,
                speed,
                direction,
                colors,
            },
        ))
    }

    fn controller_data(input: &[u8]) -> IResult<&[u8], ControllerData> {
        let (input, _size) = u32(input)?;
        let (input, ty) = controller_type(input)?;
        let (input, name_len) = u16(input)?;
        let (input, name) = null_terminated_string(name_len, input)?;
        let (input, description_len) = u16(input)?;
        let (input, description) = null_terminated_string(description_len, input)?;
        let (input, version_len) = u16(input)?;
        let (input, version) = null_terminated_string(version_len, input)?;
        let (input, serial_len) = u16(input)?;
        let (input, serial) = null_terminated_string(serial_len, input)?;
        let (input, location_len) = u16(input)?;
        let (input, location) = null_terminated_string(location_len, input)?;
        let (input, num_modes) = u16(input)?;
        let (input, active_mode) = u32(input)?;
        let (input, modes) = count(mode, num_modes as usize)(input)?;
        let (input, num_zones) = u16(input)?;
        let (input, zones) = count(zone, num_zones as usize)(input)?;
        let (input, num_leds) = u16(input)?;
        let (input, leds) = count(led, num_leds as usize)(input)?;
        let (input, num_colors) = u16(input)?;
        let (input, colors) = count(color, num_colors as usize)(input)?;
        Ok((
            input,
            ControllerData {
                ty,
                name: name.into(),
                description: description.into(),
                version: version.into(),
                serial: serial.into(),
                location: location.into(),
                modes,
                active_mode,
                zones,
                leds,
                colors,
            },
        ))
    }

    pub(super) fn packet_header(input: &[u8]) -> IResult<&[u8], PacketHeader> {
        let (input, _) = tag(b"ORGB")(input)?;
        let (input, dev_idx) = u32(input)?;
        let (input, pkt_id) = u32(input)?;
        let (input, pkt_size) = u32(input)?;
        Ok((
            input,
            PacketHeader {
                _dev_idx: dev_idx,
                pkt_id,
                pkt_size,
            },
        ))
    }

    pub(super) fn response(header: PacketHeader, input: &[u8]) -> IResult<&[u8], Response> {
        match header.pkt_id {
            0 => map(u32, Response::ControllerCount)(input),
            1 => map(controller_data, Response::ControllerData)(input),
            40 => map(u32, Response::ProtocolVersion)(input),
            100 => Ok((input, Response::DeviceListUpdated)),
            _ => panic!("Unknown command id"),
        }
    }
}

mod unparse {
    use super::*;

    pub fn u16(x: u16, output: &mut Vec<u8>) {
        output.extend(x.to_ne_bytes());
    }

    pub fn u32(x: u32, output: &mut Vec<u8>) {
        output.extend(x.to_ne_bytes())
    }

    pub fn color(c: Rgb, output: &mut Vec<u8>) {
        u32(u32::from_ne_bytes([c.0, c.1, c.2, 0x00]), output);
    }
}
