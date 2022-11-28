pub fn degree_average(f1: f32, f2: f32) -> f32 {
    let c1 = f1.to_radians().cos();
    let c2 = f2.to_radians().cos();
    let s1 = f1.to_radians().sin();
    let s2 = f2.to_radians().sin();
    let cavg = (c1 + c2) / 2.0;
    let savg = (s1 + s2) / 2.0;

    return savg.atan2(cavg).to_degrees();
}

pub fn degree_diff(f1: f32, f2: f32) -> f32 {
    let c = (f1.to_radians() - f2.to_radians()).cos();
    let s = (f1.to_radians() - f2.to_radians()).sin();

    return s.atan2(c).to_degrees().abs();
}

#[cfg(test)]
mod test {
    use crate::degree_average;
    use crate::degree_diff;

    #[test]
    fn test_averages() {
        assert!(degree_average(20.0, 40.0) - 30.0 < 0.0001);
        assert!(degree_average(350.0, 20.0) - 10.0 < 0.0001);
    }

    #[test]
    fn test_diffs() {
        assert!(degree_diff(20.0, 40.0) - 20.0 < 0.0001);
        assert!(degree_diff(350.0, 20.0) - 30.0 < 0.0001);
    }
}
