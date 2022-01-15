use crate::app;
use core::fmt::Write;
use systick_monotonic::*;
// Prints Periodically.
pub fn print_status(cx: app::print_status::Context) {
    let tx = cx.local.tx;
    let range = cx.shared.range;
    write!(tx, "measured range: {:.2}cm\r", range).unwrap();

    // print every 1 second
    app::print_status::spawn_after(1.secs()).unwrap();
}
