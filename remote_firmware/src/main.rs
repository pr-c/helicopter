#![deny(unsafe_code)]
#![allow(clippy::empty_loop)]
#![no_main]
#![no_std]

use panic_halt as _;

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use embedded_nrf24l01::{NRF24L01, Configuration, DataRate, CrcMode, Device};
use nb::block;
use stm32f1xx_hal::{pac, flash::FlashExt, prelude::*, adc, spi::{self, *}};
use stm32f1xx_hal::timer::Timer;
use protocol::JoystickInput;

const SPI_MODE: spi::Mode = spi::Mode {
    phase: Phase::CaptureOnFirstTransition,
    polarity: Polarity::IdleLow,
};

#[entry]
fn main() -> ! {
    hprintln!("Startup REMOTE...");
    let core_peripherals = cortex_m::Peripherals::take().unwrap();
    let device_peripherals = pac::Peripherals::take().unwrap();

    let mut flash = device_peripherals.FLASH.constrain();
    let rcc = device_peripherals.RCC.constrain();
    let mut afio = device_peripherals.AFIO.constrain();

    let clock = rcc.cfgr.freeze(&mut flash.acr);

    let mut gpioa = device_peripherals.GPIOA.split();
    let mut gpiob = device_peripherals.GPIOB.split();

    let mut adc = adc::Adc::adc1(device_peripherals.ADC1, clock);
    let mut ch0 = gpiob.pb0.into_analog(&mut gpiob.crl);

    let mut timer = Timer::syst(core_peripherals.SYST, &clock).counter_hz();
    timer.start(60.Hz()).unwrap();


    let spi_sck_pin = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let spi_miso_pin = gpioa.pa6;
    let spi_mosi_pin = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let spi_cs_pin = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);

    let ce_pin = gpioa.pa9.into_push_pull_output(&mut gpioa.crh);

    let spi = Spi::spi1(device_peripherals.SPI1, (spi_sck_pin, spi_miso_pin, spi_mosi_pin), &mut afio.mapr, SPI_MODE, 1.MHz(), clock);
    hprintln!("SPI");
    let mut nrf24 = NRF24L01::new(ce_pin, spi_cs_pin, spi).unwrap();
    hprintln!("Transceiver object created.");
    configure_nrf24(&mut nrf24).unwrap();
    hprintln!("Configured");
    let mut nrf24_tx = nrf24.tx().unwrap();
    hprintln!("Into TX");

    let mut binary = [0u8, 32];
    loop {
        block!(timer.wait()).unwrap();
        let pitch: u16 = adc.read(&mut ch0).unwrap();
        let object = JoystickInput::new(pitch);

        postcard::to_slice(&object, &mut binary).unwrap();
        let _ = nrf24_tx.send(&binary);

    }
}

fn configure_nrf24<T: Configuration>(nrf24: &mut T) -> Result<(), <<T as Configuration>::Inner as Device>::Error> {
    nrf24.set_frequency(8)?;
    nrf24.set_rf(&DataRate::R250Kbps, 0)?;
    nrf24.set_crc(CrcMode::Disabled)?;
    nrf24.set_tx_addr(&b"fnord"[..])?;
    nrf24.set_auto_retransmit(0, 0)?;
    nrf24.set_auto_ack(&[false; 6])?;
    nrf24.set_pipes_rx_lengths(&[None; 6])?;
    Ok(())
}