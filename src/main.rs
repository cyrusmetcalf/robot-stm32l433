#![no_std]
#![no_main]

use panic_halt as _;

// Speed of Sound in cm/ms @ standard temperature/pressure, non-adjusted.
const SPEED_OF_SOUND: f32 = 34.3;

fn measured_range(echo_length_ms: f32) -> f32 {
    echo_length_ms * SPEED_OF_SOUND / 2.0
}

#[rtic::app(device = stm32l4xx_hal::stm32, dispatchers = [EXTI0])]
mod app {
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

    use crate::measured_range;
    use core::fmt::Write;
    use cortex_m::peripheral::DWT;
    use embedded_hal_pwm_utilities::rgb_controller::{RgbController, SixColor};
    use systick_monotonic::*;

    const SYSTEM_CLOCK: u32 = 80_000_000;

    #[monotonic(binds=SysTick, default = true)]
    type MonotonicClock = Systick<1000>;

    #[shared]
    struct SharedResources {
        #[lock_free]
        ping_pong_pin: PB1<Output<PushPull>>,
        #[lock_free]
        range: f32,
    }

    #[local]
    struct LocalResources {
        tx: serial::Tx<USART2>,
        led: PB13<Output<PushPull>>,
        echo: PB6<Input<PullDown>>,
        duration_timer: MonoTimer,
        light_controller: RgbController<Pwm<TIM1, C1>, Pwm<TIM1, C2>, Pwm<TIM1, C3>>,
    }

    #[init]
    fn init(mut cx: init::Context) -> (SharedResources, LocalResources, init::Monotonics) {
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

        let systick = cx.core.SYST;

        // mono timer
        let monotonic_clock = Systick::new(systick, clocks.sysclk().0);

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

        let (tx, _rx) = Serial::usart2(
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
        let (r, g, b) = dp.TIM1.pwm(
            (red_pin, green_pin, blue_pin),
            RGB_LED_PWM_FREQUENCY.khz(),
            clocks,
            &mut rcc.apb2,
        );

        let mut light_controller = RgbController(r, g, b);
        light_controller.enable();

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

        let _wheel_pins = dp.TIM2.pwm(
            (left_wheel_pin, right_wheel_pin),
            RGB_LED_PWM_FREQUENCY.hz(),
            clocks,
            &mut rcc.apb1r1,
        );

        //        let mut wheel_controller = Wheels::new(wheel_pins);
        //        wheel_controller.enable();
        //
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
        heartbeat::spawn_after(1.secs()).unwrap();
        disco_mode::spawn_after(1.secs()).unwrap();
        print_status::spawn_at(monotonics::now()).unwrap();
        ping::spawn_at(monotonics::now()).unwrap();

        (
            SharedResources {
                ping_pong_pin,
                range: 0.0,
            },
            LocalResources {
                tx,
                led,
                echo,
                duration_timer,
                light_controller,
            },
            init::Monotonics(monotonic_clock),
        )
    }

    // Prints Periodically.
    #[task(local = [tx], shared = [range])]
    fn print_status(cx: print_status::Context) {
        let tx = cx.local.tx;
        let range = cx.shared.range;
        write!(tx, "measured range: {:.2}cm\r", range).unwrap();

        // print every 1 second
        print_status::spawn_after(1.secs()).unwrap();
    }

    // Beats Periodically.
    #[task(local = [led, toggle: bool = false] )]
    fn heartbeat(cx: heartbeat::Context) {
        let led = cx.local.led;
        let toggle = cx.local.toggle;

        if *toggle {
            led.set_high().unwrap();
            *toggle = false;
        } else {
            led.set_low().unwrap();
            *toggle = true;
        }
        heartbeat::spawn_after(1.secs()).unwrap();
    }

    // Parties Periodically.
    #[task(local = [light_controller, counter: u32 = 0])]
    fn disco_mode(cx: disco_mode::Context) {
        let lc = cx.local.light_controller;
        let counter = cx.local.counter;

        *counter += 1;
        match counter {
            1 => lc.red(),
            2 => lc.yellow(),
            3 => lc.green(),
            4 => lc.cyan(),
            5 => lc.blue(),
            _ => {
                lc.magenta();
                *counter = 0;
            }
        }
        disco_mode::spawn_after(1.secs()).unwrap();
    }

    // Pings, then Pongs, Periodically.
    #[task(shared = [ping_pong_pin])]
    fn ping(cx: ping::Context) {
        // HCSR04-23070007.pdf suggests 10uS pulse to trigger system
        cx.shared.ping_pong_pin.set_high().unwrap();
        pong::spawn_after(systick_monotonic::ExtU64::micros(10)).unwrap();
    }

    // Only pongs if pinged. Then pings. Periodically.
    #[task(shared = [ping_pong_pin])]
    fn pong(cx: pong::Context) {
        // HCSR04-23070007.pdf suggests >60ms measurement cycle.
        cx.shared.ping_pong_pin.set_low().unwrap();
        ping::spawn_after(systick_monotonic::ExtU64::micros(10)).unwrap();
    }

    // Measures pulse-width on an input EXTI pin to the ms.  Pretty handy little task.
    // also outputs this as a measured distance using the measured_range function.
    #[task(binds = EXTI9_5, local = [echo, duration_timer, start_time: Option<Instant> = None],shared = [range])]
    fn receive_echo(cx: receive_echo::Context) {
        let start_time = cx.local.start_time;
        if cx.local.echo.check_interrupt() {
            cx.local.echo.clear_interrupt_pending_bit();

            let pin = cx.local.echo;
            let tim = cx.local.duration_timer;
            let output = cx.shared.range;

            *start_time = if pin.is_high().unwrap() {
                Some(tim.now())
            } else {
                if let Some(get_time) = *start_time {
                    let Hertz(freq) = tim.frequency();
                    let pulse_time_ms = 1000.0 * get_time.elapsed() as f32 / freq as f32;
                    *output = measured_range(pulse_time_ms);
                }
                None
            };
        }
    }
}
