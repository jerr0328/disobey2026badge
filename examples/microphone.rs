//! VU meter: reads the I2S microphone as fast as possible and displays
//! peak amplitude on both LED bars (green → yellow → red).

#![no_std]
#![no_main]

#[allow(clippy::wildcard_imports)]
use disobey2026badge::*;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{dma::DmaDescriptor, timer::timg::TimerGroup};
use esp_println as _;
use palette::Srgb;

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

/// VU meter colors from bottom (quiet) to top (loud).
const VU_COLORS: [Srgb<u8>; BAR_COUNT] = [
    Srgb::new(0, 20, 0),  // green
    Srgb::new(0, 20, 0),  // green
    Srgb::new(20, 20, 0), // yellow
    Srgb::new(20, 10, 0), // orange
    Srgb::new(20, 0, 0),  // red
];

const OFF: Srgb<u8> = Srgb::new(0, 0, 0);

/// Maximum expected amplitude from the mic (tuning knob — adjust to taste).
const MAX_AMPLITUDE: u16 = 4000;

/// Map a peak amplitude (0..MAX_AMPLITUDE) to a LED bar pattern.
fn amplitude_to_bar(peak: u16) -> [Srgb<u8>; BAR_COUNT] {
    let level = if peak >= MAX_AMPLITUDE {
        BAR_COUNT
    } else {
        (peak as usize * BAR_COUNT) / MAX_AMPLITUDE as usize
    };

    let mut bar = [OFF; BAR_COUNT];
    for (i, led) in bar.iter_mut().enumerate() {
        if i < level {
            *led = VU_COLORS[i];
        }
    }
    bar
}

#[embassy_executor::task]
async fn vu_task(
    mic: &'static mut microphone::Microphone<'static>,
    leds: &'static mut Leds<'static>,
) {
    let mut buf = [0i16; 512];

    loop {
        match mic.rx.read_words(&mut buf) {
            Ok(()) => {
                let peak = buf.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
                let bar = amplitude_to_bar(peak);
                leds.set_both_bars(&bar);
                leds.update().await;
            }
            Err(_) => {
                // On error, briefly pause to avoid tight-looping
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let peripherals = disobey2026badge::init();
    let resources = split_resources!(peripherals);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let descriptors = mk_static!([DmaDescriptor; 8], [DmaDescriptor::EMPTY; 8]);

    let mic = mk_static!(
        microphone::Microphone<'static>,
        microphone::Microphone::new(resources.mic, microphone::DEFAULT_SAMPLE_RATE, descriptors)
    );

    let leds = mk_static!(Leds<'static>, resources.leds.into());

    spawner.must_spawn(vu_task(mic, leds));

    loop {
        Timer::after(Duration::from_secs(600)).await;
    }
}
