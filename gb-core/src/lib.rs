#![no_std]

extern crate alloc;
#[macro_use]
extern crate unroll;
mod cpu;
pub mod hardware;
mod memory;
mod util;
pub mod gameboy;


extern crate num_traits;

#[macro_use]
extern crate enum_display_derive;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate num_derive;

#[cfg(test)]
mod tests {

}
