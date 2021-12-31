#![no_std]
#![no_main]

use panic_halt as _;

use crate::rgb_controller::RgbController;
use core::fmt::Write;
use cortex_m::peripheral::DWT;
use rtic::cyccnt::U32Ext;

use stm32l4xx_hal::{
    self,
    gpio::{Edge, Input, Output, PullDown, PushPull},
    gpio::{PB1, PB13, PB6},
    interrupt,
    pac::{TIM1, USART2},
    prelude::*,
    pwm::Pwm,
    pwm::{C1, C2, C3},
    serial,
    serial::{Config, Serial},
    time::{Hertz, Instant, MonoTimer},
};

const SYSTEM_CLOCK: u32 = 80_000_000;

// Speed of Sound in cm/ms @ standard temperature/pressure, non-adjusted.
const SPEED_OF_SOUND: f32 = 34.3;

fn measured_range(echo_length_ms: f32) -> f32 {
    echo_length_ms * SPEED_OF_SOUND / 2.0
}

pub mod rgb_controller {
    use embedded_hal::PwmPin;
    pub struct RgbController<R, G, B> {
        red: R,
        green: G,
        blue: B,
    }

    impl<R: PwmPin, G: PwmPin, B: PwmPin>  embedded_hal::PwmPin for RgbController<R,G,B> {
        type Duty = (R::Duty, G::Duty, B::Duty);
        fn disable(&mut self) {
            self.red.disable();
            self.green.disable();
            self.blue.disable();
        }
        fn enable(&mut self) {
            self.red.enable();
            self.green.enable();
            self.blue.enable();
        }
        fn get_duty(&self) -> Self::Duty {
            (self.red.get_duty(),self.green.get_duty(), self.blue.get_duty())
        }
        fn get_max_duty(&self) -> Self::Duty {
            (self.red.get_max_duty(),self.green.get_max_duty(), self.blue.get_max_duty())

        }
        fn set_duty(&mut self, duty: Self::Duty) {
            let (r,g,b) = duty;
            self.red.set_duty(r);
            self.green.set_duty(g);
            self.blue.set_duty(b);
        }

    }

    impl<T, R: PwmPin<Duty = T>, G: PwmPin<Duty = T>, B: PwmPin<Duty = T>> RgbController<R, G, B> {
        pub fn new(rgb_pwm:(R,G,B)) -> RgbController<R, G, B> {
            RgbController { red: rgb_pwm.0, green: rgb_pwm.1, blue: rgb_pwm.2 }
        }

        pub fn set_color_rgb(&mut self, red_level: T, green_level: T, blue_level: T) {
            self.set_duty((red_level,green_level,blue_level));
        }
    }
}

#[rtic::app(device = stm32l4xx_hal::stm32,peripherals=true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        rx: serial::Rx<USART2>,
        tx: serial::Tx<USART2>,
        led: PB13<Output<PushPull>>,
        ping_pong_pin: PB1<Output<PushPull>>,
        echo: PB6<Input<PullDown>>,
        duration_timer: MonoTimer,
        range: f32,
        light_controller: RgbController<Pwm<TIM1, C1>, Pwm<TIM1, C2>, Pwm<TIM1, C3>>,
    }

    #[init(schedule = [heartbeat, print_status, ping, disco_mode])]
    fn init(mut cx: init::Context) -> init::LateResources {
        let mut dp = cx.device;

        // Prevent instibility on sleep with Probe-run
        dp.DBGMCU.cr.modify(|_, w| {
            w.dbg_sleep().set_bit();
            w.dbg_standby().set_bit();
            w.dbg_stop().set_bit()
        });

        // set up cycle-count
        cx.core.DCB.enable_trace();
        DWT::unlock();
        cx.core.DWT.enable_cycle_counter();

        let mut rcc = dp.RCC.constrain();
        let mut flash = dp.FLASH.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc
            .cfgr
            .sysclk(SYSTEM_CLOCK.hz())
            .hclk(SYSTEM_CLOCK.hz())
            .freeze(&mut flash.acr, &mut pwr);

        //General Purpose Duration Timer
        let duration_timer = MonoTimer::new(cx.core.DWT, clocks);

        // GPIO Bank Initialization
        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);

        // General Purpose/Heart-beat LED
        let led = gpiob
            .pb13
            .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);

        // Serial Communication with Virtual Comm Port USART 2
        let baudrate = 38_400.bps();

        let tx = gpioa.pa2.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
        let rx = gpioa.pa3.into_af7(&mut gpioa.moder, &mut gpioa.afrl);

        let (tx, rx) = Serial::usart2(
            dp.USART2,
            (tx, rx),
            Config::default().baudrate(baudrate),
            clocks,
            &mut rcc.apb1r1,
        )
        .split();

        //RGB Light Controller;
        //Timer 1 - 16-bit timer.  Channels 1,2,3
        const RGB_LED_PWM_FREQUENCY: u32 = 1_u32; // kHz

        // TIM1 CH1
        let red_pin = gpioa
            .pa8
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrh);

        // TIM1 CH2
        let green_pin = gpioa
            .pa9
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrh);

        // TIM1 CH3
        let blue_pin = gpioa
            .pa10
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrh);

        // TIM1 PWM Initialization.
        let rgb_pwms = dp.TIM1.pwm(
            (red_pin, green_pin, blue_pin),
            RGB_LED_PWM_FREQUENCY.khz(),
            clocks,
            &mut rcc.apb2,
        );

        let mut light_controller = RgbController::new(rgb_pwms);
        light_controller.enable();
        light_controller.set_duty((u16::MAX, 0, 0));

        
        // Design Decision:  PA1/PA0 assigned to drive servo motors.
        // TIM2 CH3/CH4 conflict with Usart2 TX/RX pin assignments on PA2/PA3,
        // and for this reason may not be used.  The servo controls require only
        // two channels.

        // TIM2 CH1
        let left_wheel_pin = gpioa
            .pa0
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrl);

        // TIM2 CH2
        let right_wheel_pin = gpioa
            .pa1
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrl);

        let (_left_wheel, _right_wheel) = dp.TIM2.pwm(
            (left_wheel_pin, right_wheel_pin),
            RGB_LED_PWM_FREQUENCY.hz(),
            clocks,
            &mut rcc.apb1r1,
        );

        // Range Finder

        // we need an edge-triggered interrupt that measures how long it was held high.
        let mut echo = gpiob
            .pb6
            .into_pull_down_input(&mut gpiob.moder, &mut gpiob.pupdr);
        echo.make_interrupt_source(&mut dp.SYSCFG, &mut rcc.apb2);
        echo.trigger_on_edge(&mut dp.EXTI, Edge::RISING_FALLING);
        echo.enable_interrupt(&mut dp.EXTI);

        // and we need a pin to trigger the ping, pulse 10us every 60ms
        let mut ping_pong_pin = gpiob
            .pb1
            .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);

        ping_pong_pin.set_low().unwrap();

        rtic::pend(interrupt::EXTI9_5);

        // Scheduled Tasks
        cx.schedule
            .heartbeat(cx.start + SYSTEM_CLOCK.cycles())
            .unwrap();

        cx.schedule
            .print_status(cx.start + (SYSTEM_CLOCK / 2).cycles())
            .unwrap();

        cx.schedule
            .disco_mode(cx.start + (SYSTEM_CLOCK / 4).cycles())
            .unwrap();

        cx.schedule.ping(cx.start).unwrap();

        init::LateResources {
            tx,
            rx,
            led,
            ping_pong_pin,
            echo,
            duration_timer,
            range: 0.0_f32,
            light_controller,
        }
    }

    #[task(schedule = [print_status], resources = [tx, range])]
    fn print_status(cx: print_status::Context) {
        let tx = cx.resources.tx;
        let range = cx.resources.range;
        write!(tx, "measured range: {:.2}cm\r", range).unwrap();

        // print every 1 second
        cx.schedule
            .print_status(cx.scheduled + SYSTEM_CLOCK.cycles())
            .unwrap();
    }

    #[task(schedule = [heartbeat], resources = [led] )]
    fn heartbeat(cx: heartbeat::Context) {
        static mut TOGGLE: bool = false;

        let led = cx.resources.led;

        if *TOGGLE {
            led.set_high().unwrap();
            *TOGGLE = false;
        } else {
            led.set_low().unwrap();
            *TOGGLE = true;
        }

        cx.schedule
            .heartbeat(cx.scheduled + SYSTEM_CLOCK.cycles())
            .unwrap();
    }

    #[task(schedule = [pong], resources = [ping_pong_pin])]
    fn ping(cx: ping::Context) {
        // HCSR04-23070007.pdf suggests 10uS pulse to trigger system
        const NEXT: u32 = (SYSTEM_CLOCK / 1_000_000) * 10;
        cx.resources.ping_pong_pin.set_high().unwrap();
        cx.schedule.pong(cx.scheduled + NEXT.cycles()).unwrap();
    }

    #[task(schedule = [ping], resources = [ping_pong_pin])]
    fn pong(cx: pong::Context) {
        // HCSR04-23070007.pdf suggests >60ms measurement cycle.
        const NEXT: u32 = (SYSTEM_CLOCK / 1000) * 60;
        cx.resources.ping_pong_pin.set_low().unwrap();
        cx.schedule.ping(cx.scheduled + NEXT.cycles()).unwrap();
    }

    #[task(binds = EXTI9_5, resources = [echo, duration_timer, range])]
    fn receive_echo(cx: receive_echo::Context) {
        static mut START_TIME: Option<Instant> = None;
        if cx.resources.echo.check_interrupt() {
            cx.resources.echo.clear_interrupt_pending_bit();

            let pin = cx.resources.echo;
            let tim = cx.resources.duration_timer;
            let output = cx.resources.range;

            *START_TIME = if pin.is_high().unwrap() {
                Some(tim.now())
            } else {
                if let Some(get_time) = *START_TIME {
                    let Hertz(freq) = tim.frequency();
                    let pulse_time_ms = 1000.0 * get_time.elapsed() as f32 / freq as f32;
                    *output = measured_range(pulse_time_ms);
                }
                None
            };
        }
    }

    #[task(schedule = [disco_mode], resources = [light_controller]) ]
    fn disco_mode(cx: disco_mode::Context) {
        static mut RED: u16 = 0;
        static mut GREEN: u16 = 0;
        static mut BLUE: u16 = 0;



        // insert code here

        let lc = cx.resources.light_controller;
        lc.set_color_rgb(*RED, *GREEN, *BLUE);

        cx.schedule
            .disco_mode(cx.scheduled + (u16::MAX as u32 / 8).cycles())
            .unwrap();
    }

    extern "C" {
        fn EXTI0();
    }
};
