use embedded_hal::PwmPin;

pub struct RgbController<R: embedded_hal::PwmPin, G: embedded_hal::PwmPin, B: embedded_hal::PwmPin>(
    pub R,
    pub G,
    pub B,
);

pub struct Wheels<L, R> {
    left: L,
    right: R,
}

impl<R: PwmPin, G: PwmPin, B: PwmPin> embedded_hal::PwmPin for RgbController<R, G, B> {
    type Duty = (R::Duty, G::Duty, B::Duty);
    fn disable(&mut self) {
        let RgbController(red, green, blue) = self;
        red.disable();
        green.disable();
        blue.disable();
    }
    fn enable(&mut self) {
        let RgbController(red, green, blue) = self;
        red.enable();
        green.enable();
        blue.enable();
    }
    fn get_duty(&self) -> Self::Duty {
        let RgbController(red, green, blue) = self;
        (red.get_duty(), green.get_duty(), blue.get_duty())
    }
    fn get_max_duty(&self) -> Self::Duty {
        (
            self.0.get_max_duty(),
            self.1.get_max_duty(),
            self.2.get_max_duty(),
        )
    }
    fn set_duty(&mut self, duty: Self::Duty) {
        let (r, g, b) = duty;
        self.0.set_duty(r);
        self.1.set_duty(g);
        self.2.set_duty(b);
    }
}

impl<T, L: PwmPin<Duty = T>, R: PwmPin<Duty = T>> Wheels<L, R> {
    pub fn new(wheel_pwm: (L, R)) -> Wheels<L, R> {
        Wheels {
            left: wheel_pwm.0,
            right: wheel_pwm.1,
        }
    }
}

impl<L: PwmPin, R: PwmPin> embedded_hal::PwmPin for Wheels<L, R> {
    type Duty = (L::Duty, R::Duty);
    fn disable(&mut self) {
        self.left.disable();
        self.right.disable();
    }
    fn enable(&mut self) {
        self.left.enable();
        self.right.enable();
    }
    fn get_duty(&self) -> Self::Duty {
        (self.left.get_duty(), self.right.get_duty())
    }
    fn get_max_duty(&self) -> Self::Duty {
        (self.left.get_max_duty(), self.right.get_max_duty())
    }

    fn set_duty(&mut self, duty: Self::Duty) {
        let (l, r) = duty;
        self.left.set_duty(l);
        self.right.set_duty(r);
    }
}
