//! Implements the Tockloader protocol.
//!
//! TockOS applications are loaded with `tockloader`.
//! This speaks to the TockOS bootloader using a specific
//! protocol. This crate implements that protocol so
//! that you can write future tockloader compatible bootloaders
//! in Rust!
//#![no_std]

enum State {
    Loading,
    Escape,
}

/// Commands supported by the protocol
#[derive(Debug)]
pub enum Command<'a> {
    Ping,
    Info,
    Reset,
    ErasePage(u32),
    WritePage(u32, &'a [u8]),
    ReadRange,
    SetAttribute,
    GetAttribute,
    CrcInternalFlash,
    ChangeBaudRate,
    BadCommand,
}

/// The Parser takes bytes and gives you `Command`s.
pub struct Parser {
    state: State,
    buffer: [u8; 520],
    count: usize,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            state: State::Loading,
            buffer: [0u8; 520],
            count: 0,
        }
    }

    pub fn receive(&mut self, ch: u8) -> Option<Command> {
        match self.state {
            State::Loading => self.handle_loading(ch),
            State::Escape => self.handle_escape(ch),
        }
    }

    fn load_char(&mut self, ch: u8) {
        if self.count < self.buffer.len() {
            self.buffer[self.count] = ch;
            self.count = self.count + 1;
        }
    }

    fn handle_loading(&mut self, ch: u8) -> Option<Command> {
        if ch == 0xFC {
            self.state = State::Escape;
        } else {
            self.load_char(ch);
        }
        None
    }

    fn handle_escape(&mut self, ch: u8) -> Option<Command> {
        self.state = State::Loading;
        let result = match ch {
            0xFC => {
                // Double escape means just load an escape
                self.load_char(ch);
                None
            }
            0x01 => Some(Command::Ping),
            0x03 => Some(Command::Info),
            0x05 => Some(Command::Reset),
            0x06 => {
                if self.count >= 4 {
                    // Little-endian address in buffer
                    let addr = Self::parse_u32(&self.buffer[self.count - 4..self.count - 1]);
                    Some(Command::ErasePage(addr))
                } else {
                    Some(Command::BadCommand)
                }
            }
            0x07 => {
                let num_expected_bytes: usize = 512 + 4;
                if self.count >= num_expected_bytes {
                    // Little-endian address in buffer
                    let start = self.count - num_expected_bytes;
                    let addr = Self::parse_u32(&self.buffer[start..start + 4]);
                    Some(Command::WritePage(
                        addr,
                        &self.buffer[start + 4..start + num_expected_bytes],
                    ))
                } else {
                    Some(Command::BadCommand)
                }
            }
            _ => None,
        };
        if result.is_some() {
            self.count = 0;
        }
        result
    }

    fn parse_u32(data: &[u8]) -> u32 {
        println!("Parsing: {:?}", data);
        let mut result: u32 = 0;
        result += data[3] as u32;
        result <<= 8;
        result += data[2] as u32;
        result <<= 8;
        result += data[1] as u32;
        result <<= 8;
        result += data[0] as u32;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_ping() {
        let mut p = Parser::new();
        {
            let o = p.receive(0xFF);
            assert!(o.is_none());
        }
        {
            let o = p.receive(0xFC);
            assert!(o.is_none());
        }
        let o = p.receive(0x01);
        match o.unwrap() {
            Command::Ping => {}
            e => panic!("Did not expect: {:?}", e),
        }
    }

    #[test]
    fn check_info() {
        let mut p = Parser::new();
        {
            let o = p.receive(0xFF);
            assert!(o.is_none());
        }
        {
            let o = p.receive(0xFC);
            assert!(o.is_none());
        }
        let o = p.receive(0x03);
        match o.unwrap() {
            Command::Info => {}
            e => panic!("Did not expect: {:?}", e),
        }
    }

    fn make_byte(index: u32) -> u8 {
        (index & 0xFF) as u8
    }

    #[test]
    fn check_write() {
        let mut p = Parser::new();
        p.receive(0xEF);
        p.receive(0xBE);
        p.receive(0xAD);
        p.receive(0xDE);
        for i in 0..512 {
            let datum = make_byte(i);
            p.receive(datum);
            if datum == 0xFC {
                p.receive(datum);
            }
        }
        p.receive(0xFC); // Escape
        let o = p.receive(0x07); // WriteFlash
        match o.unwrap() {
            Command::WritePage(addr, ref page) => {
                assert_eq!(addr, 0xDEADBEEF);
                assert_eq!(page.len(), 512);
                for i in 0..512 {
                    let datum = make_byte(i);
                    assert_eq!(datum, page[i as usize]);
                }
            }
            e => panic!("Did not expect: {:?}", e),
        }
    }
}
