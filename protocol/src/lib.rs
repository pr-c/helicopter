#![no_std]

use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize)]
pub struct JoystickInput {
    pitch: u16
}

impl JoystickInput {
    pub fn new(pitch: u16) -> JoystickInput {
        JoystickInput{
            pitch
        }
    }

    pub fn get_pitch(&self) -> u16 {
        self.pitch
    }
}