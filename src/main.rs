#![no_std]
#![no_main]

use panic_halt as _;

//use core::time::Duration;
use cortex_m::peripheral::DWT;
use stm32l4xx_hal::{
    gpio::{PB13, PB6},
    gpio::{Output,Input, PushPull, PullDown, Edge},
    pac::USART2,
    prelude::*,
    serial,
    serial::{Config, Serial},
    interrupt,
};
use rtic::cyccnt::{U32Ext};
use servo_controller::ServoController;


const SYSTEM_CLOCK_HZ: u32 = 80_000_000;

#[rtic::app(device = stm32l4xx_hal::stm32,peripherals=true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        rx: serial::Rx<USART2>,
        tx: serial::Tx<USART2>,
        led: PB13<Output<PushPull>>,
        //echo: PB6<Input<PullDown>>, 
    }

    #[init(schedule = [heartbeat])]
    fn init(mut cx: init::Context) -> init::LateResources {
        let mut dp = cx.device;

        // Prevent instibility on sleep with Probe-run
//        dp.DBGMCU.cr.modify(|_, w| {
//            w.dbg_sleep().set_bit();
//            w.dbg_standby().set_bit();
//            w.dbg_stop().set_bit()
//        });
//
        // set up cycle-count
        cx.core.DCB.enable_trace();
        DWT::unlock();
        cx.core.DWT.enable_cycle_counter();

        let mut rcc = dp.RCC.constrain();
        let mut flash = dp.FLASH.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc
            .cfgr
            .sysclk(SYSTEM_CLOCK_HZ.mhz())
            .hclk(SYSTEM_CLOCK_HZ.mhz())
            .freeze(&mut flash.acr, &mut pwr);

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
    //    let mut echo = gpiob.pb6.into_pull_down_input(&mut gpiob.moder, &mut gpiob.pupdr);
    //    echo.make_interrupt_source(&mut dp.SYSCFG, &mut rcc.apb2);
    //    echo.trigger_on_edge(&mut dp.EXTI, Edge::RISING_FALLING);
    //    echo.enable_interrupt(&mut dp.EXTI);

    //    rtic::pend(interrupt::EXTI9_5);


        // Scheduled Tasks
        cx.schedule
            .heartbeat(cx.start + SYSTEM_CLOCK_HZ.cycles())
            .unwrap();
        init::LateResources { tx, rx, led }
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
            .heartbeat(cx.scheduled + SYSTEM_CLOCK_HZ.cycles())
            .unwrap();
    }

    //#[task(binds = EXTI9_5, resources = [echo])]
    //fn receive_echo(cx:  receive_echo::Context) {
    //    static mut TEST_VALUE: bool = false;
    //    if cx.resources.echo.check_interrupt() { 
    //        cx.resources.echo.clear_interrupt_pending_bit();
    //        if cx.resources.echo.is_high().unwrap() {
    //            *TEST_VALUE = true;
    //        } else {
    //            *TEST_VALUE = false;
    //        }
    //    }
    //}

    extern "C" {
        fn EXTI0();
    }
};

