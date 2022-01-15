use crate::app;
use embedded_hal_pwm_utilities::rgb_controller::SixColor;
use systick_monotonic::*;

pub fn set_light_from_range(cx: app::set_light_from_range::Context) {
    let lc = cx.local.light_controller;
    let range = cx.shared.range;

    match *range as u32 {
        0..=3 => lc.red(),
        4..=9 => lc.yellow(),
        10..=19 => lc.green(),
        20..=29 => lc.cyan(),
        30..=40 => lc.blue(),
        _ => lc.magenta(),
    }
    app::set_light_from_range::spawn_after(100.millis()).unwrap();
}
