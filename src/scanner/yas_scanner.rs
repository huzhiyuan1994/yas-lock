use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::convert::From;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::SystemTime;
// use tract_onnx::tract_core::downcast_rs::Downcast;

use clap::ArgMatches;
use dxgcap::DXGIManager;
use enigo::*;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::artifact::internal_artifact::{
    ArtifactSetKey, ArtifactSlotKey, ArtifactStat, CharacterKey, InternalArtifact,
};
use crate::capture::{self, capture_absolute_raw_image};
use crate::common::color::Color;
use crate::common::{utils, PixelRect, RawCaptureImage};
use crate::inference::inference::CRNNModel;
use crate::inference::pre_process::pre_process;
use crate::info::info::ScanInfo;

#[derive(Debug, Default, Deserialize, Serialize)]
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
    speed: u32,
    no_check: bool,
    max_wait_scroll: u32,
    mark: bool,
    dxgcap: bool,
    default_stop: u32,
    lock_stop: u32,
    yun: bool,
    scroll_speed: f64,
}

impl YasScannerConfig {
    pub fn from_match(matches: &ArgMatches) -> Result<YasScannerConfig> {
        Ok(YasScannerConfig {
            max_row: *matches.get_one("max-row").unwrap(),
            capture_only: matches.get_flag("capture-only"),
            dump_mode: matches.get_flag("dump"),
            mark: matches.get_flag("mark"),
            min_star: *matches.get_one("min-star").unwrap(),
            min_level: *matches.get_one("min-level").unwrap(),
            max_wait_switch_artifact: *matches.get_one("max-wait-switch-artifact").unwrap(),
            scroll_stop: *matches.get_one("scroll-stop").unwrap(),
            number: *matches.get_one("number").unwrap(),
            verbose: matches.get_flag("verbose"),
            speed: *matches.get_one("speed").unwrap(),
            no_check: matches.get_flag("no-check"),
            max_wait_scroll: *matches.get_one("max-wait-scroll").unwrap(),
            dxgcap: matches.get_flag("dxgcap"),
            default_stop: *matches.get_one("default-stop").unwrap(),
            lock_stop: *matches.get_one("lock-stop").unwrap(),
            yun: matches.get_one::<String>("window").unwrap().to_string() != String::from("原神"),
            scroll_speed: *matches.get_one("scroll-speed").unwrap(),
        })
    }
}

#[allow(dead_code)]
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
            (self.main_stat_name.replace("+", "?") + "+" + self.main_stat_value.as_str()).as_str(),
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

fn eq(x: u8, y: u8, threshold: u8) -> bool {
    if x < y {
        y - x <= threshold
    } else if x > y {
        x - y <= threshold
    } else {
        true
    }
}

fn get_pool_of_rect(shot: &RawCaptureImage, rect: &PixelRect) -> Result<f64> {
    let mut pool = 0_f64;
    for x in 0..rect.width {
        for y in 0..rect.height {
            pool += shot
                .get_color((rect.left + x) as u32, (rect.top + y) as u32)?
                .1 as f64;
        }
    }
    Ok(pool)
}

pub struct YasScanner {
    model: CRNNModel,
    enigo: Enigo,
    dxg: Option<DXGIManager>,

    info: ScanInfo,
    config: YasScannerConfig,

    row: u32,
    col: u32,

    pool: f64,

    // for scrolls
    scrolled_rows: u32,

    avg_switch_time: f64,
    scanned_count: u32,

    pixels_per_scroll: f64,
    offset_y: f64,
}

impl YasScanner {
    pub fn new(info: ScanInfo, config: YasScannerConfig) -> Result<YasScanner> {
        let row = info.art_row;
        let col = info.art_col;

        let mut dxg = None;
        if config.dxgcap {
            dxg = Some(DXGIManager::new(1000).map_err(|s| anyhow!(s))?);
            // dxg的第一张截图可能是黑屏
            dxg.as_mut()
                .unwrap()
                .capture_frame()
                .map_err(|e| anyhow!("dxg capture init err: {:?}", e))?;
        }

        Ok(YasScanner {
            model: CRNNModel::new(
                String::from("model_training.onnx"),
                String::from("index_2_word.json"),
            )?,
            enigo: Enigo::new(),
            dxg,

            info,
            config,

            row,
            col,

            pool: -1.0,
            scrolled_rows: 0,

            avg_switch_time: 0.0,
            scanned_count: 0,

            pixels_per_scroll: 0.0,
            offset_y: 0.0,
        })
    }
}

impl YasScanner {
    fn align_panel(&mut self) {
        let left: i32 = self.info.left + self.info.lock_x as i32;
        let top: i32 = self.info.top + self.info.lock_y as i32;
        self.enigo.mouse_move_to(left, top);
        self.scroll(10);
    }

    fn capture(&mut self, rect: &PixelRect) -> Result<RawCaptureImage> {
        if self.config.dxgcap {
            let (pixels, (w, _)) = self
                .dxg
                .as_mut()
                .unwrap()
                .capture_frame()
                .map_err(|e| anyhow!("dxg capture err: {:?}", e))?;

            let mut im = RawCaptureImage {
                data: vec![0; (rect.width * rect.height * 4) as usize],
                w: rect.width as u32,
                h: rect.height as u32,
            };

            for x in rect.left..rect.left + rect.width {
                for y in rect.top..rect.top + rect.height {
                    let p = (y * w as i32 + x) as usize;
                    let pos = ((rect.height - 1 - (y - rect.top)) * rect.width + (x - rect.left))
                        as usize
                        * 4;
                    im.data[pos + 0] = pixels[p].b;
                    im.data[pos + 1] = pixels[p].g;
                    im.data[pos + 2] = pixels[p].r;
                    im.data[pos + 3] = pixels[p].a;
                }
            }
            return Ok(im);
        }

        capture_absolute_raw_image(&rect)
    }

    fn move_to(&mut self, row: u32, col: u32) {
        let info = &self.info;
        let left = info.left
            + info.left_margin as i32
            + info.art_width as i32 / 2
            + (info.art_shift_x * col as f64) as i32;
        let top = info.top
            + info.top_margin as i32
            + info.art_height as i32 / 2
            + (info.art_shift_y * row as f64) as i32;
        self.enigo.mouse_move_to(left, top);
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
        utils::sleep(self.config.scroll_stop);
    }

    fn get_color(&self, x: u32, y: u32) -> Result<Color> {
        let x = x as i32 + self.info.left;
        let y = y as i32 + self.info.top;
        let color = capture::get_color(x as u32, y as u32)?;

        Ok(color)
    }

    fn get_ruler(&mut self) -> Result<RawCaptureImage> {
        let rect = PixelRect {
            left: self.info.left + self.info.ruler_left as i32,
            top: self.info.top + self.info.ruler_top as i32,
            width: 1,
            height: self.info.ruler_height as i32,
        };
        self.capture(&rect)
    }

    fn get_pool(&mut self, shot: &RawCaptureImage) -> Result<f64> {
        let mut pool = 0_f64;
        pool += get_pool_of_rect(&shot, &self.info.sub_stat1_position)?;
        pool += get_pool_of_rect(&shot, &self.info.sub_stat2_position)?;
        pool += get_pool_of_rect(&shot, &self.info.sub_stat3_position)?;
        pool += get_pool_of_rect(&shot, &self.info.sub_stat4_position)?;
        Ok(pool)
    }

    fn check_menu(&self) -> Result<()> {
        if self.config.no_check {
            return Ok(());
        }
        let color = self.get_color(self.info.menu_x, self.info.menu_y)?;
        // if Color::from(236, 229, 216).dis_2(&color) > 0 {
        if color.0 < 200 {
            return Err(anyhow!("请打开背包圣遗物栏"));
        }
        Ok(())
    }

    fn scroll_to_top(&mut self) -> Result<()> {
        let rect = PixelRect {
            left: self.info.left + self.info.scrollbar_left as i32,
            top: self.info.top + self.info.scrollbar_top as i32,
            width: 1,
            height: self.info.scrollbar_height as i32,
        };
        let pixels = self.capture(&rect)?.data;
        // println!("{:?}", pixels);
        let mut offset = 0;
        let mut color_last = pixels[0] as i32 + pixels[1] as i32 + pixels[2] as i32;
        let mut delta_max = 0_i32;
        for i in 1..rect.height as usize {
            let color = pixels[4 * i] as i32 + pixels[4 * i + 1] as i32 + pixels[4 * i + 2] as i32;
            let delta = (color - color_last).abs();
            if delta > delta_max {
                delta_max = delta;
                offset = rect.height - if color > color_last { i } else { i - 1 } as i32;
            }
            color_last = color;
            // println!(
            //     "{} {} {} {}",
            //     pixels[4 * i],
            //     pixels[4 * i + 1],
            //     pixels[4 * i + 2],
            //     pixels[4 * i + 3]
            // );
        }
        self.enigo.mouse_move_to(rect.left, rect.top + offset);
        self.enigo.mouse_down(MouseButton::Left);
        utils::sleep(self.config.default_stop);
        self.enigo.mouse_move_to(rect.left, self.info.top + 10);
        utils::sleep(self.config.default_stop);
        self.enigo.mouse_up(MouseButton::Left);
        Ok(())
    }

    fn get_scroll_speed(&mut self) -> Result<()> {
        if self.config.yun {
            self.pixels_per_scroll = self.config.scroll_speed;
            return Ok(());
        }

        self.create_dumps_folder()?;
        // move focus to the first artifact
        self.move_to(0, 0);
        self.enigo.mouse_click(MouseButton::Left);
        utils::sleep(self.config.default_stop);
        // match ruler and ruler_shift to get scroll speed
        let ruler = self.get_ruler()?.data;
        fs::write("dumps/scroll_0.txt", format!("{:?}", ruler))
            .map_err(|_| anyhow!("fail to write scroll_0.txt"))?;
        // scroll until rulers are matched
        // this is because some pixels are mixed after scrolling
        'scroll: for n_scroll in 1..=5 {
            self.scroll(-1);
            let ruler_shift = self.get_ruler()?.data;
            fs::write(
                format!("dumps/scroll_{}.txt", n_scroll),
                format!("{:?}", ruler_shift),
            )
            .map_err(|_| anyhow!("fail to write scroll_x.txt"))?;
            //   4321
            // 6543
            'match_: for i in (4..ruler.len()).step_by(4) {
                for j in 0..(ruler.len() - i) {
                    if !eq(ruler_shift[i + j], ruler[j], 5) {
                        continue 'match_;
                    }
                }
                self.pixels_per_scroll = (i / 4) as f64 / (n_scroll as f64);
                // undo scrolls
                self.scroll(n_scroll);
                break 'scroll;
            }
        }
        if self.pixels_per_scroll < 1.0 {
            return Err(anyhow!("检测滚动速度失败"));
        }
        info!("pixels per scroll: {}", self.pixels_per_scroll);
        Ok(())
    }

    fn get_art_count(&mut self) -> Result<u32> {
        let count = self.config.number;
        if let 0 = count {
            let info = &self.info;
            let raw_after_pp = self.info.art_count_position.capture_relative(info)?;
            // raw_after_pp.to_gray_image().save("count.png");
            let s = self.model.inference_string(&raw_after_pp)?;
            info!("raw count string: {}", s);
            if s.starts_with("圣遗物") {
                let chars = s.chars().collect::<Vec<char>>();
                let count_str = (&chars[4..chars.len() - 5]).iter().collect::<String>();
                let count = match count_str.parse::<u32>() {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(anyhow!("无法识别圣遗物数量"));
                    }
                };
                return Ok(count);
            }
            Err(anyhow!("无法识别圣遗物数量"))
        } else {
            return Ok(count);
        }
    }

    fn scroll_rows(&mut self, count: u32) -> Result<()> {
        if count == 0 {
            return Ok(());
        }

        let total_pixels = self.offset_y + self.info.art_shift_y * count as f64;
        let total_scrolls = (total_pixels / self.pixels_per_scroll).round();
        self.offset_y = total_pixels - total_scrolls * self.pixels_per_scroll;
        self.scroll(-total_scrolls as i32);
        self.scrolled_rows += count;

        if self.config.max_wait_scroll == 0 || count == 1 {
            return Ok(());
        }

        // wait until scrolled
        let rect = PixelRect {
            left: self.info.left + self.info.left_margin.round() as i32,
            top: self.info.top
                + (self.info.top_margin
                    + self.offset_y
                    + self.info.art_shift_y * (self.row - count) as f64)
                    .round() as i32,
            width: self.info.art_width as i32,
            height: self.info.art_height as i32,
        };
        let now = SystemTime::now();
        let mut delta = 1.0;
        while now.elapsed()?.as_millis() < self.config.max_wait_scroll as u128 {
            let shot = self.capture(&rect)?;
            let mut ratio = 0.0;
            for y in 0..(rect.height as u32) {
                if shot.get_color(5, y)?.eq(&Color(233, 229, 220)) {
                    ratio = y as f64 / rect.height as f64;
                    break;
                }
            }
            delta = (ratio - 0.822).abs();
            if delta < 0.02 {
                return Ok(());
                // } else {
                //     warn!(
                //         "翻页可能未完成，等待中 (row {}, delta {} > 0.02)",
                //         self.scrolled_rows, delta
                //     );
            }
        }

        self.create_dumps_folder()?;
        let shot = self.capture(&rect)?;
        shot.save("dumps/err_scroll.png")?;
        Err(anyhow!(
            "翻页超时 (time: {}, delta: {}, shot: dumps/err_scroll.png)",
            now.elapsed()?.as_millis(),
            delta
        ))
    }

    fn wait_until_switched(&mut self) -> Result<RawCaptureImage> {
        if self.config.yun {
            utils::sleep(self.config.default_stop);
            return self.capture_panel();
        }

        let now = SystemTime::now();
        let mut consecutive_time = 0;
        let mut diff_flag = false;
        let mut pools = Vec::new();

        while now.elapsed()?.as_millis() < self.config.max_wait_switch_artifact as u128 {
            let shot = self.capture_panel()?;
            let pool = self.get_pool(&shot)?;

            pools.push(pool);

            if (pool - self.pool).abs() > 0.000001 {
                self.pool = pool;
                diff_flag = true;
                consecutive_time = 0;
                // info!("avg switch time: {}ms", self.avg_switch_time);
            }
            if diff_flag {
                consecutive_time += 1;
                if consecutive_time + self.config.speed >= 6 {
                    self.avg_switch_time = (self.avg_switch_time * self.scanned_count as f64
                        + now.elapsed()?.as_millis() as f64)
                        / (self.scanned_count as f64 + 1.0);
                    self.scanned_count += 1;
                    if self.config.verbose {
                        info!("pools: {:?}", pools);
                    }
                    return Ok(shot);
                }
            }
        }

        warn!(
            "圣遗物切换超时 (time: {}ms)",
            now.elapsed()?.as_millis(),
            // pools
        );

        if self.config.verbose {
            info!("pools: {:?}", pools);
        }
        self.capture_panel()
    }

    fn capture_panel(&mut self) -> Result<RawCaptureImage> {
        let rect: PixelRect = PixelRect {
            left: self.info.left as i32 + self.info.panel_position.left,
            top: self.info.top as i32 + self.info.panel_position.top,
            width: self.info.panel_position.width,
            height: self.info.panel_position.height,
        };
        let shot = self.capture(&rect)?;
        // info!("capture time: {}ms", now.elapsed().unwrap().as_millis());
        Ok(shot)
    }

    fn get_star(&self, shot: &RawCaptureImage) -> Result<u32> {
        let color = shot.get_color(self.info.star_x, self.info.star_y)?;

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

        Ok(star)
    }

    fn create_dumps_folder(&self) -> Result<()> {
        if !Path::new("dumps").exists() {
            return fs::create_dir("dumps").map_err(|_| anyhow!("create dumps dir err"));
        }
        Ok(())
    }

    fn get_locks(&mut self, start_row: u32) -> Result<Vec<bool>> {
        // move focus out of all artifacts
        self.enigo
            .mouse_move_to(self.info.left + 10, self.info.top + 10);
        self.enigo.mouse_click(MouseButton::Left);
        utils::sleep(self.config.default_stop);
        // capture game screen
        let rect = PixelRect {
            left: self.info.left,
            top: self.info.top,
            width: self.info.width as i32,
            height: self.info.height as i32,
        };
        let mut shot = self.capture(&rect)?;
        let mut locks: Vec<bool> = Vec::new();
        let info = &self.info;
        for row in start_row..self.row {
            let y =
                (info.top_margin + self.offset_y + info.art_lock_y + info.art_shift_y * row as f64)
                    .round() as i32;
            for col in 0..self.col {
                let x = (info.left_margin + info.art_lock_x + info.art_shift_x * col as f64).round()
                    as i32;
                // 检测以(x, y)为中心的7x7方块内是否有锁的颜色
                let mut locked = false;
                'sq: for dx in -3..3 {
                    for dy in -3..3 {
                        let color = shot.get_color((x + dx) as u32, (y + dy) as u32)?;
                        // if Color::from(255, 138, 117).dis_2(&color) < 1 {
                        if color.0 > 200 {
                            locked = true;
                            break 'sq;
                        }
                    }
                }
                // mark: lock red / unlock green
                if self.config.mark {
                    for dx in -3..3 {
                        for dy in -3..3 {
                            if locked {
                                shot.set_color(
                                    (x + dx) as u32,
                                    (y + dy) as u32,
                                    &Color(255, 0, 0),
                                )?;
                            } else {
                                shot.set_color(
                                    (x + dx) as u32,
                                    (y + dy) as u32,
                                    &Color(0, 255, 0),
                                )?;
                            }
                        }
                    }
                }
                locks.push(locked);
            }
        }
        // dump marked screenshot for debug
        if self.config.mark {
            self.create_dumps_folder()?;
            shot.save(&format!("dumps/lock_{}.png", self.scrolled_rows))?;
        }
        Ok(locks)
    }

    fn start_capture_only(&mut self) -> Result<()> {
        fs::create_dir("captures")?;
        let info = &self.info.clone();

        let count = self.info.art_count_position.capture_relative(info)?;
        count.to_gray_image().save("captures/count.png")?;

        let panel = self.capture_panel()?;
        let im_title = pre_process(panel.crop_to_raw_img(&info.title_position));
        im_title.to_gray_image().save("captures/title.png")?;
        let im_main_stat_name = pre_process(panel.crop_to_raw_img(&info.main_stat_name_position));
        im_main_stat_name
            .to_gray_image()
            .save("captures/main_stat_name.png")?;
        let im_main_stat_value = pre_process(panel.crop_to_raw_img(&info.main_stat_value_position));
        im_main_stat_value
            .to_gray_image()
            .save("captures/main_stat_value.png")?;
        let im_sub_stat_1 = pre_process(panel.crop_to_raw_img(&info.sub_stat1_position));
        im_sub_stat_1
            .to_gray_image()
            .save("captures/sub_stat_1.png")?;
        let im_sub_stat_2 = pre_process(panel.crop_to_raw_img(&info.sub_stat2_position));
        im_sub_stat_2
            .to_gray_image()
            .save("captures/sub_stat_2.png")?;
        let im_sub_stat_3 = pre_process(panel.crop_to_raw_img(&info.sub_stat3_position));
        im_sub_stat_3
            .to_gray_image()
            .save("captures/sub_stat_3.png")?;
        let im_sub_stat_4 = pre_process(panel.crop_to_raw_img(&info.sub_stat4_position));
        im_sub_stat_4
            .to_gray_image()
            .save("captures/sub_stat_4.png")?;
        let im_level = pre_process(panel.crop_to_raw_img(&info.level_position));
        im_level.to_gray_image().save("captures/level.png")?;
        let im_equip = pre_process(panel.crop_to_raw_img(&info.equip_position));
        im_equip.to_gray_image().save("captures/equip.png")?;
        Ok(())
    }

    pub fn screenshot_and_mark(&mut self) -> Result<()> {
        if !self.config.mark {
            return Ok(());
        }
        // take screenshot
        let rect = PixelRect {
            left: self.info.left,
            top: self.info.top,
            width: self.info.width as i32,
            height: self.info.height as i32,
        };
        let mut shot = self.capture(&rect)?;

        // mark
        let a = |rect: &PixelRect| PixelRect {
            left: rect.left + self.info.panel_position.left,
            top: rect.top + self.info.panel_position.top,
            width: rect.width,
            height: rect.height,
        };
        let mark_color = Color(255, 0, 0);
        let alpha = 0.3;
        shot.mark(&self.info.panel_position, &mark_color, alpha)?;
        shot.mark(&a(&self.info.title_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.main_stat_name_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.main_stat_value_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.sub_stat1_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.sub_stat2_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.sub_stat3_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.sub_stat4_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.level_position), &mark_color, alpha)?;
        shot.mark(&a(&self.info.equip_position), &mark_color, alpha)?;
        shot.mark(&self.info.art_count_position.to_rect(), &mark_color, alpha)?;
        shot.set_color(self.info.menu_x, self.info.menu_y, &mark_color)?;
        shot.mark(
            &PixelRect {
                left: self.info.scrollbar_left as i32,
                top: self.info.scrollbar_top as i32,
                width: 1,
                height: self.info.scrollbar_height as i32,
            },
            &mark_color,
            alpha,
        )?;
        // save
        self.create_dumps_folder()?;
        shot.save(&format!(
            "dumps/{}x{}.png",
            self.info.width, self.info.height
        ))?;
        Ok(())
    }

    pub fn scan(&mut self) -> Result<Vec<InternalArtifact>> {
        self.align_panel();
        self.check_menu()?;
        self.scroll_to_top()?;
        self.get_scroll_speed()?;
        self.screenshot_and_mark()?;

        if self.config.capture_only {
            self.start_capture_only()?;
            return Ok(Vec::new());
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

        let (tx, rx) = mpsc::channel::<Option<(RawCaptureImage, u32, bool)>>();
        let info_2 = self.info.clone();
        // v bvvmnvbm
        let is_verbose = self.config.verbose;
        let is_dump_mode = self.config.dump_mode;
        let min_level = self.config.min_level;
        let handle = thread::spawn(move || -> Result<Vec<InternalArtifact>> {
            let mut results: Vec<InternalArtifact> = Vec::new();
            let model = CRNNModel::new(
                String::from("model_training.onnx"),
                String::from("index_2_word.json"),
            )?;
            let mut error_count = 0;
            let mut dup_count = 0;
            let mut hash = HashSet::new();
            let mut consecutive_dup_count = 0;
            let info = info_2;

            let mut cnt = 0;
            if is_dump_mode {
                fs::create_dir("dumps")?;
            }

            for i in rx {
                let (capture, rarity, lock) = match i {
                    Some(v) => v,
                    None => break,
                };
                // let now = SystemTime::now();

                let model_inference = |pos: &PixelRect, name: &str, cnt: i32| -> Result<String> {
                    let raw_img = capture.crop_to_raw_img(&pos);
                    if is_dump_mode {
                        raw_img
                            .grayscale_to_gray_image()
                            .save(format!("dumps/{}_{}.png", name, cnt))?;
                    }

                    let processed_img = pre_process(raw_img);

                    if processed_img.w == 0 || processed_img.h == 0 {
                        return Ok(String::from(""));
                    }

                    if is_dump_mode {
                        processed_img
                            .to_gray_image()
                            .save(format!("dumps/p_{}_{}.png", name, cnt))?;
                    }
                    let inference_result = model.inference_string(&processed_img)?;
                    if is_dump_mode {
                        fs::write(format!("dumps/{}_{}.txt", name, cnt), &inference_result)?;
                    }

                    Ok(inference_result)
                };

                let str_title = model_inference(&info.title_position, "title", cnt)?;
                let str_main_stat_name =
                    model_inference(&info.main_stat_name_position, "main_stat_name", cnt)?;
                let str_main_stat_value =
                    model_inference(&info.main_stat_value_position, "main_stat_value", cnt)?;

                let str_sub_stat_1 = model_inference(&info.sub_stat1_position, "sub_stat_1", cnt)?;
                let str_sub_stat_2 = model_inference(&info.sub_stat2_position, "sub_stat_2", cnt)?;
                let str_sub_stat_3 = model_inference(&info.sub_stat3_position, "sub_stat_3", cnt)?;
                let str_sub_stat_4 = model_inference(&info.sub_stat4_position, "sub_stat_4", cnt)?;

                let str_level = model_inference(&info.level_position, "level", cnt)?;
                let str_equip = model_inference(&info.equip_position, "equip", cnt)?;

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

            Ok(if min_level > 0 {
                results
                    .into_iter()
                    .filter(|result| result.level >= min_level)
                    .collect::<Vec<_>>()
            } else {
                results
            })
        });

        let mut scanned_row = 0_u32;
        let mut scanned_count = 0_u32;
        let mut start_row = 0_u32;
        // let mut lock = false;

        // self.move_to(0, 0);
        // self.enigo.mouse_click(MouseButton::Left);
        // utils::sleep(1000);
        // self.sample_initial_color();
        // let mut now = SystemTime::now();

        'outer: while scanned_count < count {
            let locks = self.get_locks(start_row)?;
            // println!("{}ms got locks", now.elapsed()?.as_millis());
            // now = SystemTime::now();
            let mut locks_idx: usize = 0;
            for row in start_row..self.row {
                let c = if scanned_row == total_row - 1 {
                    last_row_col
                } else {
                    self.col
                };
                for col in 0..c {
                    // 右键终止
                    if utils::is_rmb_down() {
                        break 'outer;
                    }

                    self.move_to(row, col);
                    self.enigo.mouse_click(MouseButton::Left);

                    let capture = self.wait_until_switched()?;

                    // capture
                    //     .save(&format!("dumps/art_{}.png", scanned_count + 1))
                    //     .expect("save image error");

                    let star = self.get_star(&capture)?;
                    if star < self.config.min_star {
                        break 'outer;
                    }

                    // lock = self.get_lock(lock);
                    let lock = locks[locks_idx];
                    locks_idx += 1;

                    tx.send(Some((capture, star, lock)))?;

                    scanned_count += 1;

                    // 大于最大数量则退出
                    if scanned_count >= count {
                        break 'outer;
                    }
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
            self.scroll_rows(scroll_row)?;
            // println!("{}ms scrolled", now.elapsed()?.as_millis());
            // now = SystemTime::now();
        }

        tx.send(None)?;

        info!("扫描结束，等待识别线程结束，请勿关闭程序");
        let results: Vec<InternalArtifact> =
            handle.join().map_err(|_| anyhow!("thread join err"))??;
        info!("count: {}", results.len());
        Ok(results)
    }
    pub fn flip_lock(&mut self, indices: Vec<u32>) -> Result<()> {
        self.align_panel();
        self.check_menu()?;
        self.scroll_to_top()?;
        self.get_scroll_speed()?;

        let mut indices = indices;
        indices.sort();

        let count = match self.get_art_count() {
            Ok(v) => v,
            Err(_) => 1500,
        };
        if indices[indices.len() - 1] > count {
            return Err(anyhow!("指标超出范围"));
        }

        let total_row = (count + self.col - 1) / self.col;
        let mut scanned_row = 0_u32;
        let mut start_row = 0_u32;

        // 如果不给第一个圣遗物加解锁，必须记录它的pool值
        // 以免wait_until_switched出错
        if indices.len() > 0 && indices[0] != 0 {
            let shot = self.capture_panel()?;
            self.pool = self.get_pool(&shot)?;
        }

        for index in indices {
            let row: u32 = index / self.col;
            let col: u32 = index % self.col;
            while row >= scanned_row + self.row {
                scanned_row += self.row;
                let remain_row = total_row - scanned_row;
                let scroll_row = remain_row.min(self.row);
                start_row = self.row - scroll_row;
                self.scroll_rows(scroll_row)?;
                // 右键终止
                if utils::is_rmb_down() {
                    break;
                }
            }
            // 右键终止
            if utils::is_rmb_down() {
                break;
            }
            // info!("{} {} {}", index, row, col);

            self.move_to(row - scanned_row + start_row, col);
            self.enigo.mouse_click(MouseButton::Left);
            self.wait_until_switched()?;

            let left: i32 = self.info.left + self.info.lock_x as i32;
            let top: i32 = self.info.top + self.info.lock_y as i32;

            self.enigo.mouse_move_to(left, top);
            self.enigo.mouse_click(MouseButton::Left);
            utils::sleep(self.config.lock_stop);

            self.move_to(row - scanned_row + start_row, col);
        }
        Ok(())
    }
}
