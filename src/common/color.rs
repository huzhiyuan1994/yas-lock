#[derive(Debug)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub fn eq(&self, other: &Color) -> bool {
        self.0 == other.0 && self.1 == other.1 && self.2 == other.2
    }

    pub fn dis_2(&self, other: &Color) -> u32 {
        let dis = (self.0 as i32 - other.0 as i32) * (self.0 as i32 - other.0 as i32)
            + (self.1 as i32 - other.1 as i32) * (self.1 as i32 - other.1 as i32)
            + (self.2 as i32 - other.2 as i32) * (self.2 as i32 - other.2 as i32);
        dis as u32
    }

    pub fn new() -> Color {
        Color(0, 0, 0)
    }

    pub fn from(r: u8, g: u8, b: u8) -> Color {
        Color(r, g, b)
    }
}
