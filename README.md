# robot-stm32l433
A Rust based Robot slash prototyping framework using RTIC for NUCLEO-L433RC-P. 

## Current Supported Features
- Ultrasonic Rangefinder (HC-SR04. Not adjusted for humidity/temperature)
- PWM based RGB LED driver (from https://github.com/cyrusmetcalf/embedded-hal-pwm-utilities)
- Shmoos through 6 discrete colors based on range measured by rangefinder.  (light updates every 100ms) 
- Prints measured range (cm) to serial terminal every 1 second.

## Coming Soon
- Hobby Servo driver supporting full-rotation, and standard servos. 
- Line-follower driver
- Annoying piezo-electric music maker.  
- PID control of stuff and things.  
- Better documentation...

## Wiring
- gpiob.pb13 -> on-board user LED as heart-beat
- gpioa.pa2 -> USART tx  (default virtual comm port)
- gpioa.pa3 -> USART rx  (default virtual comm port)
- gpioa.pa8 -> TIM1 CH1 Red LED PWM
- gpioa.pa9 -> TIM1 CH2 Green LED PWM
- gpioa.pa10 -> TIM1 CH3 Blue LED PWM
- gpioa.pa0 ->  TIM2 CH1 servo pwm
- gpioa.pa1 -> TIM2 CH2 servo pwm


