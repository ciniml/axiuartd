use axi_gpio::{AxiGpio, AxiGpioChannel};
use getopts::Options;
use std::env;
use std::fs::{OpenOptions};
use std::io::{Read, Write};
use std::time::Duration;

mod uart16550;
use uart16550::AxiUart16550;
mod axi_gpio;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} PTY [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("", "xdma_user", "XDMA User device path", "XDMA_USER");
    opts.optflag("r", "reset", "assert debug reset");
    opts.optflag("h", "help", "print help");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => { panic!("{}", f.to_string()) },
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }

    let xdma_user = matches.opt_str("xdma_user").unwrap_or(String::from("/dev/xdma0_user"));
    let pty = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print_usage(&program, opts);
        return;
    };
    let device_file = OpenOptions::new().read(true).write(true).open(xdma_user).expect("Failed to open XDMA device");

    let port = serialport::new(pty, 9600).open().expect("Failed to open port");
    let uart = AxiUart16550::new(&device_file, 0x10000)
        .expect("Failed to map XDMA register.");
    let uart = uart.initialize(125000000, 115200)
        .expect("Failed to initialize AXI UART.");
    let uart = std::sync::Arc::new(std::sync::Mutex::new(uart));
    
    if matches.opt_present("r") {
        // Pulse debug reset pin.
        let mut gpio = AxiGpio::new(&device_file, 0x0).expect("Failed to map GPIO");
        gpio.change_bits(AxiGpioChannel::Ch2, 1, 0);
        gpio.set_output(AxiGpioChannel::Ch2, 1);
        gpio.change_bits(AxiGpioChannel::Ch2, 0, 0);
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
