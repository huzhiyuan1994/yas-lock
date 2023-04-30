use anyhow::{anyhow, Result};
use std::fs;
use std::io::stdin;
use std::io::stdout;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::time::SystemTime;
use tungstenite::WebSocket;
use yas::artifact::internal_artifact::InternalArtifact;
use yas::lock::LockAction;
use yas::ws::packet::{ConfigNotifyData, LockRspData, ScanRspData};

use yas::capture::capture_absolute_image;
use yas::common::utils;
use yas::expo::genmo::GenmoFormat;
use yas::expo::good::GoodFormat;
use yas::expo::mona::MonaFormat;
use yas::info::info;
use yas::scanner::yas_scanner::{YasScanner, YasScannerConfig};
use yas::ws::packet::Packet;

use clap::{arg, value_parser, ArgMatches, Command};
use env_logger::Builder;
use log::{error, info, warn, LevelFilter};

use std::net::TcpListener;
use tungstenite::{accept, Message};

// fn get_version() -> String {
//     let s = include_str!("../Cargo.toml");
//     for line in s.lines() {
//         if line.starts_with("version = ") {
//             let temp = line.split("\"").collect::<Vec<_>>();
//             return String::from(temp[temp.len() - 2]);
//         }
//     }

//     String::from("unknown_version")
// }

fn get_cli() -> Command {
    Command::new("YAS-lock - 原神圣遗物导出&加解锁")
        .version("v1.0.11")
        .author("wormtql <584130248@qq.com>, ideles <pyjy@yahoo.com>")
        .arg(arg!(--"dump" "输出模型预测结果、二值化图像和灰度图像，debug专用"))
        .arg(arg!(--"capture-only" "只保存截图，不进行扫描，debug专用"))
        .arg(arg!(--"mark" "保存标记后的截图，debug专用"))
        .arg(arg!(--"output-dir" <DIR> "输出目录").default_value("."))
        .arg(arg!(--"verbose" "显示详细信息"))
        .arg(arg!(--"no-check" "不检测是否已打开背包等"))
        .arg(arg!(--"dxgcap" "使用dxgcap捕获屏幕"))
        .arg(arg!(--"gui" "开启Web GUI"))
        .arg(
            arg!(--"max-row" <ROW> "最大扫描行数")
                .default_value("1000")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"min-star" <STAR> "最小星级")
                .default_value("5")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"min-level" <LEVEL> "最小等级")
                .default_value("0")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"speed" <SPEED> "速度（共1-5档，如提示大量重复尝试降低速度）")
                .default_value("5")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"number" <NUM> "指定圣遗物数量（在自动识别数量不准确时使用）")
                .default_value("0")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"default-stop" <TIME> "等待动画、鼠标点击等操作的默认停顿时间(ms)")
                .default_value("500")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"scroll-stop" <TIME> "页面滚动停顿时间(ms)")
                .default_value("100")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"lock-stop" <TIME> "加解锁停顿时间(ms)")
                .default_value("100")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"max-wait-switch-artifact" <TIME> "切换圣遗物最大等待时间(ms)")
                .default_value("800")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"max-wait-scroll" <TIME> "翻页的最大等待时间(ms)（翻页不正确可以考虑加大该选项）")
                .default_value("0")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"max-wait-lock" <TIME> "加解锁的最大等待时间(ms)（加解锁不正确可以考虑加大该选项）")
                .default_value("0")
                .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(--"offset-x" <OFFSET> "人为指定横坐标偏移（截图有偏移时可用该选项校正）")
                .default_value("0")
                .value_parser(value_parser!(i32)),
        )
        .arg(
            arg!(--"offset-y" <OFFSET> "人为指定纵坐标偏移（截图有偏移时可用该选项校正）")
                .default_value("0")
                .value_parser(value_parser!(i32)),
        )
        .arg(
            arg!(--"window" <NAME> "原神窗口名")
                .default_value("原神"),
        )
        .arg(
            arg!(--"scroll-speed" <SPEED> "滚轮速度（单位：像素，仅在云原神模式下生效）")
                .default_value("15.0")
                .value_parser(value_parser!(f64)),
        )
}

fn get_info(matches: &ArgMatches) -> Result<info::ScanInfo> {
    utils::set_dpi_awareness();

    let window_name: String = matches.get_one::<String>("window").unwrap().to_string();

    let hwnd = if window_name == String::from("原神") {
        utils::find_ys_window()
    } else {
        utils::find_window_by_name(&window_name)
    }
    .map_err(|_| anyhow!("未找到原神窗口，请确认原神已经开启"))?;

    utils::show_window_and_set_foreground(hwnd);
    utils::sleep(1000);

    let mut rect = utils::get_client_rect(hwnd)?;

    let offset_x: i32 = *matches.get_one("offset-x").unwrap();
    let offset_y: i32 = *matches.get_one("offset-y").unwrap();

    rect.left += offset_x;
    rect.top += offset_y;

    capture_absolute_image(&rect)?.save("test.png")?;

    info!(
        "left = {}, top = {}, width = {}, height = {}",
        rect.left, rect.top, rect.width, rect.height
    );

    let info: info::ScanInfo;
    if rect.height * 16 == rect.width * 9 {
        info =
            info::ScanInfo::from_16_9(rect.width as u32, rect.height as u32, rect.left, rect.top);
    } else if rect.height * 8 == rect.width * 5 {
        info = info::ScanInfo::from_8_5(rect.width as u32, rect.height as u32, rect.left, rect.top);
    } else if rect.height * 4 == rect.width * 3 {
        info = info::ScanInfo::from_4_3(rect.width as u32, rect.height as u32, rect.left, rect.top);
    } else {
        return Err(anyhow!("不支持的分辨率"));
    }

    Ok(info)
}

fn do_scan(matches: ArgMatches) -> Result<Vec<InternalArtifact>> {
    let config = YasScannerConfig::from_match(&matches)?;
    let info = get_info(&matches)?;
    let output_dir = Path::new(matches.try_get_one::<String>("output-dir")?.unwrap());

    let mut scanner = YasScanner::new(info.clone(), config)?;

    let now = SystemTime::now();
    let results = scanner.scan()?;
    let t = now.elapsed()?.as_secs_f64();
    info!("time: {}s", t);

    // Mona
    let mona = MonaFormat::new(&results);
    utils::dump_json(&mona, output_dir.join("mona.json"))?;
    // Genmo
    let genmo = GenmoFormat::new(&results);
    utils::dump_json(&genmo, output_dir.join("genmo.json"))?;
    // GOOD
    let good = GoodFormat::new(&results);
    utils::dump_json(&good, output_dir.join("good.json"))?;

    Ok(results)
}

fn do_lock(matches: ArgMatches, actions: Vec<LockAction>) -> Result<()> {
    let config = YasScannerConfig::from_match(&matches)?;
    let info = get_info(&matches)?;

    let mut scanner = YasScanner::new(info.clone(), config)?;
    scanner.lock(actions)
}

fn run_once(matches: ArgMatches) -> Result<()> {
    let output_dir = Path::new(matches.get_one::<String>("output-dir").unwrap());

    let mut lock_mode = false;
    let mut actions: Vec<LockAction> = Vec::new();

    let lock_filename = output_dir.join("lock.json");
    if lock_filename.exists() {
        print!("检测到lock文件，输入y开始加解锁，直接回车开始扫描：");
        stdout().flush()?;
        let mut s: String = String::new();
        stdin().read_line(&mut s)?;
        if s.trim() == "y" {
            let json_str = fs::read_to_string(lock_filename)?;
            actions = LockAction::from_lock_json(&json_str)?;
            lock_mode = true;
        }
    }

    // let _ = scanner.test()?;
    if lock_mode {
        do_lock(matches, actions)
    } else {
        do_scan(matches).map(|_| ())
    }
}

fn run_ws(matches: ArgMatches) -> Result<()> {
    let verbose = matches.get_flag("verbose");
    let cfg_ntf = ConfigNotifyData::packet(&matches)?;

    let addr = "127.0.0.1:2022";
    let server = TcpListener::bind(addr)?;

    info!("websocket server started: ws://{}", addr);

    open::that(format!(
        "https://ideless.github.io/artifact/#?ws=ws://{}",
        addr
    ))?;

    let recv_packet = |ws: &mut WebSocket<TcpStream>| -> Result<Option<Packet>> {
        match ws.read_message() {
            Ok(Message::Text(json)) => Ok(Some(serde_json::from_str::<Packet>(&json)?)),
            Ok(m) => {
                warn!("ignored message: {:?}", m);
                Ok(None)
            }
            Err(e) => {
                warn!("connection lost: {}", e);
                Err(anyhow!(e))
            }
        }
    };

    let handle_packet = |pkt: &Packet| -> Result<Option<Packet>> {
        match pkt {
            Packet::ScanReq(p) => {
                if verbose {
                    info!("recieved: {:?}", pkt);
                } else {
                    info!("recieved: {}", pkt.name());
                }
                let matches = get_cli()
                    .no_binary_name(true)
                    .try_get_matches_from(p.argv.iter())?;
                Ok(Some(ScanRspData::packet(do_scan(matches))?))
            }
            Packet::LockReq(p) => {
                if verbose {
                    info!("recieved: {:?}", pkt);
                } else {
                    info!("recieved: {}", pkt.name());
                }
                let matches = get_cli()
                    .no_binary_name(true)
                    .try_get_matches_from(p.argv.iter())?;
                let actions = match &p.lock_json {
                    Some(json_str) => LockAction::from_lock_json(&json_str)?,
                    None => match &p.indices {
                        Some(indices) => LockAction::from_v1(&indices),
                        None => Vec::new(),
                    },
                };
                Ok(Some(LockRspData::packet(do_lock(matches, actions))?))
            }
            p => {
                warn!("unexpected packet: {}", p.name());
                Err(anyhow!("unexpected packet"))
            }
        }
    };

    let send_packet = |ws: &mut WebSocket<TcpStream>, pkt: &Packet| -> Result<()> {
        match ws.write_message(Message::Text(pkt.to_json()?)) {
            Ok(_) => {
                if verbose {
                    info!("sent: {:?}", pkt);
                } else {
                    info!("sent: {}", pkt.name());
                }
                Ok(())
            }
            Err(e) => {
                warn!("connection closed: {}", e);
                Err(anyhow!(e))
            }
        }
    };

    for stream in server.incoming() {
        let stream = stream?;
        info!("connection established: {}", stream.peer_addr()?);
        let mut ws = accept(stream)?;
        send_packet(&mut ws, &cfg_ntf)?;
        loop {
            let pkt = match recv_packet(&mut ws) {
                Ok(Some(p)) => p,
                Ok(None) => continue,
                Err(_) => break,
            };
            let rsp = match handle_packet(&pkt) {
                Ok(Some(p)) => p,
                Ok(None) => continue,
                Err(_) => continue,
            };
            if let Err(_) = send_packet(&mut ws, &rsp) {
                break;
            }
        }
    }
    Ok(())
}

fn start(matches: ArgMatches) -> Result<()> {
    if !utils::is_admin() {
        return Err(anyhow!("请以管理员身份运行该程序"));
    }

    if matches.get_flag("gui") {
        run_ws(matches)
    } else {
        run_once(matches)
    }
}

fn main() {
    let matches = get_cli().get_matches();

    Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module(
            "yas",
            if matches.get_flag("verbose") {
                LevelFilter::Trace
            } else {
                LevelFilter::Info
            },
        )
        .format_timestamp_millis()
        .init();

    start(matches).unwrap_or_else(|e| error!("{:#}", e));

    info!("按Enter退出");
    let mut s = String::new();
    stdin().read_line(&mut s).expect("Readline error");
}
