//! Testing PWM output for pre-defined pin combination: all pins for default mapping

#![deny(unsafe_code)]
#![allow(clippy::empty_loop)]
#![allow(clippy::approx_constant)]
#![no_main]
#![no_std]

use protocol::JoystickInput;
use panic_halt as _;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use stm32f1xx_hal::afio::AfioExt;
use stm32f1xx_hal::flash::FlashExt;
use stm32f1xx_hal::gpio::GpioExt;
use stm32f1xx_hal::i2c::{BlockingI2c, DutyCycle};
use stm32f1xx_hal::{i2c, pac, spi};
use stm32f1xx_hal::prelude::_fugit_RateExtU32;
use stm32f1xx_hal::rcc::RccExt;
use mpu6050_dmp::{sensor::Mpu6050, address::Address};
use stm32f1xx_hal::spi::{Phase, Polarity, Spi};
use stm32f1xx_hal::timer::{Channel, SysTimerExt, Tim2NoRemap, Timer};
use embedded_nrf24l01::{NRF24L01, Configuration, DataRate, CrcMode, Device};

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

    let spi = Spi::spi1(device_peripherals.SPI1, (spi_sck_pin, spi_miso_pin, spi_mosi_pin), &mut afio.mapr, SPI_MODE, 1.MHz(), clock);

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
    let mut nrf24 = NRF24L01::new(ce_pin, spi_cs_pin, spi).unwrap();
    configure_nrf24(&mut nrf24).unwrap();
    hprintln!("Transceiver object created.");
    let mut nrf24_rx = nrf24.rx().unwrap();
    hprintln!("---- Initialized ----");


    loop {
        if let Ok(Some(0)) = nrf24_rx.can_read() {
            if let Ok(payload) = nrf24_rx.read() {
                if let Ok((input, _)) = postcard::take_from_bytes::<JoystickInput>(&*payload) {
                    hprintln!("{}", input.get_pitch());
                    pwm1.set_duty(input.get_pitch() / 65535 * max);
                }


            }
        }
    }
}

fn configure_nrf24<T: Configuration>(nrf24: &mut T) -> Result<(), <<T as Configuration>::Inner as Device>::Error> {
    nrf24.set_frequency(8)?;
    nrf24.set_rf(&DataRate::R2Mbps, 3)?;
    nrf24.set_crc(CrcMode::OneByte)?;
    nrf24.set_rx_addr(0, b"heli")?;
    nrf24.set_auto_retransmit(0, 0)?;
    nrf24.set_auto_ack(&[true; 6])?;
    nrf24.set_pipes_rx_enable(&[true, false, false, false, false, false])?;
    nrf24.set_pipes_rx_lengths(&[None; 6])?;
    Ok(())
}