use crate::app;
use stm32l4xx_hal::{gpio::ExtiPin, prelude::*, time::Hertz};
use systick_monotonic::*;

// Speed of Sound in cm/ms @ standard temperature/pressure, non-adjusted.
const SPEED_OF_SOUND: f32 = 34.3;

fn measured_range(echo_length_ms: f32) -> f32 {
    echo_length_ms * SPEED_OF_SOUND / 2.0
}

// Measures pulse-width on an input EXTI pin to the ms.  Pretty handy little task.
// also outputs this as a measured distance using the measured_range function.
pub fn receive_echo(cx: app::receive_echo::Context) {
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

// Pings, then Pongs, Periodically.
pub fn ping(cx: app::ping::Context) {
    // HCSR04-23070007.pdf suggests 10uS pulse to trigger system
    cx.shared.ping_pong_pin.set_high().unwrap();
    app::pong::spawn_at(app::monotonics::now() + systick_monotonic::ExtU64::micros(10)).unwrap();
}

// Only pongs if pinged. Then pings. Periodically.
pub fn pong(cx: app::pong::Context) {
    // HCSR04-23070007.pdf suggests >60ms measurement cycle.
    cx.shared.ping_pong_pin.set_low().unwrap();
    app::ping::spawn_after(60.millis()).unwrap();
}
