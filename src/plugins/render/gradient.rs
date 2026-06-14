use cairo::LinearGradient;

#[derive(Debug, Clone, Copy)]
pub struct RgbColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

pub enum GradientDirection {
    Horizontal,
    Vertical,
}

#[derive(Default)]
pub struct GradientStops {
    stops: Vec<(f64, f64, f64, f64, f64)>, // offset, r, g, b, alpha
}

impl GradientStops {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_stop(mut self, offset: f64, color: RgbColor, alpha: f64) -> Self {
        self.stops.push((offset, color.r, color.g, color.b, alpha));
        self
    }

    pub fn apply(&self, gradient: &LinearGradient) {
        for &(offset, r, g, b, alpha) in &self.stops {
            gradient.add_color_stop_rgba(offset, r, g, b, alpha);
        }
    }
}

pub struct GradientBuilder {
    direction: GradientDirection,
    width: f64,
    height: f64,
}

impl GradientBuilder {
    pub fn new(direction: GradientDirection, width: f64, height: f64) -> Self {
        Self {
            direction,
            width,
            height,
        }
    }

    pub fn build(&self, stops: &GradientStops) -> LinearGradient {
        let gradient = match self.direction {
            GradientDirection::Horizontal => LinearGradient::new(0.0, 0.0, self.width, 0.0),
            GradientDirection::Vertical => LinearGradient::new(0.0, 0.0, 0.0, self.height),
        };
        stops.apply(&gradient);
        gradient
    }
}

pub fn rgb_from_hex(hex: &str) -> Option<RgbColor> {
    let s = hex.trim().strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    Some(RgbColor {
        r: u8::from_str_radix(&s[0..2], 16).ok()? as f64 / 255.0,
        g: u8::from_str_radix(&s[2..4], 16).ok()? as f64 / 255.0,
        b: u8::from_str_radix(&s[4..6], 16).ok()? as f64 / 255.0,
    })
}
