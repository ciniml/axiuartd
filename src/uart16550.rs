use memmap::{MmapMut, MmapOptions};
use volatile::Volatile;
use volatile::access::{ReadOnly, WriteOnly, ReadWrite};
use std::fs::File;
use std::io::{Read, Write};
use std::marker::PhantomData;


struct AxiUart16550Regs {
    rbr: Volatile<&'static u32, ReadOnly>,
    thr: Volatile<&'static mut u32, WriteOnly>,
    fcr: Volatile<&'static mut u32, ReadWrite>,
    lcr: Volatile<&'static mut u32, ReadWrite>,
    mcr: Volatile<&'static mut u32, ReadWrite>,
    lsr: Volatile<&'static mut u32, ReadWrite>,
    msr: Volatile<&'static mut u32, ReadWrite>,
    #[allow(unused)]
    scr: Volatile<&'static mut u32, ReadWrite>,
    dll: Volatile<&'static mut u32, ReadWrite>,
    dlm: Volatile<&'static mut u32, ReadWrite>,
}

pub trait State {}
pub struct Uninitialized {}
impl State for Uninitialized {}
pub struct Initialized{}
impl State for Initialized {}

pub struct AxiUart16550<S: State> {
    mmap: MmapMut,
    regs: AxiUart16550Regs,
    state: PhantomData<S>,
}

impl AxiUart16550<Uninitialized> {
    pub fn new(file: &File, offset: usize, map_size: Option<usize>) -> std::io::Result<Self> {
        let (mmap_offset, mmap_length, reg_offset) = match map_size {
            Some(map_size) => (0, map_size, offset),   // Some devices like XRT DRI user register requires mmapped with a specific BAR size, thus we have to mmap with `map_size` length if map_size is specified with zero offset.
            None => (offset + 0x1000, page_size::get(), 0),
        };
        let mut mmap = unsafe { MmapOptions::new().offset(mmap_offset as u64).len(mmap_length).map_mut(&file)? };
        let regs = AxiUart16550Regs::new(&mut mmap, reg_offset);
        Ok(Self {
            mmap,
            regs,
            state: PhantomData,
        })
    }
    pub fn initialize(mut self, core_freq_hz: u32, baud_rate_hz: u32) -> std::io::Result<AxiUart16550<Initialized>> {
        self.regs.lcr.write(1 << 7);    // DLAB
        let divisor = core_freq_hz / (16 * baud_rate_hz);
        self.regs.dll.write(divisor & 0xff);
        self.regs.dlm.write(divisor >> 8);
        self.regs.lcr.write(0b00000011); // Data bits = 8
        self.regs.fcr.write(0b00000110); // Purge FIFO
        self.regs.fcr.write(0b10000001); // FIFO enable
        self.regs.dump_status();
        Ok(AxiUart16550::<Initialized>{
            mmap: self.mmap,
            regs: self.regs,
            state: PhantomData,
        })
    }
}
impl AxiUart16550<Initialized> {
    fn rx_ready(&self) -> bool {
        self.regs.lsr.read() & (1 << 0) != 0
    }
    fn tx_ready(&self) -> bool {
        self.regs.lsr.read() & (1 << 5) != 0
    }
}
impl Read for AxiUart16550<Initialized> {

    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.rx_ready() {
            return Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "RX not ready"));
        }
        let mut count = 0;
        while self.rx_ready() && count < buf.len() {
            buf[count] = (self.regs.rbr.read() & 0xff) as u8;
            count += 1;
        }
        Ok(count)
    }
}
impl Write for AxiUart16550<Initialized> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !self.tx_ready() {
            return Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "TX not ready"));
        }
        let mut count = 0;
        while self.tx_ready() && count < buf.len() {
            self.regs.thr.write(buf[count] as u32);
            count += 1;
        }
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.regs.lsr.read() & (1 << 6) != 0 {
            std::io::Result::Ok(())
        } else {
            std::io::Result::Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "TX FIFO not empty"))
        }
    }
}

unsafe fn make_volatile_readonly(mmap: &mut MmapMut, offset: usize) -> Volatile<&'static u32, ReadOnly> {
    Volatile::new_read_only((mmap[offset..offset+4].as_ptr() as *const u32).as_ref().unwrap())
}
unsafe fn make_volatile_writeonly(mmap: &mut MmapMut, offset: usize) -> Volatile<&'static mut u32, WriteOnly> {
    Volatile::new_write_only((mmap[offset..offset+4].as_ptr() as *mut u32).as_mut().unwrap())
}
unsafe fn make_volatile_readwrite(mmap: &mut MmapMut, offset: usize) -> Volatile<&'static mut u32, ReadWrite> {
    Volatile::new((mmap[offset..offset+4].as_ptr() as *mut u32).as_mut().unwrap())
}


impl AxiUart16550Regs {
    fn new(mmap: &mut MmapMut, offset: usize) -> Self {
        unsafe {
            Self {
                rbr: make_volatile_readonly(mmap, offset + 0x0),
                thr: make_volatile_writeonly(mmap,offset + 0x0),
                fcr: make_volatile_readwrite(mmap,offset + 0x8),
                lcr: make_volatile_readwrite(mmap,offset + 0xc),
                mcr: make_volatile_readwrite(mmap,offset + 0x10),
                lsr: make_volatile_readwrite(mmap,offset + 0x14),
                msr: make_volatile_readwrite(mmap,offset + 0x18),
                scr: make_volatile_readwrite(mmap,offset + 0x1c),
                dll: make_volatile_readwrite(mmap,offset + 0x0),
                dlm: make_volatile_readwrite(mmap,offset + 0x4),
            }
        }
    }
    fn dump_status(&self) {
        println!(
            "fcr: {:08X}, lcr: {:08X}, mcr: {:08X}, lsr: {:08X}, msr: {:08X}", 
            self.fcr.read(),
            self.lcr.read(),
            self.mcr.read(),
            self.lsr.read(),
            self.msr.read(),
        );
    }
}
