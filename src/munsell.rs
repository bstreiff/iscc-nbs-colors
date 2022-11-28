use lazy_static::lazy_static;
use palette::{LabHue, Lch};
use regex::Regex;
use std::fmt;

const LETTER_CODES: &[&str] = &["R", "YR", "Y", "GY", "G", "BG", "B", "PB", "P", "RP"];

/// The hue is a circular type, where `0` and `100` is the same, and
/// it's normalized to `[0, 100)` when it's converted to a linear
/// number (like `f32`). This makes many calculations easier, but may
/// also have some surprising effects if it's expected to act as a
/// linear number.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct MunsellHue(f32);

impl MunsellHue {
    /// Create a new hue.
    #[inline]
    pub const fn new(angle: f32) -> Self {
        Self(angle)
    }

    #[inline]
    pub fn raw(&self) -> f32 {
        self.0
    }

    #[inline]
    pub fn from_str(huespec: &str) -> Self {
        Self::new(huespec_to_point(huespec))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn from_degrees(degrees: f32) -> Self {
        Self::new(normalize_angle_positive(degrees * (100.0 / 360.0)))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn from_radians(radians: f32) -> Self {
        Self::new(normalize_angle_positive(
            radians.to_degrees() * (100.0 / 360.0),
        ))
    }

    #[inline]
    pub fn to_degrees(&self) -> f32 {
        self.0 * (360.0 / 100.0)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn to_radians(&self) -> f32 {
        self.to_degrees().to_radians()
    }
}

impl fmt::Display for MunsellHue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hp = (self.0 + 5.0) % 100.0;
        let hn = hp % 10.0;
        let index = ((hp - hn) / 10.0) as usize;

        write!(f, "{:1.2}{}", hn, LETTER_CODES[index])
    }
}

#[inline]
fn normalize_angle_positive(point: f32) -> f32 {
    point - ((point / 100.0).floor() * 100.0)
}

fn huespec_to_point(huespec: &str) -> f32 {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^(\d*\.?\d+)(R|YR|Y|GY|G|BG|B|PB|P|RP)").unwrap();
    }

    let caps = RE.captures(huespec).unwrap();
    let hue_number = caps.get(1).unwrap().as_str().parse::<f32>().unwrap();
    let hue_code = (match caps.get(2).unwrap().as_str() {
        "R" => Ok(0),
        "YR" => Ok(1),
        "Y" => Ok(2),
        "GY" => Ok(3),
        "G" => Ok(4),
        "BG" => Ok(5),
        "B" => Ok(6),
        "PB" => Ok(7),
        "P" => Ok(8),
        "RP" => Ok(9),
        _ => Err("Invalid hue code"),
    })
    .unwrap();
    let hue_value: f32 = (((hue_code * 10) as f32) + (hue_number - 5.0) + 100.0) % 100.0;

    return hue_value;
}

#[derive(PartialEq, Debug, Clone)]
pub struct MunsellColor {
    pub hue: MunsellHue,
    pub value: f32,
    pub chroma: f32,
}

impl MunsellColor {
    #[inline]
    pub fn new(hue: MunsellHue, value: f32, chroma: f32) -> Self {
        Self::new_const(hue, value, chroma)
    }

    pub const fn new_const(hue: MunsellHue, value: f32, chroma: f32) -> Self {
        MunsellColor { hue, value, chroma }
    }

    /// Return an approximation of CIELAB Lch from this Munsell color.
    ///
    /// This uses a method similar to Paul Centore's [CIELABtoApproxMunsellSpec](https://github.com/colour-science/MunsellAndKubelkaMunkToolbox/blob/master/GeneralRoutines/CIELABtoApproxMunsellSpec.m),
    /// where the Munsell value is Lch_L / 10, and Munsell chroma
    /// is Lch_C / 5. I use a slightly different mechanism for computing
    /// the resulting hue.
    pub fn to_approximate_lch(&self) -> Lch {
        let l: f32 = self.value * 10.0;
        let c: f32 = self.chroma * 5.0;
        let hue: f32 = self.hue.raw();

        let index_float = hue / 20.00;
        let index = index_float as usize;
        let index_remainder = index_float - (index as f32);

        // LCh has four primaries; we need to sneak Purple in to match
        const LABHUE_HUES: [f32; 6] = [
            24.00,          // Red
            90.00,          // Yellow
            145.00,         // Green
            245.00,         // Blue
            310.00,         // Purple
            360.00 + 24.00, // Red (again)
        ];

        let h = interpolation::lerp(
            &LABHUE_HUES[index],
            &LABHUE_HUES[index + 1],
            &index_remainder,
        );
        let lch_hue = LabHue::from_degrees(h);

        return Lch::with_wp(l, c, lch_hue);
    }
}

impl fmt::Display for MunsellColor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}/{}", self.hue, self.value, self.chroma)
    }
}

#[cfg(test)]
mod test {
    use crate::MunsellHue;

    #[test]
    fn hue_from_string() {
        assert_eq!(MunsellHue::from_str("5R"), MunsellHue::new(0.0));
        assert_eq!(MunsellHue::from_str("5Y"), MunsellHue::new(20.0));
        assert_eq!(MunsellHue::from_str("5.5Y"), MunsellHue::new(20.5));
    }

    #[test]
    fn hue_display() {
        assert_eq!(format!("{}", MunsellHue::new(0.0)), "5.00R");
        assert_eq!(format!("{}", MunsellHue::new(20.0)), "5.00Y");
        assert_eq!(format!("{}", MunsellHue::new(20.5)), "5.50Y");
    }
}
