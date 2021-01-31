use crate::hardware::interrupt_handler::{InterruptHandler, InterruptLine};
use std::fmt::Display;
use bitflags::_core::fmt::Formatter;

use strum::IntoEnumIterator;
// 0.17.1
use strum_macros::EnumIter; // 0.17.1


#[derive(Copy, Clone, Eq, PartialEq, Hash, EnumIter)]
pub enum Button {
    A,
    B,
    UP,
    DOWN,
    LEFT,
    RIGHT,
    START,
    SELECT,
}

impl Display for Button {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), ::std::fmt::Error> {
        match *self {
            Button::A => f.write_str("A"),
            Button::B => f.write_str("B"),
            Button::UP => f.write_str("UP"),
            Button::DOWN => f.write_str("DOWN"),
            Button::LEFT => f.write_str("LEFT"),
            Button::RIGHT => f.write_str("RIGHT"),
            Button::START => f.write_str("START"),
            Button::SELECT => f.write_str("SELECT"),
        }
    }
}

pub trait Controller {
    fn is_pressed(&self, button: Button) -> bool;

    fn tick(&self);

    fn any_pressed(&self) -> bool {
        Button::iter().any(|button| { self.is_pressed(button) })
    }
}

pub struct InputController<C: Controller> {
    pub register: P1,
    pub controller: C,
}

impl<C: Controller> InputController<C> {
    pub fn new(controller: C) -> InputController<C> {
        Self {
            register: P1::INITIAL_STATE,
            controller,
        }
    }
    pub fn update_state(&mut self, interrupts: &mut InterruptHandler) {
        let mut register = self.register.clone();
        self.controller.tick();

        // if self.controller.any_pressed() {
        //  //   println!("SOMETHING IS PRESSED: Button: {} DIRECT: {}", self.register.contains(P1::SELECT_BUTTON), self.register.contains(P1::SELECT_DIRECTIONAL));
        // }

        if self.register.contains(P1::SELECT_BUTTON) {
            //  println!("check Button");
            if self.controller.is_pressed(Button::START) { register.insert(P1::P13) } else { register.remove(P1::P13) }
            if self.controller.is_pressed(Button::SELECT) { register.insert(P1::P12) } else { register.remove(P1::P12) }
            if self.controller.is_pressed(Button::A) { register.insert(P1::P10) } else { register.remove(P1::P10) }
            if self.controller.is_pressed(Button::B) { register.insert(P1::P11) } else { register.remove(P1::P11) }
        } else if self.register.contains(P1::SELECT_DIRECTIONAL) {
            // println!("check direction");
            if self.controller.is_pressed(Button::DOWN) { register.insert(P1::P13) } else { register.remove(P1::P13) }
            if self.controller.is_pressed(Button::UP) { register.insert(P1::P12) } else { register.remove(P1::P12) }
            if self.controller.is_pressed(Button::RIGHT) { register.insert(P1::P10) } else { register.remove(P1::P10) }
            if self.controller.is_pressed(Button::LEFT) { register.insert(P1::P11) } else { register.remove(P1::P11) }
        }
        if register != self.register {
          //  println!("Trigger joypad");
            self.register = register;
            interrupts.request(InterruptLine::JOYPAD, true);
        }
    }

    pub fn write_register(&mut self, value: u8) {
        self.register = P1::from_bits_truncate(!value);
        self.register &= P1::WRITABLE;
    }

    pub fn read_register(&self) -> u8 {
     //   println!("Reading Joy Reg: {:#b}", !self.register.bits());
        !self.register.bits()
    }
}

bitflags!(
  /// P1 register
  ///
  /// Bits are inverted in get_register/set_register, so in P1
  /// a set bit is 1 as usual.
  pub struct P1: u8 {
    const P10                = 1 << 0; // P10: →, A
    const P11                = 1 << 1; // P11: ←, B
    const P12                = 1 << 2; // P12: ↑, Select
    const P13                = 1 << 3; // P13: ↓, Start
    const SELECT_DIRECTIONAL = 1 << 4; // P14: Select dpad
    const SELECT_BUTTON      = 1 << 5; // P15: Select buttons

    /// Only select bits are writable
    const WRITABLE =
      P1::SELECT_DIRECTIONAL.bits | P1::SELECT_BUTTON.bits;

    /// DMG: initial state 0xCF
    /// See docs/accuracy/joypad.markdown
    const INITIAL_STATE = P1::WRITABLE.bits;
  }
);

impl P1 {
    fn directional(key: &Button) -> P1 {
        match *key {
            Button::RIGHT => P1::P10,
            Button::LEFT => P1::P11,
            Button::UP => P1::P12,
            Button::DOWN => P1::P13,
            _ => P1::empty(),
        }
    }
    fn button(key: &Button) -> P1 {
        match *key {
            Button::A => P1::P10,
            Button::B => P1::P11,
            Button::SELECT => P1::P12,
            Button::START => P1::P13,
            _ => P1::empty(),
        }
    }
}