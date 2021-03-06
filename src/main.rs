#![no_std]
#![no_main]

use panic_halt as _;

pub mod blinky;
pub mod range_finder;
pub mod rgb_led;
pub mod status_reporter;

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
        time::{Instant, MonoTimer},
    };

    use cortex_m::peripheral::DWT;
    use systick_monotonic::*;

    use crate::{
        blinky::heartbeat,
        range_finder::{ping, pong, receive_echo},
        rgb_led::set_light_from_range,
        status_reporter::print_status,
    };
    use embedded_hal_pwm_utilities::rgb_controller::RgbController;

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
        set_light_from_range::spawn_after(1.secs()).unwrap();
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

    // RGB Led control task
    extern "Rust" {
        #[task(local = [light_controller], shared = [range])]
        fn set_light_from_range(_: set_light_from_range::Context);
    }

    // Status Print
    extern "Rust" {
        #[task(local = [tx], shared = [range])]
        fn print_status(_: print_status::Context);
    }

    // Heartbeat Task
    extern "Rust" {
        #[task(local = [led, toggle: bool = false] )]
        fn heartbeat(_: heartbeat::Context);
    }

    // Ultrasonic Range Finder Tasks
    extern "Rust" {
        // Measures pulse-width of return signal in ms and calculates range based on speed of sound.
        #[task(binds = EXTI9_5, local = [echo, duration_timer, start_time: Option<Instant> = None],shared = [range])]
        fn receive_echo(_: receive_echo::Context);

        // Pings, then Pongs, Periodically.
        #[task(shared = [ping_pong_pin])]
        fn ping(_: ping::Context);

        // Only pongs if pinged. Then pings. Periodically.
        #[task(shared = [ping_pong_pin])]
        fn pong(_: pong::Context);
    }
}
