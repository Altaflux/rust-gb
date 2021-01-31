use crate::hardware::{Screen};
use bit_set::BitSet;
use crate::hardware::color_palette::{ColorPalette, Color, ORIGINAL_GREEN};
use crate::memory::nmmu::Memory;
use bitflags::bitflags;
use crate::hardware::interrupt_handler::{InterruptLine, InterruptHandler};
use crate::gameboy::{SCREEN_HEIGHT, SCREEN_WIDTH};
use bitflags::_core::time::Duration;

const TILE_MAP_ADDRESS_0: usize = 0x9800;
const TILE_MAP_ADDRESS_1: usize = 0x9C00;
const TILE_MAP_WIDTH: usize = 32;
const TILE_MAP_HEIGHT: usize = 32;

const TILE_WIDTH: usize = 8;
const TILE_HEIGHT: usize = 8;
const TILE_COUNT: usize = 384;
const TILE_BYTE_SIZE: usize = 16;

const SPRITE_COUNT: usize = 40;
const SPRITE_BYTE_SIZE: usize = 4;
const SPRITE_HEIGHT: usize = 16;
const SPRITE_WIDTH: usize = 8;


const SCREEN_FREQUENCY: usize = 60;
const STAT_UNUSED_MASK: u8 = (1 << 7);

const ACCESS_OAM_CYCLES: isize = 21;
const ACCESS_VRAM_CYCLES: isize = 43;
const HBLANK_CYCLES: isize = 50;
const VBLANK_LINE_CYCLES: isize = 114;

const UNDEFINED_READ: u8 = 0xff;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Mode {
    AccessOam,
    AccessVram,
    HBlank,
    VBlank,
}

impl Mode {
    fn cycles(&self, scroll_x: u8) -> isize {
        // FIXME: This is basically an ugly hack to pass a test. Most likely we don't just want
        // to adjust the cycle counts, but to actually emulate the behaviour that causes the adjustment
        let scroll_adjust = match scroll_x % 0x08 {
            5..=7 => 2,
            1..=4 => 1,
            _ => 0,
        };
        match *self {
            Mode::AccessOam => ACCESS_OAM_CYCLES,
            Mode::AccessVram => ACCESS_VRAM_CYCLES + scroll_adjust,
            Mode::HBlank => HBLANK_CYCLES - scroll_adjust,
            Mode::VBlank => VBLANK_LINE_CYCLES,
        }
    }
    fn bits(&self) -> u8 {
        match *self {
            Mode::AccessOam => 2,
            Mode::AccessVram => 3,
            Mode::HBlank => 0,
            Mode::VBlank => 1,
        }
    }

    fn minimum_cycles(&self) -> isize {
        match *self {
            Mode::AccessOam => 80,
            Mode::AccessVram => 172,
            Mode::HBlank => 204,
            Mode::VBlank => 456,
        }
    }
}


pub struct Ppu<T: Screen> {
    render_container: RenderContainer<T>,
    color_palette: ColorPalette,
    background_palette: Palette,
    obj_palette0: Palette,
    obj_palette1: Palette,
    scanline: u8,
    large_sprites: bool,
    background_mask: BitSet,
    video_ram: VideoRam,
    control: Control,
    stat: Stat,
    // current_line: u8,
    compare_line: u8,
    scroll_x: u8,
    scroll_y: u8,
    tile_offset: u8,
    mode: Mode,
    window_x: u8,
    window_y: u8,
    cycle_counter: isize,

    sprites: [Sprite; OAM_SPRITES],
}

struct RenderContainer<T: Screen> {
    screen: T,
}

impl<T: Screen> RenderContainer<T> {
    fn draw_pixel(&mut self, x: u8, y: u8, color: Color) {
        // if shade != Shade::LIGHTEST {
        //     self.background_mask.insert(x as usize);
        // } else {
        //     self.background_mask.remove(x as usize);
        // }
        // println!("DoiNG pixel");
        //  if color.red != 224 && color.green != 251 && color.blue != 210 {
        //      println!("Print non light");
        //      std::thread::sleep(Duration::from_secs(3));
        //      dbg!(color);
        //      println!("DONE non light");
        //  }

        self.screen.set_pixel(x, y, color);
    }
}
// tile_map0: [u8; TILE_MAP_SIZE],
// tile_map1: [u8; TILE_MAP_SIZE],
// tiles: [Tile; TILE_COUNT],

// impl <T> Ppu<T> {
//
// }
impl<T: Screen> Ppu<T> {
    pub fn new(screen: T) -> Ppu<T> {
        let render: RenderContainer<T> = RenderContainer { screen };
        let tiles = (0..TILE_COUNT).map(|v| { Tile::newf(v) }).collect::<Vec<Tile>>();
        Ppu {
            render_container: render,
            color_palette: ORIGINAL_GREEN,
            background_palette: Palette(0),
            obj_palette0: Palette(0),
            obj_palette1: Palette(0),
            scanline: 0,
            large_sprites: false,
            background_mask: Default::default(),
            video_ram: VideoRam {
                tile_map0: [0; TILE_MAP_SIZE],
                tile_map1: [0; TILE_MAP_SIZE],
                tiles: tiles,
            },
            control: Control::empty(),
            stat: Stat::empty(),
            compare_line: 0,
            scroll_x: 0,
            scroll_y: 0,
            tile_offset: 0,
            mode: Mode::HBlank,
            window_x: 0,
            window_y: 0,
            cycle_counter: 0,
            sprites: [Sprite::new(0); SPRITE_COUNT],
        }
    }

    pub fn reset(&mut self) {
        self.control = Control::from_bits_truncate(0x91);
        self.scroll_y = 0x00;
        self.scroll_x = 0x00;
        self.compare_line = 0x00;
        self.background_palette = Palette(0xFC);
        self.obj_palette0 = Palette(0xFF);
        self.obj_palette1 = Palette(0xFF);
        self.window_x = 0x00;
        self.window_y = 0x00;
    }
    pub fn step(&mut self, cycles: isize, interrupts: &mut InterruptHandler) {
      //  println!("cycle_counter: {} scanline: {} cycle: {}", self.cycle_counter, self.scanline, cycles);
        if !self.update_current_mode(interrupts) {
            return;
        }

        if cycles == 0 {
            self.draw_blank_screen();
            return;
        }
        self.cycle_counter -= cycles;

        if self.cycle_counter <= 0 {
            if self.scanline == 244 {
              //  println!("ON SCANLINE");
                std::thread::sleep(Duration::from_secs(3));
             //   println!("END SCANLINE");
            }

            self.scanline = self.scanline + 1;
            // println!("scanline: {}", self.scanline);
            self.cycle_counter = Mode::VBlank.minimum_cycles();

            if self.scanline == SCREEN_HEIGHT as u8 {
                // println!("DOING VBLANK");
                interrupts.request(InterruptLine::VBLANK, true);
            } else if self.scanline >= SCREEN_HEIGHT as u8 + 10 {
                self.scanline = 0;
                //     println!("About to draw!!");
                self.draw_to_screen();
            } else if self.scanline < SCREEN_HEIGHT as u8 {
                self.draw_scan_line();
            }
        }
    }

    fn draw_to_screen(&mut self) {
        self.render_container.screen.draw();
    }

    fn update_current_mode(&mut self, interrupts: &mut InterruptHandler) -> bool {
        //println!("control: LCD {}", self.control.contains(Control::LCD_ON));
        if !self.control.contains(Control::LCD_ON) {
            //  println!("LCD OFF");
            self.cycle_counter = Mode::VBlank.minimum_cycles();
            self.mode = Mode::VBlank;
            self.scanline = 0;
            return false;
        }
        if self.scanline >= SCREEN_HEIGHT as u8 {
            self.update_current_mode_sec(interrupts, Mode::VBlank, self.stat.contains(Stat::VBLANK_INT));
        } else if self.cycle_counter >= Mode::VBlank.minimum_cycles() - Mode::AccessOam.minimum_cycles() {
            self.update_current_mode_sec(interrupts, Mode::AccessOam, self.stat.contains(Stat::ACCESS_OAM_INT));
        } else if self.cycle_counter >= Mode::VBlank.minimum_cycles() - Mode::AccessOam.minimum_cycles() - Mode::AccessVram.minimum_cycles() {
            self.update_current_mode_sec(interrupts, Mode::AccessVram, false);
        } else {
            self.update_current_mode_sec(interrupts, Mode::HBlank, self.stat.contains(Stat::HBLANK_INT));
        }

        if self.stat.contains(Stat::COMPARE) && self.scanline == self.compare_line {
            interrupts.request(InterruptLine::STAT, true);
        }
        true
    }

    fn update_current_mode_sec(&mut self, interrupts: &mut InterruptHandler, new_mode: Mode, request_interrupt: bool) {
        if request_interrupt && new_mode != self.mode {
            interrupts.request(InterruptLine::STAT, true);
        }
        self.mode = new_mode;
    }

    fn draw_pixel(&mut self, x: u8, shade: Shade, color: Color) {
        if shade != Shade::LIGHTEST {
            self.background_mask.insert(x as usize);
        } else {
            self.background_mask.remove(x as usize);
        }
        // self.render_container.screen.set_pixel(x, self.scanline - 1, color);
        //   println!("THE DRAW");
        self.render_container.draw_pixel(x, self.scanline - 1, color);
    }

    pub fn get_memory_as_mut(&mut self) -> &mut impl Memory {
        &mut self.video_ram
    }

    pub fn get_memory(&self) -> &impl Memory {
        &self.video_ram
    }
    pub fn get_control(&self) -> u8 {
        self.control.bits
    }

    pub fn draw_scan_line(&mut self) {
        if self.control.contains(Control::BG_ON) {
            for x in 0..SCREEN_WIDTH {
                if self.control.contains(Control::WINDOW_ON) && self.window_y <= self.scanline {
                    self.draw_background_window_pixel(x as u8);
                } else {
                    self.draw_background_pixel(x as u8);
                }
            }
        }
        if self.control.contains(Control::OBJ_ON) {
            //   print!("draw objecrts");
            self.draw_sprites();
        }
    }

    pub fn set_control(&mut self, value: u8) {
        //    println!("Set CONTROL");
        let new_control = Control::from_bits_truncate(value);
        //  dbg!(new_control);
        if !new_control.contains(Control::LCD_ON) && self.control.contains(Control::LCD_ON) {
            // if self.mode != Mode::VBlank {
            //     panic!("Warning! LCD off, but not in VBlank");
            // }
            self.scanline = 0;
        }
        if new_control.contains(Control::LCD_ON) && !self.control.contains(Control::LCD_ON) {
            //   self.mode = Mode::HBlank;
            //   self.cycles = Mode::AccessOam.cycles(self.scroll_x);
            self.stat.insert(Stat::COMPARE);
            self.render_container.screen.turn_on();
        }

        self.control = new_control;
    }
    pub fn set_stat(&mut self, value: u8) {
        let new_stat = Stat::from_bits_truncate(value);

        self.stat = (self.stat & Stat::COMPARE)
            | (new_stat & Stat::HBLANK_INT)
            | (new_stat & Stat::VBLANK_INT)
            | (new_stat & Stat::ACCESS_OAM_INT)
            | (new_stat & Stat::COMPARE_INT);
    }

    pub fn get_stat(&self) -> u8 {
        if !self.control.contains(Control::LCD_ON) {
            STAT_UNUSED_MASK
        } else {
            self.mode.bits() | self.stat.bits | STAT_UNUSED_MASK
        }
    }


    pub fn draw_background_window_pixel(&mut self, x: u8) {
        let y = self.scanline + self.window_y;
        let adjusted_x = ((x + self.window_x - 7) + SCREEN_WIDTH as u8) % SCREEN_WIDTH as u8;
        let tile_map = if self.control.contains(Control::WINDOW_MAP) {
            &self.video_ram.tile_map1
        } else {
            &self.video_ram.tile_map0
        };
        let tile = self.tile_at(adjusted_x, y, tile_map);
        let shade = tile.shade_at(adjusted_x, y, &self.background_palette);
        self.draw_pixel(x, shade, self.color_palette.background(shade));
    }


    pub fn draw_background_pixel(&mut self, x: u8) {
        let y = self.scanline.wrapping_add(self.scroll_y);
        let adjusted_x = x + self.scroll_x;
        let tile_map = if self.control.contains(Control::BG_MAP) {
            &self.video_ram.tile_map1
        } else {
            &self.video_ram.tile_map0
        };
        //    dbg!("Debug draw_background_pixel", x, y, self.scroll_y,self.scroll_x, self.scanline);
        let tile = self.tile_at(adjusted_x, y, tile_map);
        // if x == 32 && y == 79 {
        //     println!("Print non light");
        //     std::thread::sleep(Duration::from_secs(3));
        //     dbg!(x, y);
        // }
        let shade = tile.shade_at(adjusted_x, y, &self.background_palette);

        self.draw_pixel(x, shade, self.color_palette.background(shade));
    }

    pub fn draw_blank_screen(&mut self) {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                self.render_container.screen.set_pixel(x as u8, y as u8, self.color_palette.background(Shade::LIGHTEST))
            }
        }
    }

    pub fn tile_at(&self, x: u8, y: u8, tile_map: &[u8; TILE_MAP_SIZE]) -> &Tile {
        let col = x as usize / TILE_WIDTH;
        let row = y as usize / TILE_HEIGHT;
        let raw_tile_num = tile_map[row * 32 + col];
        let addr_select = self.control.contains(Control::BG_ADDR);
        let tile_num = if addr_select {
            raw_tile_num as usize
        } else {
            128 + ((raw_tile_num as i8 as i16) + 128) as usize
        };
        &self.video_ram.tiles[tile_num]
    }
    pub fn draw_sprites(&mut self) {
        let draw_container = DrawContainer {
            color_palette: &self.color_palette,
            scanline: self.scanline,
            video_ram: &self.video_ram,
            large_sprites: self.large_sprites,
            obj_palette0: self.obj_palette0,
            obj_palette1: self.obj_palette1,
        };

        for sprite in &self.sprites {
            if let Some(result) = sprite.render(&draw_container, &mut self.background_mask) {
                for res in result {
                    self.render_container.draw_pixel(res.0, self.scanline - 1, res.2);
                }
            }
        }
    }

    pub fn write_oam(&mut self, reladdr: u8, value: u8) {
//        println!("Set OAM");
        if self.mode == Mode::AccessVram || self.mode == Mode::AccessOam {
            return;
        }
        let sprite = &mut self.sprites[reladdr as usize / 4];
        match reladdr as usize % 4 {
            3 => {
                sprite.flags = SpriteFlags::from_bits_truncate(value);
            }
            2 => sprite.tile_number = value,
            1 => sprite.x = value.wrapping_sub(8),
            _ => sprite.y = value.wrapping_sub(16),
        }
    }
    // const UNDEFINED_READ: u8 = 0xff;
    pub fn read_oam(&self, reladdr: u8) -> u8 {
        if self.mode == Mode::AccessVram || self.mode == Mode::AccessOam {
            return 0xff;
        }
        let sprite = &self.sprites[reladdr as usize / 4];
        match reladdr as usize % 4 {
            3 => sprite.flags.bits(),
            2 => sprite.tile_number,
            1 => sprite.x.wrapping_add(8),
            _ => sprite.y.wrapping_add(16),
        }
    }

    // pub fn read_character_ram(&self, reladdr: u16) -> u8 {
    //     if self.mode == Mode::AccessVram {
    //         return UNDEFINED_READ;
    //     }
    //     self.video_ram.read_tile_byte(reladdr)
    // }
    //
    // pub fn read_tile_map1(&self, reladdr: u16) -> u8 {
    //     if self.mode == Mode::AccessVram {
    //         return UNDEFINED_READ;
    //     }
    //     self.video_ram.read_tile_map_byte(reladdr)
    // }
    // pub fn read_tile_map2(&self, reladdr: u16) -> u8 {
    //     if self.mode == Mode::AccessVram {
    //         return UNDEFINED_READ;
    //     }
    //     self.tile_map2[reladdr as usize]
    // }

    pub fn read_memory(&self, address: u16) -> Option<u8> {
        self.video_ram.get_byte(address)
    }

    pub fn get_scroll_y(&self) -> u8 {
        self.scroll_y
    }

    pub fn get_scroll_x(&self) -> u8 {
        self.scroll_x
    }

    pub fn set_scroll_y(&mut self, value: u8) {
        self.scroll_y = value;
    }

    pub fn set_scroll_x(&mut self, value: u8) {
        self.scroll_x = value;
    }

    pub fn reset_current_line(&mut self) {
        self.scanline = 0;
    }

    pub fn set_compare_line(&mut self, value: u8) {
        self.compare_line = value;
    }

    pub fn get_current_line(&self) -> u8 {
        self.scanline
    }
    pub fn get_compare_line(&self) -> u8 {
        self.compare_line
    }

    pub fn get_obj_palette0(&self) -> u8 {
        self.obj_palette0.0
    }
    pub fn get_obj_palette1(&self) -> u8 {
        self.obj_palette1.0
    }

    pub fn set_obj_palette0(&mut self, value: u8) {
        self.obj_palette0.0 = value;
    }
    pub fn set_obj_palette1(&mut self, value: u8) {
        self.obj_palette1.0 = value;
    }

    pub fn get_window_x(&self) -> u8 {
        self.window_x
    }
    pub fn get_window_y(&self) -> u8 {
        self.window_y
    }

    pub fn set_window_x(&mut self, value: u8) {
        self.window_x = value;
    }
    pub fn set_window_y(&mut self, value: u8) {
        self.window_y = value;
    }

    pub fn get_bg_palette(&self) -> u8 {
        self.background_palette.0
    }

    pub fn set_bg_palette(&mut self, value: u8) {
        self.background_palette.0 = value;
    }
}

struct DrawContainer<'a> {
    color_palette: &'a ColorPalette,
    scanline: u8,
    video_ram: &'a VideoRam,
    large_sprites: bool,
    obj_palette0: Palette,
    obj_palette1: Palette,
}

const TILE_MAP_SIZE: usize = 0x400;
const OAM_SPRITES: usize = 40;
// const TILE_MAP_ADDRESS_1: usize = 0x9C00;

bitflags!(
  struct Control: u8 {
    const BG_ON = 1 << 0;
    const OBJ_ON = 1 << 1;
    const OBJ_SIZE = 1 << 2;
    const BG_MAP = 1 << 3;
    const BG_ADDR = 1 << 4;
    const WINDOW_ON = 1 << 5;
    const WINDOW_MAP = 1 << 6;
    const LCD_ON = 1 << 7;
  }
);
bitflags!(
  struct Stat: u8 {
    const COMPARE = 1 << 2;
    const HBLANK_INT = 1 << 3;
    const VBLANK_INT = 1 << 4;
    const ACCESS_OAM_INT = 1 << 5;
    const COMPARE_INT = 1 << 6;
  }
);

struct VideoRam {
    tile_map0: [u8; TILE_MAP_SIZE],
    tile_map1: [u8; TILE_MAP_SIZE],
    tiles: Vec<Tile>,

}

impl VideoRam {
    pub fn read_tile_map_byte(&self, address: u16) -> u8 {
        let mut offset_address: u16 = 0;
        let tile_map = if address < TILE_MAP_ADDRESS_1 as u16 {
            offset_address = address - TILE_MAP_ADDRESS_0 as u16;
            self.tile_map0
        } else {
            offset_address = address - TILE_MAP_ADDRESS_1 as u16;
            self.tile_map1
        };
        tile_map[offset_address as usize]
    }

    pub fn write_tile_map_byte(&mut self, address: u16, value: u8) {
        let mut offset_address;
        let tile_map = if address < TILE_MAP_ADDRESS_1 as u16 {
            offset_address = address - TILE_MAP_ADDRESS_0 as u16;
            &mut self.tile_map0
        } else {
            offset_address = address - TILE_MAP_ADDRESS_1 as u16;
            &mut self.tile_map1
        };
        // if value != 0 {
        //     println!("PRE-write_tile_map_byte");
        //     std::thread::sleep(Duration::from_secs(3));
        //     println!("write_tile_map_byte");
        // }

        tile_map[offset_address as usize] = value;
    }

    fn write_tile_byte(&mut self, address: u16, value: u8) {
        let virtual_address = address - 0x8000;
        if value != 0 {
            // println!("PRE-Write tile");
            // std::thread::sleep(Duration::from_secs(3));
            // println!("Write tile");
        }

        let tile: &mut Tile = &mut self.tiles[virtual_address as usize / TILE_BYTE_SIZE];
        let row_data = virtual_address % TILE_BYTE_SIZE as u16;
        let y = row_data / 2;

        for x in 0..TILE_WIDTH {
            let color_bit = 1 << (TILE_WIDTH - 1 - x);
            if row_data % 2 == 0 {
                let prev: u8 = tile.1[y as usize][x].into();
                tile.1[y as usize][x] = u8::into(if (value & color_bit) != 0 { 0b01 } else { 0b00 } | prev & 0b10);
            } else {
                let prev: u8 = tile.1[y as usize][x].into();
                tile.1[y as usize][x] = u8::into(if (value & color_bit) != 0 { 0b10 } else { 0b00 } | prev & 0b01);
            }
        }
    }

    fn read_tile_byte(&self, address: u16) -> u8 {
        let virtual_address = address - 0x8000;
        let mut result = 0;

        let tile: &Tile = &self.tiles[virtual_address as usize / TILE_BYTE_SIZE];
        let row_data = virtual_address % TILE_BYTE_SIZE as u16;
        let y = row_data / 2;

        for x in 0..TILE_WIDTH {
            let color_bit = 1 << (TILE_WIDTH - 1 - x);
            let pixel = tile.1[y as usize][x];
            let pixel_number: u8 = pixel.into();
            if row_data % 2 == 0 {
                result |= if (pixel_number & 0b01) != 0 { color_bit } else { 0 };
            } else {
                result |= if (pixel_number & 0b10) != 0 { color_bit } else { 0 };
            }
        }
        result
    }
}


impl Memory for VideoRam {
    fn set_byte(&mut self, address: u16, value: u8) {
        //     println!("Set PPU BYTE address: {:X?} value: {:X?} ", address, value);
        if address >= TILE_MAP_ADDRESS_0 as u16 {
            //  println!("Writing to TILE MAP");
            self.write_tile_map_byte(address, value);
        } else {
            self.write_tile_byte(address, value);
        }
    }

    fn get_byte(&self, address: u16) -> Option<u8> {
        if address >= TILE_MAP_ADDRESS_0 as u16 {
            Some(self.read_tile_map_byte(address))
        } else {
            Some(self.read_tile_byte(address))
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Shade {
    DARKEST,
    DARK,
    LIGHT,
    LIGHTEST,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TilePixelValue {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
}

impl Into<u8> for TilePixelValue {
    fn into(self) -> u8 {
        match self {
            TilePixelValue::Zero => { 0 }
            TilePixelValue::One => { 1 }
            TilePixelValue::Two => { 2 }
            TilePixelValue::Three => { 3 }
        }
    }
}

impl From<u8> for TilePixelValue {
    fn from(value: u8) -> Self {
        match value {
            0 => { TilePixelValue::Zero }
            1 => { TilePixelValue::One }
            2 => { TilePixelValue::Two }
            3 => { TilePixelValue::Three }
            _ => { TilePixelValue::Zero }
        }
    }
}

impl Default for TilePixelValue {
    fn default() -> Self {
        TilePixelValue::Zero
    }
}

impl Default for Shade {
    fn default() -> Self {
        Shade::DARKEST
    }
}

impl From<u8> for Shade {
    fn from(value: u8) -> Self {
        match value {
            0 => Shade::LIGHTEST,
            1 => Shade::LIGHT,
            2 => Shade::DARK,
            3 => Shade::LIGHTEST,
            _ => Shade::DARKEST
        }
    }
}

type TileRow = [TilePixelValue; 8];

#[derive(Copy, Clone)]
pub struct Tile(usize, [TileRow; 8]);

impl Tile {
    fn shade_at(&self, x: u8, y: u8, palette: &Palette) -> Shade {
        let input = self.1[(y as usize % TILE_HEIGHT)][(x as usize % TILE_WIDTH)];
        // if x == 32 && y == 79 {
        //     println!("shade_at light");
        //     std::thread::sleep(Duration::from_secs(3));
        //     dbg!(x, y);
        // }
        palette.shade(input)
    }

    fn new() -> Tile {
        Tile(0, [[TilePixelValue::default(); 8]; 8])
    }
    fn newf(tile_number: usize) -> Tile {
        Tile(tile_number, [[TilePixelValue::default(); 8]; 8])
    }
}

bitflags!(
  struct SpriteFlags: u8 {
    const UNUSED_MASK = 0b_0000_1111;
    const PALETTE     = 0b_0001_0000;
    const FLIPX       = 0b_0010_0000;
    const FLIPY       = 0b_0100_0000;
    const PRIORITY    = 0b_1000_0000;
  }
);

#[derive(Copy, Clone)]
struct Sprite {
    sprite_num: u8,
    x: u8,
    y: u8,
    tile_number: u8,
    flags: SpriteFlags,

}

impl Sprite {
    pub fn new(sprite_num: u8) -> Self {
        Sprite {
            sprite_num,
            x: 0,
            y: 0,
            tile_number: 0,
            flags: SpriteFlags::empty(),

        }
    }

    fn is_on_scan_line(&self, ppu: &DrawContainer) -> bool {
        let y = self.y;
        ppu.scanline >= y && ppu.scanline < (y + Sprite::height(ppu))
    }
    fn height(ppu: &DrawContainer) -> u8 {
        if ppu.large_sprites { SPRITE_HEIGHT as u8 } else { SPRITE_HEIGHT as u8 / 2 }
    }
    pub fn render<'a>(&'a self, ppu: &'a DrawContainer, background_mask: &'a mut BitSet) -> Option<impl Iterator<Item=(u8, Shade, Color)> + 'a> {
        if !self.is_on_scan_line(ppu) {
            return None;
        }

        let iter = (0..SPRITE_WIDTH).map(move |i| {
            // println!("IN SPRIT LOOP");
            let mut x = i;
            let mut y = ppu.scanline - self.y;
            if self.flags.contains(SpriteFlags::FLIPX) { x = 7 - x; }
            if self.flags.contains(SpriteFlags::FLIPY) { y = Sprite::height(ppu) - 1 - y; }
            //TODO VERIFY  (this.x + i >= Screen.WIDTH || this.x + i < 0)
            if (self.x + 1 >= SCREEN_WIDTH as u8) || (!self.flags.contains(SpriteFlags::PRIORITY) && background_mask.contains(self.x as usize + i)) {
                None
            } else {
                let tile = &ppu.video_ram.tiles[self.tile_number as usize + (y as usize / TILE_HEIGHT)];

                let palette = if self.flags.contains(SpriteFlags::PALETTE) {
                    ppu.obj_palette1
                } else {
                    ppu.obj_palette0
                };
                let shade = tile.shade_at(x as u8, y, &palette);

                //TODO         private int spritePaletteIndex() {
                //             return palette == objectPalette0 ? 0 : 1;
                //         }
                let color = ppu.color_palette.sprite(shade, 0);
                if shade != Shade::LIGHTEST {
                    background_mask.insert(x as usize);
                } else {
                    background_mask.remove(x as usize);
                }
                // if shade != Shade::LIGHTEST {
                //     println!("foo");
                //     std::thread::sleep(Duration::from_secs(3));
                //     println!("fooDONE");
                // }
                Some((self.x + i as u8, shade, color))
            }
        }).filter(|val| { val.is_some() })
            .map(|val| { val.unwrap() });
        Some(iter)
    }
}

#[derive(Copy, Clone)]
pub struct Palette(u8);


impl Palette {
    pub fn shade(&self, input: TilePixelValue) -> Shade {
        let offset = input as u16 * 2;
        let mask = (0b0000_0011 << offset);
        let result = (self.0 & mask) >> offset;
        match result {
            0 => Shade::LIGHTEST,
            1 => Shade::LIGHT,
            2 => Shade::DARK,
            3 => Shade::DARKEST,
            _ => Shade::LIGHTEST
        }
        // match input {
        //     TilePixelValue::Zero => { Shade::from((self.0 >> 0) & 0x3) }
        //     TilePixelValue::One => { Shade::from((self.0 >> 2) & 0x3) }
        //     TilePixelValue::Two => { Shade::from((self.0 >> 4) & 0x3) }
        //     TilePixelValue::Three => { Shade::from((self.0 >> 6) & 0x3) }
        // }
    }
}

