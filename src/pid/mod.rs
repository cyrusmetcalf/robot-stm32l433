use core::time::Duration;

#[derive(Debug, PartialEq)]
pub struct Pid {
    p_coefficient: f32,
    i_coefficient: f32,
    d_coefficient: f32,
    last_iterm: f32,
    last_error: f32,
}

impl Pid {
    pub fn new(p_coefficient: f32, i_coefficient: f32, d_coefficient: f32) -> Pid {
        Pid {
            p_coefficient,
            i_coefficient,
            d_coefficient,
            last_iterm: 0.0,
            last_error: 0.0,
        }
    }

    pub fn update(&mut self, setpoint: f32, measurement: f32, dt: Duration) -> f32 {
        let error = self.error(setpoint, measurement);
        self.p_term(error) * self.p_coefficient
            + self.i_term(error, dt.as_secs_f32()) * self.i_coefficient
            + self.d_term(error, dt.as_secs_f32()) * self.d_coefficient
    }

    fn error(&self, setpoint: f32, measurement: f32) -> f32 {
        setpoint - measurement
    }

    fn p_term(&self, error: f32) -> f32 {
        error
    }

    fn i_term(&mut self, error: f32, delta_time: f32) -> f32 {
        let i_term = self.last_iterm + error * delta_time;
        self.last_iterm = i_term;
        i_term
    }

    fn d_term(&mut self, error: f32, delta_time: f32) -> f32 {
        if delta_time == 0.0_f32 {
            return 0.0_f32;
        }
        let d_term = (error - self.last_error) / delta_time;
        self.last_error = error;
        d_term
    }
}

#[cfg(test)]
mod pid_tests {
    use super::*;
    use core::time::Duration;

    struct TestPid;
    impl TestPid {
        const P: f32 = 1.0;
        const I: f32 = 1.0;
        const D: f32 = 1.0;
        const ZERO: f32 = 0.0;
    }

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn can_make_new_pid_controller() {
        assert_eq!(
            Pid::new(TestPid::P, TestPid::I, TestPid::D),
            Pid {
                p_coefficient: TestPid::P,
                i_coefficient: TestPid::I,
                d_coefficient: TestPid::D,
                last_iterm: TestPid::ZERO,
                last_error: TestPid::ZERO,
            }
        );
    }

    #[test]
    fn can_calculate_error_zero() {
        let setpoint = 24.7_f32;
        let measurement = 24.7_f32;
        let pid = Pid::new(TestPid::P, TestPid::I, TestPid::D);
        assert!(pid.error(setpoint, measurement) < f32::EPSILON);
    }

    #[test]
    fn can_calculate_proportional_term_private() {
        let setpoint = 234.34_f32;
        let measurement = 0.0_f32;
        let expected = setpoint - measurement;
        let dt = Duration::new(0, 0);

        let mut pid = Pid::new(TestPid::P, TestPid::ZERO, TestPid::ZERO);
        assert_eq!(pid.p_term(expected), expected);
        assert_eq!(pid.update(setpoint, measurement, dt), expected);
    }

    #[test]
    fn can_calculate_integral_term_private() {
        let error = 55.6_f32;
        let dt = Duration::from_millis(1000).as_secs_f32();

        let mut pid = Pid::new(TestPid::ZERO, TestPid::I, TestPid::ZERO);
        let first_error = pid.i_term(error, dt);
        assert_eq!(first_error, error * dt);

        let second_error = pid.i_term(error, dt);
        assert_eq!(second_error, first_error + error * dt);
    }

    #[test]
    fn can_calculate_derivative_term_private() {
        let error = 32.4_f32;
        let dt = Duration::from_millis(1000).as_secs_f32();

        let mut pid = Pid::new(TestPid::ZERO, TestPid::ZERO, TestPid::D);
        assert_eq!(pid.d_term(error, TestPid::ZERO), TestPid::ZERO);

        let first_d_term = pid.d_term(error, dt);
        assert_eq!(first_d_term, error / dt);

        let second_d_term = pid.d_term(error, dt);
        assert_eq!(second_d_term, (error - first_d_term) / dt);
    }
}
