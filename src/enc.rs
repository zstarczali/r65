//! Character encodings: ascii, petscii_mixed/upper, screencode_mixed/upper.
//! The petscii tables come from the original; screen codes are derived from
//! petscii with the same rule (see MECHANICS.md).

use crate::tables::{PETSCII_MIXED, PETSCII_UPPER};
use std::collections::HashMap;

pub const NAMES: [&str; 5] = ["ascii", "petscii_mixed", "petscii_upper", "screencode_mixed", "screencode_upper"];

pub struct Encodings {
    maps: Vec<HashMap<char, u8>>,
}

fn petscii_to_screen(v: u32) -> u32 {
    if v < 32 {
        v + 128
    } else if v < 64 {
        v
    } else if v < 96 {
        v - 64
    } else if v < 128 {
        v - 32
    } else if v < 160 {
        v + 64
    } else if v < 192 {
        v - 64
    } else if v < 255 {
        v - 128
    } else {
        94
    }
}

fn make(table: &[u32], screen: bool) -> HashMap<char, u8> {
    let mut m = HashMap::new();
    for pair in table.chunks(2) {
        let mut b = pair[0];
        if screen {
            b = petscii_to_screen(b);
        }
        if let Some(c) = char::from_u32(pair[1]) {
            m.insert(c, b as u8);
        }
    }
    m
}

impl Default for Encodings {
    fn default() -> Self {
        Self::new()
    }
}

impl Encodings {
    pub fn new() -> Self {
        let mut ascii = HashMap::new();
        for i in 0..255u32 {
            ascii.insert(char::from_u32(i).unwrap(), i as u8);
        }
        Encodings {
            maps: vec![ascii, make(PETSCII_MIXED, false), make(PETSCII_UPPER, false), make(PETSCII_MIXED, true), make(PETSCII_UPPER, true)],
        }
    }
    pub fn index(name: &str) -> Option<usize> {
        NAMES.iter().position(|n| *n == name)
    }
    /// converts one character; unknown characters keep their own code
    pub fn conv(&self, enc: usize, c: char) -> u8 {
        let u = c as u32;
        if (0x800..0x900).contains(&u) {
            return (u - 0x800) as u8;
        }
        match self.maps[enc].get(&c) {
            Some(b) => *b,
            None => u as u8,
        }
    }
}
