#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m::peripheral::DWT;
use stm32l4xx_hal::{
    gpio::PB13,
    gpio::{Output, PushPull},
    pac::USART2,
    prelude::*,
    serial,
    serial::{Config, Serial},
};

use rtic::cyccnt::U32Ext;

use servo_controller::ServoController;

#[rtic::app(device = stm32l4xx_hal::stm32,peripherals=true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        rx: serial::Rx<USART2>,
        tx: serial::Tx<USART2>,
        led: PB13<Output<PushPull>>,
    }

    #[init(schedule = [heartbeat])]
    fn init(mut cx: init::Context) -> init::LateResources {
        let dp = cx.device;

        // set up cycle-count
        cx.core.DCB.enable_trace();
        DWT::unlock();
        cx.core.DWT.enable_cycle_counter();

        let mut rcc = dp.RCC.constrain();
        let mut flash = dp.FLASH.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc
            .cfgr
            .sysclk(80.mhz())
            .hclk(80.mhz())
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
        let (pwm1,pwm2) =
            dp.TIM2
                .pwm((c1, c2), SERVO_PWM_FREQUENCY.hz(), clocks, &mut rcc.apb1r1);

        let mut servo_one = ServoController::new(pwm1, SERVO_PWM_FREQUENCY);
        let mut servo_two = ServoController::new(pwm2, SERVO_PWM_FREQUENCY);
        
        servo_one.enable(true);
        servo_two.enable(true);


        servo_one.set_vector(1.0);
        servo_two.set_vector(-1.0);
        

        // Scheduled Tasks
        cx.schedule
            .heartbeat(cx.start + 80_000_000_u32.cycles())
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
            .heartbeat(cx.scheduled + 80_000_000_u32.cycles())
            .unwrap();
    }

    extern "C" {
        fn EXTI0();
    }
};
