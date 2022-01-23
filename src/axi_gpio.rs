use memmap::{MmapMut, MmapOptions};
use volatile::Volatile;
use volatile::access::{ReadOnly, WriteOnly, ReadWrite};
use std::fs::File;

struct AxiGpioRegs {
    data: Volatile<&'static mut u32, ReadWrite>,
    tri: Volatile<&'static mut u32, ReadWrite>,
    data2: Volatile<&'static mut u32, ReadWrite>,
    tri2: Volatile<&'static mut u32, ReadWrite>,
}

#[allow(unused)]
pub struct AxiGpio {
    mmap: MmapMut,
    regs: AxiGpioRegs,
}

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub enum AxiGpioChannel {
    Ch1,
    Ch2,
}

impl AxiGpio {
    pub fn new(file: &File, offset: usize) -> std::io::Result<Self> {
        let mut mmap = unsafe { MmapOptions::new().offset((offset + 0x1000) as u64).len(page_size::get()).map_mut(&file)? };
        let regs = AxiGpioRegs::new(&mut mmap);
        Ok(Self {
            mmap,
            regs,
        })
    }

    pub fn read(&self, ch: AxiGpioChannel) -> u32 {
        match ch {
            AxiGpioChannel::Ch1 => self.regs.data.read(),
            AxiGpioChannel::Ch2 => self.regs.data2.read(),
        }
    }
    pub fn write(&mut self, ch: AxiGpioChannel, value: u32) {
        match ch {
            AxiGpioChannel::Ch1 => self.regs.data.write(value),
            AxiGpioChannel::Ch2 => self.regs.data2.write(value),
        }
    }
    fn tristate(&self, ch: AxiGpioChannel) -> u32 {
        match ch {
            AxiGpioChannel::Ch1 => self.regs.tri.read(),
            AxiGpioChannel::Ch2 => self.regs.tri.read(),
        }
    }
    fn set_tristate(&mut self, ch: AxiGpioChannel, value: u32) {
        match ch {
            AxiGpioChannel::Ch1 => self.regs.tri.write(value),
            AxiGpioChannel::Ch2 => self.regs.tri.write(value),
        }
    }
    pub fn change_bits(&mut self, ch: AxiGpioChannel, set_bits: u32, clear_bits: u32) {
        let value = self.read(ch);
        self.write(ch, (value & !clear_bits) | set_bits);
    }
    #[allow(unused)]
    pub fn set_input(&mut self, ch: AxiGpioChannel, bits: u32) {
        let tri = self.tristate(ch);
        self.set_tristate(ch, tri | bits);
    }
    pub fn set_output(&mut self, ch: AxiGpioChannel, bits: u32) {
        let tri = self.tristate(ch);
        self.set_tristate(ch, tri & !bits);
    }
}

#[allow(unused)]
unsafe fn make_volatile_readonly(mmap: &mut MmapMut, offset: usize) -> Volatile<&'static u32, ReadOnly> {
    Volatile::new_read_only((mmap[offset..offset+4].as_ptr() as *const u32).as_ref().unwrap())
}
#[allow(unused)]
unsafe fn make_volatile_writeonly(mmap: &mut MmapMut, offset: usize) -> Volatile<&'static mut u32, WriteOnly> {
    Volatile::new_write_only((mmap[offset..offset+4].as_ptr() as *mut u32).as_mut().unwrap())
}
unsafe fn make_volatile_readwrite(mmap: &mut MmapMut, offset: usize) -> Volatile<&'static mut u32, ReadWrite> {
    Volatile::new((mmap[offset..offset+4].as_ptr() as *mut u32).as_mut().unwrap())
}


impl AxiGpioRegs {
    fn new(mmap: &mut MmapMut) -> Self {
        unsafe {
            Self {
                data: make_volatile_readwrite(mmap, 0x0),
                tri: make_volatile_readwrite(mmap, 0x4),
                data2: make_volatile_readwrite(mmap, 0x8),
                tri2: make_volatile_readwrite(mmap, 0xc),
            }
        }
    }
    #[allow(unused)]
    fn dump_status(&self) {
        println!(
            "data: {:08X}, tri: {:08X}, data2: {:08X}, tri2: {:08X}", 
            self.data.read(),
            self.tri.read(),
            self.data2.read(),
            self.tri2.read(),
        );
    }
}
