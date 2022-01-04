use embedded_hal::PwmPin;

pub trait SixColor: embedded_hal::PwmPin {
    fn red(&mut self);
    fn blue(&mut self);
    fn green(&mut self);
    fn yellow(&mut self);
    fn magenta(&mut self);
    fn cyan(&mut self);
}

pub struct RgbController<R: embedded_hal::PwmPin, G: embedded_hal::PwmPin, B: embedded_hal::PwmPin>(
    pub R,
    pub G,
    pub B,
);

impl<R: embedded_hal::PwmPin<Duty=u16>,G: embedded_hal::PwmPin<Duty=u16>, B: embedded_hal::PwmPin<Duty=u16>> SixColor for RgbController<R,G,B> {
    fn red(&mut self) {
        let (r, _,_) = self.get_max_duty();
        self.set_duty((r, 0, 0));
    }

    fn blue(&mut self) {
        let (_,_,b) = self.get_max_duty();
        self.set_duty((0, 0, b));
    }

    fn green(&mut self) {
        let (_,g,_) = self.get_max_duty();
        self.set_duty((0, g, 0));
    }

    fn yellow(&mut self) {
        let (r,g,_) = self.get_max_duty();
        self.set_duty((r,g,0));
    }

    fn magenta(&mut self) {
        let (r,_,b) = self.get_max_duty();
        self.set_duty((r, 0, b));
    }

    fn cyan(&mut self) {
        let (_,g,b) = self.get_max_duty();
        self.set_duty((0, g, b));
    }
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
        let RgbController(red, green, blue) = self;
        (
            red.get_max_duty(),
            green.get_max_duty(),
            blue.get_max_duty(),
        )
    }
    fn set_duty(&mut self, duty: Self::Duty) {
        let (r, g, b) = duty;
        let RgbController(red, green, blue) = self;
        red.set_duty(r);
        green.set_duty(g);
        blue.set_duty(b);
    }
}


