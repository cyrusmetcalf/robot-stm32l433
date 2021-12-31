    use embedded_hal::PwmPin;
    pub struct RgbController<R, G, B> {
        red: R,
        green: G,
        blue: B,
    }

    impl<R: PwmPin, G: PwmPin, B: PwmPin>  embedded_hal::PwmPin for RgbController<R,G,B> {
        type Duty = (R::Duty, G::Duty, B::Duty);
        fn disable(&mut self) {
            self.red.disable();
            self.green.disable();
            self.blue.disable();
        }
        fn enable(&mut self) {
            self.red.enable();
            self.green.enable();
            self.blue.enable();
        }
        fn get_duty(&self) -> Self::Duty {
            (self.red.get_duty(),self.green.get_duty(), self.blue.get_duty())
        }
        fn get_max_duty(&self) -> Self::Duty {
            (self.red.get_max_duty(),self.green.get_max_duty(), self.blue.get_max_duty())

        }
        fn set_duty(&mut self, duty: Self::Duty) {
            let (r,g,b) = duty;
            self.red.set_duty(r);
            self.green.set_duty(g);
            self.blue.set_duty(b);
        }

    }

    impl<T, R: PwmPin<Duty = T>, G: PwmPin<Duty = T>, B: PwmPin<Duty = T>> RgbController<R, G, B> {
        pub fn new(rgb_pwm:(R,G,B)) -> RgbController<R, G, B> {
            RgbController { red: rgb_pwm.0, green: rgb_pwm.1, blue: rgb_pwm.2 }
        }

        pub fn set_color_rgb(&mut self, red_level: T, green_level: T, blue_level: T) {
            self.set_duty((red_level,green_level,blue_level));
        }
    }
