use std::collections::HashSet;
use std::convert::From;
use std::fs;
use std::sync::mpsc;
use std::thread;
use std::time::SystemTime;

use clap::ArgMatches;
use enigo::*;
use log::{error, info, warn};

use crate::artifact::internal_artifact::{
    ArtifactSetKey, ArtifactSlotKey, ArtifactStat, CharacterKey, InternalArtifact,
};
use crate::capture;
use crate::common::color::Color;
use crate::common::{utils, PixelRect, PixelRectBound, RawCaptureImage};
use crate::inference::inference::CRNNModel;
use crate::inference::pre_process::pre_process;
use crate::info::info::ScanInfo;

pub struct YasScannerConfig {
    max_row: u32,
    capture_only: bool,
    min_star: u32,
    min_level: u32,
    max_wait_switch_artifact: u32,
    scroll_stop: u32,
    number: u32,
    verbose: bool,
    dump_mode: bool,
    // offset_x: i32,
    // offset_y: i32,
}

impl YasScannerConfig {
    pub fn from_match(matches: &ArgMatches) -> YasScannerConfig {
        YasScannerConfig {
            max_row: matches
                .value_of("max-row")
                .unwrap_or("1000")
                .parse::<u32>()
                .unwrap(),
            capture_only: matches.is_present("capture-only"),
            dump_mode: matches.is_present("dump"),
            min_star: matches
                .value_of("min-star")
                .unwrap_or("4")
                .parse::<u32>()
                .unwrap(),
            min_level: matches
                .value_of("min-level")
                .unwrap_or("0")
                .parse::<u32>()
                .unwrap(),
            max_wait_switch_artifact: matches
                .value_of("max-wait-switch-artifact")
                .unwrap_or("800")
                .parse::<u32>()
                .unwrap(),
            scroll_stop: matches
                .value_of("scroll-stop")
                .unwrap_or("80")
                .parse::<u32>()
                .unwrap(),
            number: matches
                .value_of("number")
                .unwrap_or("0")
                .parse::<u32>()
                .unwrap(),
            verbose: matches.is_present("verbose"),
            // offset_x: matches.value_of("offset-x").unwrap_or("0").parse::<i32>().unwrap(),
            // offset_y: matches.value_of("offset-y").unwrap_or("0").parse::<i32>().unwrap(),
        }
    }
}

pub struct YasScanner {
    model: CRNNModel,
    enigo: Enigo,

    info: ScanInfo,
    config: YasScannerConfig,

    row: u32,
    col: u32,

    pool: f64,

    initial_color: Color,

    // for scrolls
    scrolled_rows: u32,
    avg_scroll_one_row: f64,

    avg_switch_time: f64,
    scanned_count: u32,

    pixel_per_scroll: u32,
    art_count: u32,
    offset_y: i32,
}

enum ScrollResult {
    TLE, // time limit exceeded
    Interrupt,
    Success,
    Skip,
}

#[derive(Debug)]
pub struct YasScanResult {
    name: String,
    main_stat_name: String,
    main_stat_value: String,
    sub_stat_1: String,
    sub_stat_2: String,
    sub_stat_3: String,
    sub_stat_4: String,
    level: String,
    location: String,
    rarity: u32,
    lock: bool,
}

impl YasScanResult {
    pub fn to_internal_artifact(&self) -> Option<InternalArtifact> {
        let set_key = ArtifactSetKey::from_zh_cn(&self.name)?;
        let slot_key = ArtifactSlotKey::from_zh_cn(&self.name)?;
        let rarity = self.rarity;
        if !self.level.contains("+") {
            return None;
        }
        let level = self
            .level
            .chars()
            .skip(1)
            .collect::<String>()
            .parse::<u32>()
            .ok()?;
        let main_stat = ArtifactStat::from_zh_cn_raw(
            (self.main_stat_name.clone() + "+" + self.main_stat_value.as_str()).as_str(),
        )?;
        let sub1 = ArtifactStat::from_zh_cn_raw(&self.sub_stat_1);
        let sub2 = ArtifactStat::from_zh_cn_raw(&self.sub_stat_2);
        let sub3 = ArtifactStat::from_zh_cn_raw(&self.sub_stat_3);
        let sub4 = ArtifactStat::from_zh_cn_raw(&self.sub_stat_4);

        let location = if self.location.contains("已装备") {
            let len = self.location.chars().count();
            let character_name: String = self.location.chars().take(len - 3).collect::<String>();
            CharacterKey::from_zh_cn(&character_name)
        } else {
            None
        };

        let art = InternalArtifact {
            set_key,
            slot_key,
            rarity,
            level,
            location,
            lock: self.lock,
            main_stat,
            sub_stat_1: sub1,
            sub_stat_2: sub2,
            sub_stat_3: sub3,
            sub_stat_4: sub4,
        };
        Some(art)
    }
}

fn calc_pool(row: &Vec<u8>) -> f64 {
    let len = row.len() / 4;
    let mut pool: f64 = 0.0;

    for i in 0..len {
        pool += row[i * 4] as f64;
    }
    // pool /= len as f64;
    pool
}

fn eq(x: u8, y: u8, threshold: u8) -> bool {
    if x < y {
        y - x <= threshold
    } else if x > y {
        x - y <= threshold
    } else {
        true
    }
}

impl YasScanner {
    pub fn new(info: ScanInfo, config: YasScannerConfig) -> YasScanner {
        let row = info.art_row;
        let col = info.art_col;

        YasScanner {
            model: CRNNModel::new(
                String::from("model_training.onnx"),
                String::from("index_2_word.json"),
            ),
            enigo: Enigo::new(),
            info,
            config,

            row,
            col,

            pool: -1.0,
            initial_color: Color::new(),
            scrolled_rows: 0,
            avg_scroll_one_row: 0.0,

            avg_switch_time: 0.0,
            scanned_count: 0,

            pixel_per_scroll: 0,
            art_count: 1500,
            offset_y: 0,
        }
    }
}

impl YasScanner {
    fn move_to(&mut self, row: u32, col: u32) {
        let info = &self.info;
        let left = info.left
            + (info.left_margin + (info.art_width + info.art_gap_x) * col + info.art_width / 2)
                as i32;
        let top = info.top
            + (info.top_margin + (info.art_height + info.art_gap_y) * row + info.art_height / 4)
                as i32;
        self.enigo.mouse_move_to(left as i32, top as i32);
    }

    fn scroll(&mut self, offset: i32) {
        if offset < 0 {
            for _ in 0..(-offset) {
                self.enigo.mouse_scroll_y(-1);
            }
        } else {
            for _ in 0..offset {
                self.enigo.mouse_scroll_y(1);
            }
        }
        // utils::sleep(self.config.scroll_stop);
    }

    fn get_color(&self, x: u32, y: u32) -> Color {
        let x = x as i32 + self.info.left;
        let y = y as i32 + self.info.top;
        let color = capture::get_color(x as u32, y as u32);

        color
    }

    fn get_flag_color(&self) -> Color {
        self.get_color(self.info.flag_x, self.info.flag_y)
    }

    fn sample_initial_color(&mut self) {
        self.initial_color = self.get_flag_color();
    }

    fn get_ruler(&self) -> Vec<u8> {
        let rect = PixelRect {
            left: self.info.left as i32 + 150,
            top: self.info.top as i32 + 120,
            width: 1,
            height: 80,
        };
        capture::capture_absolute(&rect).unwrap()
    }

    fn check_menu(&self) {
        let color = self.get_color(self.info.menu_x, self.info.menu_y);
        if Color::from(236, 229, 216).dis_2(&color) > 0 {
            utils::error_and_quit("请打开背包圣遗物栏");
        }
    }

    fn check_at_top(&mut self) {
        self.move_to(0, 1);
        self.enigo.mouse_click(MouseButton::Left);
        let ruler = self.get_ruler();
        let pool = calc_pool(&ruler);
        self.scroll(1);
        let mut ruler_new = self.get_ruler();
        let mut pool_new = calc_pool(&ruler_new);
        utils::sleep(1000);
        loop {
            ruler_new = self.get_ruler();
            let pool_newer = calc_pool(&ruler_new);
            if (pool_newer - pool_new).abs() < 0.000001 {
                break;
            }
            pool_new = pool_newer;
            utils::sleep(500);
        }
        // println!("{} {}", pool, pool_new);
        if (pool - calc_pool(&ruler)).abs() > 0.000001 {
            utils::error_and_quit("请翻到圣遗物界面顶部");
        }
    }

    fn scroll_to_top(&mut self) {
        self.move_to(0, 1);
        let mut ruler = self.get_ruler();
        'outer: loop {
            self.scroll(100);
            utils::sleep(2000);
            let ruler_new = self.get_ruler();
            for i in 0..ruler.len() {
                if ruler[i] != ruler_new[i] {
                    ruler = ruler_new;
                    continue 'outer;
                }
            }
            break;
        }
    }

    fn get_scroll_speed(&mut self) {
        //   4321
        // 6543
        let ruler = self.get_ruler();
        self.scroll(-1);
        utils::sleep(400);
        let ruler_new = self.get_ruler();
        // println!("{:?}", ruler);
        // println!("{:?}", ruler_new);
        'outer: for i in (0..ruler.len()).step_by(4) {
            if eq(ruler_new[i], ruler[0], 5) {
                // println!("{}", i);
                for j in 0..(ruler.len() - i) {
                    if !eq(ruler_new[i + j], ruler[j], 5) {
                        // println!("fail at {}", j);
                        continue 'outer;
                    }
                }
                self.pixel_per_scroll = (i as u32) / 4;
                break 'outer;
            }
        }
        if self.pixel_per_scroll == 0 {
            utils::error_and_quit("检测滚动速度失败");
        }
        self.scroll(1);
        utils::sleep(400);
        info!("pixels per scroll: {}", self.pixel_per_scroll);
    }

    fn get_art_count(&mut self) -> Result<u32, String> {
        let count = self.config.number;
        if let 0 = count {
            let info = &self.info;
            let raw_after_pp = self.info.art_count_position.capture_relative(info).unwrap();
            // raw_after_pp.to_gray_image().save("count.png");
            let s = self.model.inference_string(&raw_after_pp);
            info!("raw count string: {}", s);
            if s.starts_with("圣遗物") {
                let chars = s.chars().collect::<Vec<char>>();
                let count_str = (&chars[4..chars.len() - 5]).iter().collect::<String>();
                let count = match count_str.parse::<u32>() {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(String::from("无法识别圣遗物数量"));
                    }
                };
                return Ok(count);
            }
            Err(String::from("无法识别圣遗物数量"))
        } else {
            return Ok(count);
        }
    }

    fn scroll_one_row(&mut self) -> ScrollResult {
        let mut state = 0;
        let mut count = 0;
        let max_scroll = 20;
        while count < max_scroll {
            if utils::is_rmb_down() {
                return ScrollResult::Interrupt;
            }

            self.enigo.mouse_scroll_y(-5);
            utils::sleep(self.config.scroll_stop);
            count += 1;
            let color: Color = self.get_flag_color();
            // println!("{:?}", color);
            if state == 0 && !color.is_same(&self.initial_color) {
                state = 1;
            } else if state == 1 && self.initial_color.is_same(&color) {
                self.avg_scroll_one_row = (self.avg_scroll_one_row * self.scrolled_rows as f64
                    + count as f64)
                    / (self.scrolled_rows as f64 + 1.0);
                info!("avg scroll/row: {}", self.avg_scroll_one_row);
                self.scrolled_rows += 1;
                return ScrollResult::Success;
            }
        }

        ScrollResult::TLE
    }

    fn scroll_rows(&mut self, count: u32) -> ScrollResult {
        let total_pixels =
            self.offset_y + (count * (self.info.art_height + self.info.art_gap_y)) as i32;
        let total_scrolls = (total_pixels as f64 / self.pixel_per_scroll as f64).round() as i32;
        self.offset_y = total_pixels - total_scrolls * self.pixel_per_scroll as i32;
        self.scroll(-total_scrolls);
        /*
        if self.pixel_per_scroll > 0 {
        } else {
            if self.scrolled_rows >= 5 {
                let scroll = ((self.avg_scroll_one_row * count as f64 - 3.0).round() as u32).max(0);
                for _ in 0..scroll {
                    self.enigo.mouse_scroll_y(-1);
                }
                utils::sleep(400);
                self.align_row();
                return ScrollResult::Skip;
            }

            for _ in 0..count {
                match self.scroll_one_row() {
                    ScrollResult::TLE => return ScrollResult::TLE,
                    ScrollResult::Interrupt => return ScrollResult::Interrupt,
                    _ => (),
                }
            }
        }
        */

        ScrollResult::Success
    }

    fn align_row(&mut self) -> bool {
        let mut count = 0;
        while count < 10 {
            let color = self.get_flag_color();
            if color.is_same(&self.initial_color) {
                return true;
            }

            self.enigo.mouse_scroll_y(-1);
            utils::sleep(self.config.scroll_stop);
            count += 1;
        }

        false
    }

    fn wait_until_switched(&mut self) -> bool {
        let now = SystemTime::now();
        let mut consecutive_time = 0;
        let mut diff_flag = false;
        while now.elapsed().unwrap().as_millis() < self.config.max_wait_switch_artifact as u128 {
            // let pool_start = SystemTime::now();
            let rect = PixelRect {
                left: self.info.left as i32 + self.info.pool_position.left,
                top: self.info.top as i32 + self.info.pool_position.top,
                width: self.info.pool_position.right - self.info.pool_position.left,
                height: self.info.pool_position.bottom - self.info.pool_position.top,
            };
            let im = capture::capture_absolute(&rect).unwrap();
            let pool = calc_pool(&im);
            // info!("pool: {}", pool);
            // println!("pool time: {}ms", pool_start.elapsed().unwrap().as_millis());

            if (pool - self.pool).abs() > 0.000001 {
                // info!("pool: {}", pool);
                // let raw = RawCaptureImage {
                //     data: im,
                //     w: rect.width as u32,
                //     h: rect.height as u32,
                // };
                // let raw = raw.to_raw_image();
                // println!("{:?}", &raw.data[..10]);
                // raw.save(&format!("captures/{}.png", rand::thread_rng().gen::<u32>()));

                self.pool = pool;
                diff_flag = true;
                consecutive_time = 0;
            // info!("avg switch time: {}ms", self.avg_switch_time);
            } else {
                if diff_flag {
                    consecutive_time += 1;
                    if consecutive_time == 1 {
                        self.avg_switch_time = (self.avg_switch_time * self.scanned_count as f64
                            + now.elapsed().unwrap().as_millis() as f64)
                            / (self.scanned_count as f64 + 1.0);
                        self.scanned_count += 1;
                        return true;
                    }
                }
            }
        }

        false
    }

    fn capture_panel(&mut self) -> Result<RawCaptureImage, String> {
        let w = self.info.panel_position.right - self.info.panel_position.left;
        let h = self.info.panel_position.bottom - self.info.panel_position.top;
        let rect: PixelRect = PixelRect {
            left: self.info.left as i32 + self.info.panel_position.left,
            top: self.info.top as i32 + self.info.panel_position.top,
            width: w,
            height: h,
        };
        let u8_arr = capture::capture_absolute(&rect)?;
        // info!("capture time: {}ms", now.elapsed().unwrap().as_millis());
        Ok(RawCaptureImage {
            data: u8_arr,
            w: w as u32,
            h: h as u32,
        })
    }

    fn get_star(&self) -> u32 {
        let color = capture::get_color(
            (self.info.star_x as i32 + self.info.left) as u32,
            (self.info.star_y as i32 + self.info.top) as u32,
        );

        let color_1 = Color::from(113, 119, 139);
        let color_2 = Color::from(42, 143, 114);
        let color_3 = Color::from(81, 127, 203);
        let color_4 = Color::from(161, 86, 224);
        let color_5 = Color::from(188, 105, 50);

        let min_dis: u32 = color_1.dis_2(&color);
        let mut star = 1_u32;
        if color_2.dis_2(&color) < min_dis {
            star = 2;
        }
        if color_3.dis_2(&color) < min_dis {
            star = 3;
        }
        if color_4.dis_2(&color) < min_dis {
            star = 4;
        }
        if color_5.dis_2(&color) < min_dis {
            star = 5;
        }

        star
    }

    fn get_lock(&self, lock_last: bool) -> bool {
        let color = capture::get_color(
            (self.info.lock_x as i32 + self.info.left) as u32,
            (self.info.lock_y as i32 + self.info.top) as u32,
        );
        // info!("Lock color: {} {} {}", color.0, color.1, color.2);

        let color_t = Color::from(73, 83, 102);
        let color_f = Color::from(241, 237, 232);

        if color_t.dis_2(&color) <= 3 {
            return true;
        } else if color_f.dis_2(&color) <= 3 {
            return false;
        } else {
            return !lock_last; // switch animation
        }
    }

    fn get_locks(&mut self, start_row: u32) -> Vec<bool> {
        self.enigo
            .mouse_move_to(self.info.left + 10, self.info.top + 10);
        self.enigo.mouse_click(MouseButton::Left);
        utils::sleep(100);
        let mut locks: Vec<bool> = Vec::new();
        let info = &self.info;
        for row in start_row..self.row {
            let y = info.top_margin + row * (info.art_height + info.art_gap_y) + info.art_lock_y;
            let y = (y as i32 + self.offset_y) as u32;
            for col in 0..self.col {
                let x =
                    info.left_margin + col * (info.art_width + info.art_gap_x) + info.art_lock_x;
                let color = self.get_color(x, y);
                locks.push(Color::from(255, 138, 117).dis_2(&color) < 1);
            }
        }
        locks
    }

    fn start_capture_only(&mut self) {
        fs::create_dir("captures").expect("Create dir error");
        let info = &self.info.clone();

        let count = self.info.art_count_position.capture_relative(info).unwrap();
        count
            .to_gray_image()
            .save("captures/count.png")
            .expect("Save png error");

        let convert_rect = |rect: &PixelRectBound| PixelRect {
            left: rect.left - info.panel_position.left,
            top: rect.top - info.panel_position.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        };

        let panel = self.capture_panel().unwrap();
        let im_title = pre_process(panel.crop_to_raw_img(&convert_rect(&info.title_position)));
        im_title
            .to_gray_image()
            .save("captures/title.png")
            .expect("Err");
        let im_main_stat_name =
            pre_process(panel.crop_to_raw_img(&convert_rect(&info.main_stat_name_position)));
        im_main_stat_name
            .to_gray_image()
            .save("captures/main_stat_name.png")
            .expect("Err");
        let im_main_stat_value =
            pre_process(panel.crop_to_raw_img(&convert_rect(&info.main_stat_value_position)));
        im_main_stat_value
            .to_gray_image()
            .save("captures/main_stat_value.png")
            .expect("Err");
        let im_sub_stat_1 =
            pre_process(panel.crop_to_raw_img(&convert_rect(&info.sub_stat1_position)));
        im_sub_stat_1
            .to_gray_image()
            .save("captures/sub_stat_1.png")
            .expect("Err");
        let im_sub_stat_2 =
            pre_process(panel.crop_to_raw_img(&convert_rect(&info.sub_stat2_position)));
        im_sub_stat_2
            .to_gray_image()
            .save("captures/sub_stat_2.png")
            .expect("Err");
        let im_sub_stat_3 =
            pre_process(panel.crop_to_raw_img(&convert_rect(&info.sub_stat3_position)));
        im_sub_stat_3
            .to_gray_image()
            .save("captures/sub_stat_3.png")
            .expect("Err");
        let im_sub_stat_4 =
            pre_process(panel.crop_to_raw_img(&convert_rect(&info.sub_stat4_position)));
        im_sub_stat_4
            .to_gray_image()
            .save("captures/sub_stat_4.png")
            .expect("Err");
        let im_level = pre_process(panel.crop_to_raw_img(&convert_rect(&info.level_position)));
        im_level
            .to_gray_image()
            .save("captures/level.png")
            .expect("Err");
        let im_equip = pre_process(panel.crop_to_raw_img(&convert_rect(&info.equip_position)));
        im_equip
            .to_gray_image()
            .save("captures/equip.png")
            .expect("Err");
    }

    pub fn scan(&mut self) -> Vec<InternalArtifact> {
        self.check_menu();
        self.check_at_top();
        self.get_scroll_speed();

        if self.config.capture_only {
            self.start_capture_only();
            return Vec::new();
        }

        let count = match self.get_art_count() {
            Ok(v) => v,
            Err(_) => 1500,
        };

        let total_row = (count + self.col - 1) / self.col;
        let last_row_col = if count % self.col == 0 {
            self.col
        } else {
            count % self.col
        };

        // println!("检测到圣遗物数量：{}，若无误请按回车，否则输入正确的圣遗物数量：", count);
        // let mut s: String = String::new();
        // stdin().read_line(&mut s);
        // if s.trim() != "" {
        //     count = s.trim().parse::<u32>().unwrap();
        // }

        // info!("detected count: {}", count);
        // info!("total row: {}", total_row);
        // info!("last column: {}", last_row_col);

        let (tx, rx) = mpsc::channel::<Option<(RawCaptureImage, u32, bool)>>();
        let info_2 = self.info.clone();
        // v bvvmnvbm
        let is_verbose = self.config.verbose;
        let is_dump_mode = self.config.dump_mode;
        let min_level = self.config.min_level;
        let handle = thread::spawn(move || {
            let mut results: Vec<InternalArtifact> = Vec::new();
            let model = CRNNModel::new(
                String::from("model_training.onnx"),
                String::from("index_2_word.json"),
            );
            let mut error_count = 0;
            let mut dup_count = 0;
            let mut hash = HashSet::new();
            let mut consecutive_dup_count = 0;
            let info = info_2;

            let mut cnt = 0;
            if is_dump_mode {
                fs::create_dir("dumps").expect("Err");
            }

            let convert_rect = |rect: &PixelRectBound| PixelRect {
                left: rect.left - info.panel_position.left,
                top: rect.top - info.panel_position.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            };

            for i in rx {
                let (capture, rarity, lock) = match i {
                    Some(v) => v,
                    None => break,
                };
                // let now = SystemTime::now();

                let model_inference = |pos: &PixelRectBound, name: &str, cnt: i32| {
                    let raw_img = capture.crop_to_raw_img(&convert_rect(pos));
                    if is_dump_mode {
                        raw_img
                            .grayscale_to_gray_image()
                            .save(format!("dumps/{}_{}.png", name, cnt))
                            .expect("Err");
                    }

                    let processed_img = pre_process(raw_img);
                    if is_dump_mode {
                        processed_img
                            .to_gray_image()
                            .save(format!("dumps/p_{}_{}.png", name, cnt))
                            .expect("Err");
                    }
                    let inference_result = model.inference_string(&processed_img);
                    if is_dump_mode {
                        fs::write(format!("dumps/{}_{}.txt", name, cnt), &inference_result)
                            .expect("Err");
                    }

                    inference_result
                };

                let str_title = model_inference(&info.title_position, "title", cnt);
                let str_main_stat_name =
                    model_inference(&info.main_stat_name_position, "main_stat_name", cnt);
                let str_main_stat_value =
                    model_inference(&info.main_stat_value_position, "main_stat_value", cnt);

                let str_sub_stat_1 = model_inference(&info.sub_stat1_position, "sub_stat_1", cnt);
                let str_sub_stat_2 = model_inference(&info.sub_stat2_position, "sub_stat_2", cnt);
                let str_sub_stat_3 = model_inference(&info.sub_stat3_position, "sub_stat_3", cnt);
                let str_sub_stat_4 = model_inference(&info.sub_stat4_position, "sub_stat_4", cnt);

                let str_level = model_inference(&info.level_position, "level", cnt);
                let str_equip = model_inference(&info.equip_position, "equip", cnt);

                cnt += 1;

                // let predict_time = now.elapsed().unwrap().as_millis();
                // println!("predict time: {}ms", predict_time);

                let result = YasScanResult {
                    name: str_title,
                    main_stat_name: str_main_stat_name,
                    main_stat_value: str_main_stat_value,
                    sub_stat_1: str_sub_stat_1,
                    sub_stat_2: str_sub_stat_2,
                    sub_stat_3: str_sub_stat_3,
                    sub_stat_4: str_sub_stat_4,
                    level: str_level,
                    location: str_equip,
                    rarity,
                    lock,
                };
                if is_verbose {
                    info!("{:?}", result);
                }
                // println!("{:?}", result);
                let art = result.to_internal_artifact();
                if let Some(a) = art {
                    if hash.contains(&a) {
                        dup_count += 1;
                        consecutive_dup_count += 1;
                        warn!("dup artifact detected: {:?}", result);
                    } else {
                        consecutive_dup_count = 0;
                        hash.insert(a.clone());
                        // results.push(a);
                    }
                    results.push(a);
                } else {
                    error!("wrong detection: {:?}", result);
                    error_count += 1;
                    // println!("error parsing results");
                }
                if consecutive_dup_count >= info.art_row {
                    error!("检测到连续多个重复圣遗物，可能为翻页错误，或者为非背包顶部开始扫描");
                    break;
                }
            }

            info!("error count: {}", error_count);
            info!("dup count: {}", dup_count);

            if min_level > 0 {
                results
                    .into_iter()
                    .filter(|result| result.level >= min_level)
                    .collect::<Vec<_>>()
            } else {
                results
            }
        });

        let mut scanned_row = 0_u32;
        let mut scanned_count = 0_u32;
        let mut start_row = 0_u32;
        // let mut lock = false;

        // self.move_to(0, 0);
        // self.enigo.mouse_click(MouseButton::Left);
        // utils::sleep(1000);
        // self.sample_initial_color();

        'outer: while scanned_count < count {
            let locks = self.get_locks(start_row);
            let mut locks_idx: usize = 0;
            // println!("{:?}", locks);
            for row in start_row..self.row {
                let c = if scanned_row == total_row - 1 {
                    last_row_col
                } else {
                    self.col
                };
                for col in 0..c {
                    // 大于最大数量则退出
                    if scanned_count > count {
                        break 'outer;
                    }

                    // 右键终止
                    if utils::is_rmb_down() {
                        break 'outer;
                    }

                    self.move_to(row, col);
                    self.enigo.mouse_click(MouseButton::Left);

                    self.wait_until_switched();
                    // utils::sleep(80);

                    let capture = self.capture_panel().unwrap();
                    let star = self.get_star();
                    if star < self.config.min_star {
                        break 'outer;
                    }
                    // lock = self.get_lock(lock);
                    let lock = locks[locks_idx];
                    locks_idx += 1;
                    tx.send(Some((capture, star, lock))).unwrap();

                    scanned_count += 1;
                } // end 'col

                // info!("{:?}", locks);
                scanned_row += 1;

                if scanned_row >= self.config.max_row {
                    info!("max row reached, quiting...");
                    break 'outer;
                }
            } // end 'row

            let remain = count - scanned_count;
            let remain_row = (remain + self.col - 1) / self.col;
            let scroll_row = remain_row.min(self.row);
            start_row = self.row - scroll_row;
            match self.scroll_rows(scroll_row) {
                ScrollResult::TLE => {
                    error!("翻页出现问题");
                    break 'outer;
                }
                ScrollResult::Interrupt => break 'outer,
                _ => (),
            }

            utils::sleep(100);
        }

        tx.send(None).unwrap();

        info!("扫描结束，等待识别线程结束，请勿关闭程序");
        let results: Vec<InternalArtifact> = handle.join().unwrap();
        info!("count: {}", results.len());
        results
    }
    pub fn flip_lock(&mut self, indices: Vec<u32>) {
        self.check_menu();
        self.check_at_top();
        self.get_scroll_speed();
        let mut indices = indices;
        indices.sort();

        let count = match self.get_art_count() {
            Ok(v) => v,
            Err(_) => 1500,
        };
        if indices[indices.len() - 1] > count {
            error!("指标超出范围");
            return;
        }
        self.sample_initial_color();

        let total_row = (count + self.col - 1) / self.col;
        let mut scanned_row = 0_u32;
        let mut start_row = 0_u32;

        // self.move_to(0, 0);
        // self.enigo.mouse_click(MouseButton::Left);
        // utils::sleep(1000);
        // self.sample_initial_color();

        for index in indices {
            let row: u32 = index / self.col;
            let col: u32 = index % self.col;
            while row >= scanned_row + self.row {
                scanned_row += self.row;
                let remain_row = total_row - scanned_row;
                let scroll_row = remain_row.min(self.row);
                start_row = self.row - scroll_row;
                match self.scroll_rows(scroll_row) {
                    ScrollResult::TLE => {
                        error!("翻页出现问题");
                        break;
                    }
                    ScrollResult::Interrupt => break,
                    _ => (),
                }
                // 右键终止
                if utils::is_rmb_down() {
                    break;
                }
                utils::sleep(100);
            }
            // 右键终止
            if utils::is_rmb_down() {
                break;
            }
            // info!("{} {} {}", index, row, col);

            self.move_to(row - scanned_row + start_row, col);
            self.enigo.mouse_click(MouseButton::Left);
            self.wait_until_switched();
            // utils::sleep(100);

            let left: i32 = self.info.left + self.info.lock_x as i32;
            let top: i32 = self.info.top + self.info.lock_y as i32;
            self.enigo.mouse_move_to(left, top);
            self.enigo.mouse_click(MouseButton::Left);
            utils::sleep(100);
            self.move_to(row - scanned_row + start_row, col);
        }
    }
    pub fn capture_game(&self, filename: &str) {
        let rect: PixelRect = PixelRect {
            left: self.info.left as i32,
            top: self.info.top as i32,
            width: self.info.width as i32,
            height: self.info.height as i32,
        };
        capture::capture_absolute_image(&rect)
            .unwrap()
            .save(filename)
            .expect("error");
    }
    pub fn test(&mut self) {
        self.check_menu();
        self.check_at_top();
        self.get_scroll_speed();
        /*
        self.move_to(0, 1);
        self.enigo.mouse_click(MouseButton::Left);
        let mut last_offset = 0;
        for offset in [0, 1, 5, 10] {
            println!("{} {}", offset, offset - last_offset);
            self.enigo.mouse_scroll_y(last_offset - offset);
            utils::sleep(400);
            self.capture_game(&format!("origin_{:02}.png", offset));
            last_offset = offset;
            // print
            let rect = PixelRect {
                left: self.info.left as i32 + 150,
                top: self.info.top as i32 + 120,
                width: 1,
                height: 80,
            };
            let u8_arr = capture::capture_absolute(&rect).unwrap();
            println!("{:?}", u8_arr);
        }
        */
    }
}
