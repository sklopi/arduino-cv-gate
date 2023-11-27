#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]


use core::cell;
use panic_halt as _;
use mcp4725::{MCP4725, PowerDown};

const PRESCALER: u32 = 1024;
const TIMER_COUNTS: u32 = 125;

const MILLIS_INCREMENT: u32 = PRESCALER * TIMER_COUNTS / 16000;

static MILLIS_COUNTER: avr_device::interrupt::Mutex<cell::Cell<u32>> =
    avr_device::interrupt::Mutex::new(cell::Cell::new(0));

fn millis_init(tc0: arduino_hal::pac::TC0) {
    // Configure the timer for the above interval (in CTC mode)
    // and enable its interrupt.
    tc0.tccr0a.write(|w| w.wgm0().ctc());
    tc0.ocr0a.write(|w| w.bits(TIMER_COUNTS as u8));
    tc0.tccr0b.write(|w| match PRESCALER {
        8 => w.cs0().prescale_8(),
        64 => w.cs0().prescale_64(),
        256 => w.cs0().prescale_256(),
        1024 => w.cs0().prescale_1024(),
        _ => panic!(),
    });
    tc0.timsk0.write(|w| w.ocie0a().set_bit());

    // Reset the global millisecond counter
    avr_device::interrupt::free(|cs| {
        MILLIS_COUNTER.borrow(cs).set(0);
    });
}

#[avr_device::interrupt(atmega328p)]
fn TIMER0_COMPA() {
    avr_device::interrupt::free(|cs| {
        let counter_cell = MILLIS_COUNTER.borrow(cs);
        let counter = counter_cell.get();
        counter_cell.set(counter + MILLIS_INCREMENT);
    })
}

fn millis() -> u32 {
    avr_device::interrupt::free(|cs| MILLIS_COUNTER.borrow(cs).get())
}

// ----------------------------------------------------------------------------

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    millis_init(dp.TC0);

    // Enable interrupts globally
    unsafe { avr_device::interrupt::enable() };

    let i2c = arduino_hal::I2c::new(
        dp.TWI,
        pins.a4.into_pull_up_input(),
        pins.a5.into_pull_up_input(),
        50000,
    );

    // Configure the MCP4725 DAC
    let mut dac = MCP4725::new(i2c, 0b010);
    dac.wake_up().unwrap();

    let delta = 200;

    let ms_in_a_minute = 60 * 1000;

    let ms_between_hits = ms_in_a_minute / 128;

    // Wait for a character and print current time once it is received
    let mut time;
    let mut start_last_gate = 0;

    // Reset the chip to set the output low again
    dac.reset().unwrap();

    loop {
        time = millis();
        if time > start_last_gate + ms_between_hits {
            dac.set_dac_fast(PowerDown::Normal, 0xffff).unwrap();
            start_last_gate = time;
        }

        if time > start_last_gate + delta {
            dac.set_dac_fast(PowerDown::Normal, 0x0000).unwrap();
        }
    }
}