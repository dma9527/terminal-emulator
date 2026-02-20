/// Table-driven VT parser based on Paul Williams' state diagram.
/// Reference: https://vt100.net/emu/dec_ansi_parser

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    OscString,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    SosPmApcString,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Printable character
    Print(char),
    /// C0 control (BEL, BS, HT, LF, CR, etc.)
    Execute(u8),
    /// CSI dispatch: final byte, params, intermediates
    CsiDispatch {
        final_byte: u8,
        params: Vec<u16>,
        intermediates: Vec<u8>,
    },
    /// ESC dispatch
    EscDispatch {
        final_byte: u8,
        intermediates: Vec<u8>,
    },
    /// OSC string complete
    OscDispatch(Vec<u8>),
    /// No action
    None,
}

pub struct VtParser {
    state: State,
    params: Vec<u16>,
    current_param: u16,
    intermediates: Vec<u8>,
    osc_data: Vec<u8>,
}

impl VtParser {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            params: Vec::with_capacity(16),
            current_param: 0,
            intermediates: Vec::with_capacity(4),
            osc_data: Vec::with_capacity(256),
        }
    }

    /// Feed a single byte, return the resulting action.
    pub fn advance(&mut self, byte: u8) -> Action {
        // Anywhere transitions (highest priority)
        match byte {
            0x18 | 0x1a => {
                self.state = State::Ground;
                return Action::Execute(byte);
            }
            0x1b => {
                self.clear();
                self.state = State::Escape;
                return Action::None;
            }
            _ => {}
        }

        match self.state {
            State::Ground => self.ground(byte),
            State::Escape => self.escape(byte),
            State::EscapeIntermediate => self.escape_intermediate(byte),
            State::CsiEntry => self.csi_entry(byte),
            State::CsiParam => self.csi_param(byte),
            State::CsiIntermediate => self.csi_intermediate(byte),
            State::CsiIgnore => self.csi_ignore(byte),
            State::OscString => self.osc_string(byte),
            _ => Action::None, // TODO: DCS, SOS/PM/APC
        }
    }

    /// Feed a slice of bytes, collecting all actions.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Action> {
        data.iter()
            .map(|&b| self.advance(b))
            .filter(|a| *a != Action::None)
            .collect()
    }

    fn clear(&mut self) {
        self.params.clear();
        self.current_param = 0;
        self.intermediates.clear();
        self.osc_data.clear();
    }

    fn ground(&mut self, byte: u8) -> Action {
        match byte {
            0x00..=0x1f => Action::Execute(byte),
            0x20..=0x7e => Action::Print(byte as char),
            0x7f => Action::None, // DEL
            0x80..=0xff => {
                // UTF-8 lead/continuation — simplified: treat as printable
                Action::Print(char::REPLACEMENT_CHARACTER)
            }
        }
    }

    fn escape(&mut self, byte: u8) -> Action {
        match byte {
            0x20..=0x2f => {
                self.intermediates.push(byte);
                self.state = State::EscapeIntermediate;
                Action::None
            }
            0x5b => {
                // '[' → CSI
                self.clear();
                self.state = State::CsiEntry;
                Action::None
            }
            0x5d => {
                // ']' → OSC
                self.osc_data.clear();
                self.state = State::OscString;
                Action::None
            }
            0x30..=0x7e => {
                self.state = State::Ground;
                Action::EscDispatch {
                    final_byte: byte,
                    intermediates: self.intermediates.clone(),
                }
            }
            _ => Action::None,
        }
    }

    fn escape_intermediate(&mut self, byte: u8) -> Action {
        match byte {
            0x20..=0x2f => {
                self.intermediates.push(byte);
                Action::None
            }
            0x30..=0x7e => {
                self.state = State::Ground;
                Action::EscDispatch {
                    final_byte: byte,
                    intermediates: self.intermediates.clone(),
                }
            }
            _ => Action::None,
        }
    }

    fn csi_entry(&mut self, byte: u8) -> Action {
        match byte {
            0x30..=0x39 => {
                self.current_param = (byte - b'0') as u16;
                self.state = State::CsiParam;
                Action::None
            }
            0x3b => {
                self.params.push(0);
                self.state = State::CsiParam;
                Action::None
            }
            0x3c..=0x3f => {
                // Private marker (e.g., '?')
                self.intermediates.push(byte);
                self.state = State::CsiParam;
                Action::None
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
                self.state = State::CsiIntermediate;
                Action::None
            }
            0x40..=0x7e => {
                self.state = State::Ground;
                Action::CsiDispatch {
                    final_byte: byte,
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
                }
            }
            _ => Action::None,
        }
    }

    fn csi_param(&mut self, byte: u8) -> Action {
        match byte {
            0x30..=0x39 => {
                self.current_param = self.current_param.saturating_mul(10)
                    .saturating_add((byte - b'0') as u16);
                Action::None
            }
            0x3b => {
                self.params.push(self.current_param);
                self.current_param = 0;
                Action::None
            }
            0x20..=0x2f => {
                self.params.push(self.current_param);
                self.intermediates.push(byte);
                self.state = State::CsiIntermediate;
                Action::None
            }
            0x40..=0x7e => {
                self.params.push(self.current_param);
                self.state = State::Ground;
                Action::CsiDispatch {
                    final_byte: byte,
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
                }
            }
            _ => {
                self.state = State::CsiIgnore;
                Action::None
            }
        }
    }

    fn csi_intermediate(&mut self, byte: u8) -> Action {
        match byte {
            0x20..=0x2f => {
                self.intermediates.push(byte);
                Action::None
            }
            0x40..=0x7e => {
                self.state = State::Ground;
                Action::CsiDispatch {
                    final_byte: byte,
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
                }
            }
            _ => {
                self.state = State::CsiIgnore;
                Action::None
            }
        }
    }

    fn csi_ignore(&mut self, byte: u8) -> Action {
        if (0x40..=0x7e).contains(&byte) {
            self.state = State::Ground;
        }
        Action::None
    }

    fn osc_string(&mut self, byte: u8) -> Action {
        match byte {
            0x07 => {
                // BEL terminates OSC
                self.state = State::Ground;
                Action::OscDispatch(self.osc_data.clone())
            }
            0x9c => {
                // ST terminates OSC
                self.state = State::Ground;
                Action::OscDispatch(self.osc_data.clone())
            }
            _ => {
                self.osc_data.push(byte);
                Action::None
            }
        }
    }
}

impl Default for VtParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printable() {
        let mut p = VtParser::new();
        assert_eq!(p.advance(b'A'), Action::Print('A'));
    }

    #[test]
    fn test_csi_cursor_up() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b[5A");
        assert_eq!(actions, vec![Action::CsiDispatch {
            final_byte: b'A', params: vec![5], intermediates: vec![],
        }]);
    }

    #[test]
    fn test_csi_sgr() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b[1;31m");
        assert_eq!(actions, vec![Action::CsiDispatch {
            final_byte: b'm', params: vec![1, 31], intermediates: vec![],
        }]);
    }

    #[test]
    fn test_osc() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b]0;title\x07");
        assert_eq!(actions, vec![Action::OscDispatch(b"0;title".to_vec())]);
    }

    #[test]
    fn test_csi_no_params() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b[H"); // CUP with no params
        assert_eq!(actions, vec![Action::CsiDispatch {
            final_byte: b'H', params: vec![], intermediates: vec![],
        }]);
    }

    #[test]
    fn test_csi_private_mode() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b[?1049h");
        assert_eq!(actions, vec![Action::CsiDispatch {
            final_byte: b'h', params: vec![1049], intermediates: vec![b'?'],
        }]);
    }

    #[test]
    fn test_esc_dispatch() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b7"); // DECSC
        assert_eq!(actions, vec![Action::EscDispatch {
            final_byte: b'7', intermediates: vec![],
        }]);
    }

    #[test]
    fn test_c0_controls() {
        let mut p = VtParser::new();
        assert_eq!(p.advance(0x0a), Action::Execute(0x0a)); // LF
        assert_eq!(p.advance(0x0d), Action::Execute(0x0d)); // CR
        assert_eq!(p.advance(0x08), Action::Execute(0x08)); // BS
        assert_eq!(p.advance(0x09), Action::Execute(0x09)); // HT
        assert_eq!(p.advance(0x07), Action::Execute(0x07)); // BEL
    }

    #[test]
    fn test_esc_interrupted_by_esc() {
        let mut p = VtParser::new();
        // Start ESC, then another ESC interrupts
        let actions = p.feed(b"\x1b\x1b[5A");
        assert_eq!(actions, vec![Action::CsiDispatch {
            final_byte: b'A', params: vec![5], intermediates: vec![],
        }]);
    }

    #[test]
    fn test_csi_multiple_params() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b[1;2;3;4;5m");
        assert_eq!(actions, vec![Action::CsiDispatch {
            final_byte: b'm', params: vec![1, 2, 3, 4, 5], intermediates: vec![],
        }]);
    }

    #[test]
    fn test_csi_semicolon_default_params() {
        let mut p = VtParser::new();
        // ESC [ ; H → params should be [0, 0] (both default)
        let actions = p.feed(b"\x1b[;H");
        assert_eq!(actions, vec![Action::CsiDispatch {
            final_byte: b'H', params: vec![0, 0], intermediates: vec![],
        }]);
    }

    #[test]
    fn test_mixed_text_and_escapes() {
        let mut p = VtParser::new();
        let actions = p.feed(b"AB\x1b[1mCD");
        assert_eq!(actions, vec![
            Action::Print('A'),
            Action::Print('B'),
            Action::CsiDispatch { final_byte: b'm', params: vec![1], intermediates: vec![] },
            Action::Print('C'),
            Action::Print('D'),
        ]);
    }

    #[test]
    fn test_osc_st_terminator() {
        let mut p = VtParser::new();
        let actions = p.feed(b"\x1b]0;title\x9c"); // ST = 0x9C
        assert_eq!(actions, vec![Action::OscDispatch(b"0;title".to_vec())]);
    }

    #[test]
    fn test_cancel_with_can() {
        let mut p = VtParser::new();
        // Start CSI, then CAN (0x18) cancels
        let actions = p.feed(b"\x1b[5\x18A");
        // CAN executes, then 'A' prints
        assert_eq!(actions, vec![
            Action::Execute(0x18),
            Action::Print('A'),
        ]);
    }

    #[test]
    fn test_del_ignored() {
        let mut p = VtParser::new();
        assert_eq!(p.advance(0x7f), Action::None);
    }
}
