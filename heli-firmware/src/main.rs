//! Testing PWM output for pre-defined pin combination: all pins for default mapping

#![deny(unsafe_code)]
#![allow(clippy::empty_loop)]
#![allow(clippy::approx_constant)]
#![no_main]
#![no_std]

use panic_halt as _;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use stm32f1xx_hal::afio::AfioExt;
use stm32f1xx_hal::flash::FlashExt;
use stm32f1xx_hal::gpio::GpioExt;
use stm32f1xx_hal::i2c::{BlockingI2c, DutyCycle};
use stm32f1xx_hal::{i2c, pac, spi};
use stm32f1xx_hal::prelude::{_embedded_hal_blocking_spi_Write, _fugit_RateExtU32};
use stm32f1xx_hal::rcc::RccExt;
use mpu6050_dmp::{sensor::Mpu6050, address::Address};
use mpu6050_dmp::euler::Euler;
use mpu6050_dmp::quaternion::Quaternion;
use stm32f1xx_hal::spi::{Phase, Polarity, Spi};
use stm32f1xx_hal::timer::{Channel, SysTimerExt, Tim2NoRemap, Timer};
use embedded_nrf24l01::{NRF24L01, Configuration, DataRate, CrcMode};

const PI: f32 = 3.1415927;
const PI2_INVERTED: f32 = 1.0 / (2.0 * PI);

const SPI_MODE: spi::Mode = spi::Mode {
    phase: Phase::CaptureOnFirstTransition,
    polarity: Polarity::IdleLow,
};

#[entry]
fn main() -> ! {
    hprintln!("Startup...");
    let core_peripherals = cortex_m::Peripherals::take().unwrap();
    let device_peripherals = pac::Peripherals::take().unwrap();

    let mut flash = device_peripherals.FLASH.constrain();
    let rcc = device_peripherals.RCC.constrain();
    let mut afio = device_peripherals.AFIO.constrain();

    let clock = rcc.cfgr.use_hse(8.MHz()).sysclk(48.MHz()).pclk1(6.MHz()).freeze(&mut flash.acr);

    let mut gpioa = device_peripherals.GPIOA.split();
    let mut gpiob = device_peripherals.GPIOB.split();

    let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    let pwm_pin_pa0 = gpioa.pa0.into_alternate_push_pull(&mut gpioa.crl);
    let pwm_pin_pa1 = gpioa.pa1.into_alternate_push_pull(&mut gpioa.crl);

    let spi_sck_pin = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let spi_miso_pin = gpioa.pa6;
    let spi_mosi_pin = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let spi_cs_pin = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);

    let ce_pin = gpioa.pa9.into_push_pull_output(&mut gpioa.crh);

    let mut spi = Spi::spi1(device_peripherals.SPI1, (spi_sck_pin, spi_miso_pin, spi_mosi_pin), &mut afio.mapr, SPI_MODE, 1.MHz(), clock);

    let mut pwm = Timer::new(device_peripherals.TIM2, &clock).pwm_hz::<Tim2NoRemap, _, _>((pwm_pin_pa0, pwm_pin_pa1), &mut afio.mapr, 1.kHz());
    pwm.enable(Channel::C1);
    pwm.enable(Channel::C2);
    pwm.set_period(50.kHz());
    let max = pwm.get_max_duty();

    let (mut pwm1, mut pwm2) = pwm.split();
    pwm1.set_duty(max / 2);
    pwm2.set_duty(max / 4);

    let i2c = BlockingI2c::i2c1(
        device_peripherals.I2C1,
        (scl, sda),
        &mut afio.mapr,
        i2c::Mode::Fast {
            frequency: 400.kHz(),
            duty_cycle: DutyCycle::Ratio16to9,
        },
        clock,
        1000,
        10,
        1000,
        1000,
    );
    hprintln!("Pins configured.");


    let mut sensor = Mpu6050::new(i2c, Address::default()).unwrap();

    hprintln!("Sensor object created.");
    let mut delay = core_peripherals.SYST.delay(&clock);
    sensor.initialize_dmp(&mut delay).unwrap();
    sensor.enable_dmp().unwrap();
    hprintln!("Sensor configured.");

    spi.write("test".as_bytes()).unwrap();
    hprintln!("SPI WRIITTEN");

    let nrf24_result = NRF24L01::new(ce_pin, spi_cs_pin, spi);
    hprintln!("NO PANIC DIGGA");
    let nrf24 = nrf24_result.unwrap();
    hprintln!("Transceiver object created.");
    let mut nrf24_rx = nrf24.rx().unwrap();
    nrf24_rx.set_frequency(0).unwrap();
    nrf24_rx.set_rf(&DataRate::R2Mbps, 0).unwrap();
    nrf24_rx.set_crc(CrcMode::TwoBytes).unwrap();

    hprintln!("---- Initialized ----");


    loop {
        let len = sensor.get_fifo_count().unwrap();
        if len >= 28 {
            let mut b: [u8; 28] = [0; 28];
            let buf = sensor.read_fifo(&mut b).unwrap();
            let q = Quaternion::from_bytes(&buf[..16]).unwrap().normalize();
            let euler = Euler::from(q);

            let a = (euler.phi as f32 + PI) * PI2_INVERTED * max as f32;
            let b = (euler.psi as f32 + PI) * PI2_INVERTED * max as f32;
            pwm1.set_duty(a as u16);
            pwm2.set_duty(b as u16);
        }
    }
}