use crate::app;
use stm32l4xx_hal::prelude::*;
use systick_monotonic::*;

// Beats Periodically.
pub fn heartbeat(cx: app::heartbeat::Context) {
    let led = cx.local.led;
    let toggle = cx.local.toggle;

    if *toggle {
        led.set_high().unwrap();
        *toggle = false;
    } else {
        led.set_low().unwrap();
        *toggle = true;
    }
    app::heartbeat::spawn_after(1.secs()).unwrap();
}
