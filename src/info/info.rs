use crate::common::{PixelRect, PixelRectBound};
use crate::info::window_info::{WINDOW_16_9, WINDOW_4_3, WINDOW_8_5};

#[derive(Clone, Debug)]
pub struct ScanInfo {
    // pub panel_height: u32,
    // pub panel_width: u32,
    pub panel_position: PixelRect,

    pub title_position: PixelRect,
    pub main_stat_name_position: PixelRect,
    pub main_stat_value_position: PixelRect,
    pub level_position: PixelRect,

    pub sub_stat1_position: PixelRect,
    pub sub_stat2_position: PixelRect,
    pub sub_stat3_position: PixelRect,
    pub sub_stat4_position: PixelRect,

    pub equip_position: PixelRect,

    pub art_count_position: PixelRectBound,

    pub art_width: u32,
    pub art_height: u32,
    pub art_gap_x: u32,
    pub art_gap_y: u32,

    pub art_row: u32,
    pub art_col: u32,

    pub left_margin: f64,
    pub top_margin: f64,

    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,

    pub flag_x: u32,
    pub flag_y: u32,

    pub star_x: u32,
    pub star_y: u32,

    /* yas-lock specific */
    // a point inside lock icon
    pub lock_x: u32,
    pub lock_y: u32,
    // a point inside lock icon, relative to artifact card
    pub art_lock_x: f64,
    pub art_lock_y: f64,
    // a vertical line inside the first artifact
    pub ruler_left: u32,
    pub ruler_top: u32,
    pub ruler_height: u32,
    // a point inside the artifact icon in the top menu
    pub menu_x: u32,
    pub menu_y: u32,
    // scrollbar
    pub scrollbar_left: u32,
    pub scrollbar_top: u32,
    pub scrollbar_height: u32,
    // artifact height + gap_y
    pub art_shift_x: f64,
    pub art_shift_y: f64,
}

impl ScanInfo {
    pub fn from_16_9(width: u32, height: u32, left: i32, top: i32) -> ScanInfo {
        WINDOW_16_9.to_scan_info(height as f64, width as f64, left, top)
    }

    pub fn from_8_5(width: u32, height: u32, left: i32, top: i32) -> ScanInfo {
        WINDOW_8_5.to_scan_info(height as f64, width as f64, left, top)
    }

    pub fn from_4_3(width: u32, height: u32, left: i32, top: i32) -> ScanInfo {
        WINDOW_4_3.to_scan_info(height as f64, width as f64, left, top)
    }
}

impl ScanInfo {
    pub fn from_rect(rect: &PixelRect) -> Result<ScanInfo, String> {
        let info: ScanInfo;
        if rect.height * 16 == rect.width * 9 {
            info = ScanInfo::from_16_9(rect.width as u32, rect.height as u32, rect.left, rect.top);
        } else if rect.height * 8 == rect.width * 5 {
            info = ScanInfo::from_8_5(rect.width as u32, rect.height as u32, rect.left, rect.top);
        } else if rect.height * 4 == rect.width * 3 {
            info = ScanInfo::from_4_3(rect.width as u32, rect.height as u32, rect.left, rect.top);
        } else {
            return Err(String::from("不支持的分辨率"));
        }

        Ok(info)
    }
}
