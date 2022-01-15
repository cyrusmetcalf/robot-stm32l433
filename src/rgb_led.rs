use crate::app;
use embedded_hal_pwm_utilities::rgb_controller::SixColor;
use systick_monotonic::*;

pub fn set_light_from_range(cx: app::set_light_from_range::Context) {
    let lc = cx.local.light_controller;
    let range = cx.shared.range;

    match *range as u32 {
        0..=19 => lc.red(),
        20..=39 => lc.yellow(),
        40..=59 => lc.green(),
        60..=79 => lc.cyan(),
        80..=99 => lc.blue(),
        _ => lc.magenta(),
    }
    app::set_light_from_range::spawn_after(100.millis()).unwrap();
}
