#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m::peripheral::DWT;
use rtic::cyccnt::U32Ext;
use servo_controller::ServoController;

//use nb::block;
use stm32l4xx_hal::{
    self,
    gpio::{Edge, Input, Output, PullDown, PushPull},
    gpio::{PB13, PB6},
    interrupt,
    pac::TIM6,
    pac::USART2,
    prelude::*,
    serial,
    serial::{Config, Serial},
    time::{Instant, MonoTimer},
    timer::Timer,
};

const SYSTEM_CLOCK: u32 = 80_000_000;

#[rtic::app(device = stm32l4xx_hal::stm32,peripherals=true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        rx: serial::Rx<USART2>,
        tx: serial::Tx<USART2>,
        led: PB13<Output<PushPull>>,
        echo: PB6<Input<PullDown>>,
        duration_timer: MonoTimer,
        delay_timer: Timer<TIM6>,
    }

    #[init(schedule = [heartbeat])]
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

        //Delay Provider
        let delay_timer = Timer::tim6(dp.TIM6, SYSTEM_CLOCK.hz(), clocks, &mut rcc.apb1r1);

        //General Purpose Duration Timer
        let duration_timer = MonoTimer::new(cx.core.DWT, clocks);

        //delay_timer.start(1000);
        //block!(delay_timer.wait()).unwrap();

        //let end = start.elapsed();

        // GPIO
        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);

        // LED
        let led = gpiob
            .pb13
            .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);

        // USART 2
        let tx = gpioa.pa2.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
        let rx = gpioa.pa3.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
        let pins = (tx, rx);

        let baudrate = 38_400.bps();

        let serial2 = Serial::usart2(
            dp.USART2,
            pins,
            Config::default().baudrate(baudrate),
            clocks,
            &mut rcc.apb1r1,
        );

        let (tx, rx) = serial2.split();

        // PWM channel
        const SERVO_PWM_FREQUENCY: u32 = 50_u32; // Hz
        let c1 = gpioa
            .pa0
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrl);

        let c2 = gpioa
            .pa1
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrl);
        let (pwm1, pwm2) = dp
            .TIM2
            .pwm((c1, c2), SERVO_PWM_FREQUENCY.hz(), clocks, &mut rcc.apb1r1);

        let _servo_one = ServoController::new(pwm1, SERVO_PWM_FREQUENCY);
        let _servo_two = ServoController::new(pwm2, SERVO_PWM_FREQUENCY);

        // Range Finder

        // we need an edge-triggered interrupt that measures how long it was held high.
        let mut echo = gpiob
            .pb6
            .into_pull_down_input(&mut gpiob.moder, &mut gpiob.pupdr);
        echo.make_interrupt_source(&mut dp.SYSCFG, &mut rcc.apb2);
        echo.trigger_on_edge(&mut dp.EXTI, Edge::RISING_FALLING);
        echo.enable_interrupt(&mut dp.EXTI);

        rtic::pend(interrupt::EXTI9_5);

        // Scheduled Tasks
        cx.schedule
            .heartbeat(cx.start + SYSTEM_CLOCK.cycles())
            .unwrap();
        init::LateResources {
            tx,
            rx,
            led,
            echo,
            duration_timer,
            delay_timer,
        }
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

    //#[task(schedule = [send_sonar_ping])]
    //fn send_sonar_ping(cx: send_sonar_ping::Context) {
    //    // this task kicks the whole thing off.
    //    // 1.) send 10us pulse over a GPIO pin.
    //    // 2.)
    //}
    //

    #[task]
    fn timer_function_thing(_cx: timer_function_thing::Context, start_time: Option<Instant>) {
        static mut STOPWATCH: Option<Instant> = None;
        static mut ELAPSED_TIME: u32 = 0;

        if start_time.is_some() {
            *STOPWATCH = start_time;
        } else {
            match *STOPWATCH {
                Some(timer) => *ELAPSED_TIME = timer.elapsed(),
                _ => (),
            }
        }
    }

    #[task(binds = EXTI9_5, spawn = [timer_function_thing], resources = [echo, duration_timer])]
    fn receive_echo(cx: receive_echo::Context) {
        if cx.resources.echo.check_interrupt() {
            cx.resources.echo.clear_interrupt_pending_bit();
            let better_name_tbd = if cx.resources.echo.is_high().unwrap() {
                Some(cx.resources.duration_timer.now())
            } else {
                None
            };
            cx.spawn.timer_function_thing(better_name_tbd).unwrap();
        }
    }

    extern "C" {
        fn EXTI0();
    }
};
