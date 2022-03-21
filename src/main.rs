use axi_gpio::{AxiGpio, AxiGpioChannel};
use getopts::{Options, Matches};
use num_traits::Num;
use std::env;
use std::fs::{OpenOptions};
use std::io::{Read, Write};
use std::str::FromStr;
use std::time::Duration;

mod uart16550;
use uart16550::AxiUart16550;
mod axi_gpio;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} PTY [options]", program);
    print!("{}", opts.usage(&brief));
}

fn opt_int<T: Num + FromStr>(matches: &Matches, nm: &str, default: T) -> Result<T, T::FromStrRadixErr> {
    match matches.opt_str(nm) {
        None => Ok(default),
        Some(s) => {
            s.trim().strip_prefix("0x").map_or(T::from_str_radix(&s, 10), |hex_str| T::from_str_radix(hex_str, 16))
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("", "reg_device", "Register access device path", "REG_DEVICE");
    opts.optflag("x", "xrt", "XRT access mode");
    opts.optflag("r", "reset", "assert debug reset");
    opts.optflag("h", "help", "print help");
    opts.optopt("", "uart_core_frequency_hz", "Target UART core frequency to calculate baud rate register value.", "UART_CORE_FREQUENCY_HZ");
    opts.optopt("b", "uart_baud", "UART baud rate. default is 115200", "UART_BAUD");
    opts.optopt("", "uart_base_address", "UART register base address. default is 0x1000", "UART_BASE_ADDRESS");
    opts.optopt("", "gpio_base_address", "GPIO register base address. default is 0x0000", "GPIO_BASE_ADDRESS");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => { panic!("{}", f.to_string()) },
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let xrt_mode = matches.opt_present("xrt");
    let default_device = if xrt_mode { String::from("/dev/dri/renderD128") } else { String::from("/dev/xdma0_user") };
    let reg_device = matches.opt_str("reg_device").unwrap_or(default_device);
    let uart_core_frequency = matches.opt_str("uart_core_frequency_hz").unwrap_or(String::from("125000000")).parse().expect("Invalid frequency");
    let uart_baud = matches.opt_str("uart_baud").unwrap_or(String::from("115200")).parse().expect("Invalid baud rate");
    let uart_base_address = opt_int(&matches, "uart_base_address", 0x1000).expect("Invalid UART base address.");
    let gpio_base_address = opt_int(&matches, "gpio_base_address", 0x0000).expect("Invalid GPIO base address.");
    let pty = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print_usage(&program, opts);
        return;
    };
    let (uart_offset, gpio_offset, mmap_size) = if xrt_mode {
        (uart_base_address, gpio_base_address, Some(32*1024*1024usize))
    } else {
        (uart_base_address, gpio_base_address, None)
    };
    let device_file = OpenOptions::new().read(true).write(true).open(reg_device).expect("Failed to open register access device");

    let port = serialport::new(pty, uart_baud).open().expect("Failed to open port");
    let uart = AxiUart16550::new(&device_file, uart_offset, mmap_size.clone())
        .expect("Failed to map register.");
    let uart = uart.initialize(uart_core_frequency, 115200)
        .expect("Failed to initialize AXI UART.");
    let uart = std::sync::Arc::new(std::sync::Mutex::new(uart));
    
    if matches.opt_present("r") {
        // Pulse debug reset pin.
        println!("Resetting... GPIO: {:016X}", gpio_offset);
        let mut gpio = AxiGpio::new(&device_file, gpio_offset, mmap_size).expect("Failed to map GPIO");
        gpio.change_bits(AxiGpioChannel::Ch2, 1, 0);
        gpio.set_output(AxiGpioChannel::Ch2, 1);
        gpio.change_bits(AxiGpioChannel::Ch2, 0, 1);
    }

    let mut write_port = port.try_clone().expect("Failed to clone the port");
    let mut read_port = port;
    let read_uart = uart.clone();
    let read_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 2048];
        loop {
            if let Ok(count) = read_uart.lock().unwrap().read(&mut buf) {
                //println!("read: {}", count);
                write_port.write_all(&buf[..count]).ok();
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let write_uart = uart;
    let write_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 2048];
        loop {
            if let Ok(count) = read_port.read(&mut buf) {
                //println!("write: {}", count);
                let mut p = &buf[..count];
                while p.len() > 0 { 
                    if let Ok(count) = write_uart.lock().unwrap().write(p) {
                        p = &p[count..];
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });

    read_thread.join().unwrap();
    write_thread.join().unwrap();
}
