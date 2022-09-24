use anyhow::Result;
use std::fs::File;
use std::io::stdin;
use std::io::stdout;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;

use yas::common::utils;
use yas::expo::genmo::GenmoFormat;
use yas::expo::good::GoodFormat;
use yas::expo::mona_uranai::MonaFormat;
use yas::info::info;
use yas::scanner::yas_scanner::{YasScanner, YasScannerConfig};

use winapi::um::winuser::{SetForegroundWindow, ShowWindow, SW_RESTORE};

use clap::{App, Arg};
use env_logger::Builder;
use log::{info, LevelFilter};

// use enigo::*;

// fn open_local(path: String) -> RawImage {
//     let img = image::open(path).unwrap();
//     let img = grayscale(&img);
//     let raw_img = image_to_raw(img);

//     raw_img
// }

fn get_version() -> String {
    let s = include_str!("../Cargo.toml");
    for line in s.lines() {
        if line.starts_with("version = ") {
            let temp = line.split("\"").collect::<Vec<_>>();
            return String::from(temp[temp.len() - 2]);
        }
    }

    String::from("unknown_version")
}

fn read_lock_file<P: AsRef<Path>>(path: P) -> Result<Vec<u32>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let l: Vec<u32> = serde_json::from_reader(reader)?;

    Ok(l)
}

/*
fn main() {
    let hwnd = match utils::find_window(String::from("原神")) {
        Err(_s) => {
            utils::error_and_quit("未找到原神窗口，请确认原神已经开启");
        }
        Ok(h) => h,
    };

    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
    }
    // utils::sleep(1000);
    unsafe {
        SetForegroundWindow(hwnd);
    }
    let mut enigo = Enigo::new();
    let mut state = 0;
    loop {
        if state == 1 {
            enigo.mouse_click(MouseButton::Left);
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        if utils::is_f12_down() {
            state = 1 - state;
            if state == 0 {
                println!("\n暂停中");
            } else {
                println!("点击中");
            }
        }
        utils::sleep(200);
    }
}
*/

/*
fn gcd(mut a: u32, mut b: u32) -> u32 {
    let mut r;
    while b > 0 {
        r = a % b;
        a = b;
        b = r;
    }
    a
}

fn main() {
    set_dpi_awareness();

    let hwnd = match utils::find_window(String::from("原神")) {
        Err(_s) => {
            utils::error_and_quit("未找到原神窗口，请确认原神已经开启");
        }
        Ok(h) => h,
    };

    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
    }
    unsafe {
        SetForegroundWindow(hwnd);
    }

    utils::sleep(1000);
    let rect = utils::get_client_rect(hwnd).unwrap();

    // rect.scale(1.25);
    // info!("detected left: {}", rect.left);
    // info!("detected top: {}", rect.top);
    // info!("detected width: {}", rect.width);
    // info!("detected height: {}", rect.height);
    let g = gcd(rect.width as u32, rect.height as u32);

    // let date = Local::now();
    capture_absolute_image(&rect)
        .unwrap()
        // .save(format!("{}.png", date.format("%Y_%y_%d_%H_%M_%S")))
        .save(format!(
            "{}x{}_{}x{}.png",
            rect.width as u32 / g,
            rect.height as u32 / g,
            rect.width,
            rect.height
        ))
        .expect("fail to take screenshot");
}
*/

fn main() {
    Builder::new().filter_level(LevelFilter::Info).init();

    if !utils::is_admin() {
        utils::error_and_quit("请以管理员身份运行该程序")
    }

    let version = get_version();

    let matches = App::new("YAS - 原神圣遗物导出器")
        .version(version.as_str())
        .author("wormtql <584130248@qq.com>")
        .about("Genshin Impact Artifact Exporter")
        .arg(
            Arg::with_name("max-row")
                .long("max-row")
                .takes_value(true)
                .help("最大扫描行数"),
        )
        .arg(
            Arg::with_name("dump")
                .long("dump")
                .required(false)
                .takes_value(false)
                .help("输出模型预测结果、二值化图像和灰度图像，debug专用"),
        )
        .arg(
            Arg::with_name("capture-only")
                .long("capture-only")
                .required(false)
                .takes_value(false)
                .help("只保存截图，不进行扫描，debug专用"),
        )
        .arg(
            Arg::with_name("min-star")
                .long("min-star")
                .takes_value(true)
                .help("最小星级")
                .min_values(1)
                .max_values(5),
        )
        .arg(
            Arg::with_name("min-level")
                .long("min-level")
                .takes_value(true)
                .help("最小等级"),
        )
        .arg(
            Arg::with_name("max-wait-switch-artifact")
                .long("max-wait-switch-artifact")
                .takes_value(true)
                .min_values(10)
                .help("切换圣遗物最大等待时间(ms)"),
        )
        .arg(
            Arg::with_name("output-dir")
                .long("output-dir")
                .short("o")
                .takes_value(true)
                .help("输出目录")
                .default_value("."),
        )
        .arg(
            Arg::with_name("scroll-stop")
                .long("scroll-stop")
                .takes_value(true)
                .help("翻页时滚轮停顿时间（ms）（翻页不正确可以考虑加大该选项，默认为100）"),
        )
        .arg(
            Arg::with_name("number")
                .long("number")
                .takes_value(true)
                .help("指定圣遗物数量（在自动识别数量不准确时使用）")
                .min_values(1)
                .max_values(1500),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .help("显示详细信息"),
        )
        .arg(
            Arg::with_name("offset-x")
                .long("offset-x")
                .takes_value(true)
                .help("人为指定横坐标偏移（截图有偏移时可用该选项校正）"),
        )
        .arg(
            Arg::with_name("offset-y")
                .long("offset-y")
                .takes_value(true)
                .help("人为指定纵坐标偏移（截图有偏移时可用该选项校正）"),
        )
        .arg(
            Arg::with_name("speed")
                .long("speed")
                .takes_value(true)
                .help("速度（共1-5档，默认5，如提示大量重复尝试降低速度）")
                .min_values(1)
                .max_values(5),
        )
        // .arg(Arg::with_name("output-format").long("output-format").short("f").takes_value(true).help("输出格式。mona：莫纳占卜铺（默认）；mingyulab：原魔计算器。").possible_values(&["mona", "mingyulab"]).default_value("mona"))
        .get_matches();

    let config = YasScannerConfig::from_match(&matches)
        .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));

    let output_dir = Path::new(matches.value_of("output-dir").unwrap_or("."));

    let mut lock_mode = false;
    let mut indices: Vec<u32> = Vec::new();

    let lock_filename = output_dir.join("lock.json");
    if lock_filename.exists() {
        print!("检测到lock文件，输入y开始加解锁，直接回车开始扫描：");
        stdout()
            .flush()
            .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));
        let mut s: String = String::new();
        stdin().read_line(&mut s).expect("Readline error");
        if s.trim() == "y" {
            indices = read_lock_file(lock_filename)
                .unwrap_or_else(|_| utils::error_and_quit("无法读取lock文件"));
            lock_mode = true;
        }
    }

    crate::utils::set_dpi_awareness();

    let hwnd = match utils::find_window("原神") {
        Err(_s) => {
            utils::error_and_quit("未找到原神窗口，请确认原神已经开启");
        }
        Ok(h) => h,
    };

    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
    }
    // utils::sleep(1000);
    unsafe {
        SetForegroundWindow(hwnd);
    }
    utils::sleep(1000);

    let rect = utils::get_client_rect(hwnd).unwrap_or_else(|e| utils::error_and_quit(&e));

    // rect.scale(1.25);
    // info!("detected left: {}", rect.left);
    // info!("detected top: {}", rect.top);
    // info!("detected width: {}", rect.width);
    // info!("detected height: {}", rect.height);
    info!("分辨率: {}x{}", rect.width, rect.height);

    let mut info: info::ScanInfo;
    if rect.height * 16 == rect.width * 9 {
        info =
            info::ScanInfo::from_16_9(rect.width as u32, rect.height as u32, rect.left, rect.top);
    } else if rect.height * 8 == rect.width * 5 {
        info = info::ScanInfo::from_8_5(rect.width as u32, rect.height as u32, rect.left, rect.top);
    } else if rect.height * 4 == rect.width * 3 {
        info = info::ScanInfo::from_4_3(rect.width as u32, rect.height as u32, rect.left, rect.top);
    } else {
        utils::error_and_quit("不支持的分辨率");
    }

    let offset_x = matches
        .value_of("offset-x")
        .unwrap_or("0")
        .parse::<i32>()
        .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));
    let offset_y = matches
        .value_of("offset-y")
        .unwrap_or("0")
        .parse::<i32>()
        .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));
    info.left += offset_x;
    info.top += offset_y;

    let mut scanner = YasScanner::new(info.clone(), config)
        .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));

    // scanner.test();
    // return;

    if lock_mode {
        scanner
            .flip_lock(indices)
            .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));
    } else {
        let now = SystemTime::now();
        let results = match scanner.scan() {
            Ok(v) => v,
            Err(e) => utils::error_and_quit(&e.to_string()),
        };
        let t = now
            .elapsed()
            .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()))
            .as_secs_f64();
        info!("time: {}s", t);

        // Mona
        let output_filename = output_dir.join("mona.json");
        let mona = MonaFormat::new(&results);
        mona.save(String::from(
            output_filename.to_str().unwrap_or("mona.json"),
        ))
        .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));
        // Genmo
        let output_filename = output_dir.join("genmo.json");
        let genmo = GenmoFormat::new(&results);
        genmo
            .save(String::from(
                output_filename.to_str().unwrap_or("genmo.json"),
            ))
            .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));
        // GOOD
        let output_filename = output_dir.join("good.json");
        let good = GoodFormat::new(&results);
        good.save(String::from(
            output_filename.to_str().unwrap_or("good.json"),
        ))
        .unwrap_or_else(|e| utils::error_and_quit(&e.to_string()));
    }

    // let info = info;
    // let img = info.art_count_position.capture_relative(&info).unwrap();

    // let mut inference = CRNNModel::new(String::from("model_training.onnx"), String::from("index_2_word.json"));
    // let s = inference.inference_string(&img);
    // println!("{}", s);
    info!("按Enter退出");
    let mut s = String::new();
    stdin().read_line(&mut s).expect("Readline error");
}
