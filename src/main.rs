mod gui;
mod hid;
mod models;
mod protocol;

use crate::gui::LegionApp;
use crate::hid::{detect_connected_devices, HidConnection};
use crate::models::{BUTTON_ACTIONS, DEFAULT_PROFILE_RAW};
use crate::protocol::{
    build_chunked_packets, build_dpi_active_packet, build_dpi_max_level_packet,
    build_dpi_value_packet, build_key_action_packet, build_lighting_packet,
    build_report_rate_packet, parse_hex_rgb, run_self_test,
};
use eframe::egui;
use std::env;

fn print_help() {
    println!(
        "Lenovo Legion Accessory Central v2.0.9.10231 — Native Rust Linux Port\n\
         Supports: M300, M300s, M500, M600, M600s, M600s Qi, K300, K310, K310 AI, K500\n\n\
         Usage: legion-accessory-central [OPTIONS]\n\n\
         Options:\n\
         \x20 -d, --device <PATH>                Specify /dev/hidrawX path\n\
         \x20 --gui                              Launch graphical interface (default)\n\
         \x20 --self-test                        Verify protocol vectors\n\
         \x20 --status                           Sync and print current mouse profile\n\
         \x20 --lighting <EFFECT> [HEX1] [HEX2]  off | rainbow | random | static RRGGBB | breathing RRGGBB [RRGGBB2]\n\
         \x20 --dpi <STAGE_0_4> <DPI>            Set DPI for stage 0..4 (200..8000, step 100)\n\
         \x20 --dpi-active <STAGE_0_4>           Set active DPI stage (0..4)\n\
         \x20 --dpi-levels <COUNT_1_5>           Set number of DPI stages (1..5)\n\
         \x20 --rate <125|250|500|1000>          Set polling rate in Hz\n\
         \x20 --button <SLOT_0_9> <ACTION>       Map button slot to action\n\
         \x20 --factory-reset                    Restore device to factory defaults\n\
         \x20 -h, --help                         Print help"
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut device_path: Option<String> = None;
    let mut force_gui = false;
    let mut cli_ops = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "--self-test" => {
                run_self_test();
                return;
            }
            "--gui" => {
                force_gui = true;
                i += 1;
            }
            "-d" | "--device" if i + 1 < args.len() => {
                device_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--status" | "--lighting" | "--dpi" | "--dpi-active" | "--dpi-levels" | "--rate"
            | "--button" | "--factory-reset" => {
                cli_ops = true;
                break;
            }
            _ => {
                i += 1;
            }
        }
    }

    if cli_ops && !force_gui {
        let target_path =
            device_path.or_else(|| detect_connected_devices().first().map(|(_, p)| p.clone()));
        let Some(path) = target_path else {
            eprintln!("Error: No Lenovo Legion HID device found on USB.");
            std::process::exit(1);
        };
        let mut conn = match HidConnection::open(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        };

        while i < args.len() {
            match args[i].as_str() {
                "--factory-reset" => {
                    conn.write_raw(&[0x07, 0x05, 0x00], 80).unwrap();
                    conn.write_raw(&[0x07, 0x05, 0x01], 80).unwrap();
                    for pkt in build_chunked_packets(0x01, 0x00, &DEFAULT_PROFILE_RAW) {
                        conn.write_raw(&pkt, 50).unwrap();
                    }
                    conn.write_raw(&[0x05, 0x00, 0x00], 60).unwrap();
                    println!("Factory defaults restored.");
                    i += 1;
                }
                "--lighting" if i + 1 < args.len() => {
                    let eff = &args[i + 1];
                    let mut c1 = [0xFF, 0x00, 0x00];
                    let mut c2 = None;
                    let mut step = 2;
                    if i + 2 < args.len() && !args[i + 2].starts_with("--") {
                        c1 = parse_hex_rgb(&args[i + 2]).expect("Invalid hex color 1");
                        step += 1;
                        if i + 3 < args.len() && !args[i + 3].starts_with("--") {
                            c2 = Some(parse_hex_rgb(&args[i + 3]).expect("Invalid hex color 2"));
                            step += 1;
                        }
                    }
                    let pkt = build_lighting_packet(eff, c1, c2);
                    conn.write_raw(&pkt, 50).unwrap();
                    println!("Lighting set to {eff}.");
                    i += step;
                }
                "--dpi" if i + 2 < args.len() => {
                    let idx: u8 = args[i + 1].parse().expect("Invalid stage index");
                    let dpi: u32 = args[i + 2].parse().expect("Invalid DPI");
                    conn.write_raw(&build_dpi_value_packet(idx, dpi), 60)
                        .unwrap();
                    let _ = conn.read_packet(200);
                    println!("DPI stage {idx} set to {dpi} DPI.");
                    i += 3;
                }
                "--dpi-active" if i + 1 < args.len() => {
                    let idx: u8 = args[i + 1].parse().expect("Invalid stage index");
                    conn.write_raw(&build_dpi_active_packet(idx), 60).unwrap();
                    let _ = conn.read_packet(200);
                    println!("Active DPI stage set to {idx}.");
                    i += 2;
                }
                "--dpi-levels" if i + 1 < args.len() => {
                    let lv: u8 = args[i + 1].parse().expect("Invalid stage count");
                    conn.write_raw(&build_dpi_max_level_packet(lv), 60).unwrap();
                    let _ = conn.read_packet(200);
                    println!("DPI stages count set to {lv}.");
                    i += 2;
                }
                "--rate" if i + 1 < args.len() => {
                    let hz: u32 = args[i + 1].parse().expect("Invalid Hz");
                    conn.write_raw(&build_report_rate_packet(hz), 60).unwrap();
                    println!("Polling rate set to {hz} Hz.");
                    i += 2;
                }
                "--button" if i + 2 < args.len() => {
                    let slot: u8 = args[i + 1].parse().expect("Invalid button slot");
                    let act_name = args[i + 2].to_lowercase();
                    let act = BUTTON_ACTIONS
                        .iter()
                        .find(|a| a.key == act_name)
                        .expect("Unknown button action");
                    conn.write_raw(&build_key_action_packet(slot, act.code), 60)
                        .unwrap();
                    let _ = conn.read_packet(200);
                    println!("Button {slot} mapped to {}.", act.label);
                    i += 3;
                }
                "--status" => {
                    match conn.sync() {
                        Ok(prof) => {
                            println!(
                                "{{\n  \"name\": \"{}\",\n  \"report_rate_hz\": {},\n  \"active_dpi_index\": {},\n  \"dpi_levels\": {},\n  \"dpi_stages\": {:?},\n  \"lighting_effect\": \"{}\"\n}}",
                                prof.name,
                                prof.report_rate_hz,
                                prof.active_dpi_index,
                                prof.dpi_levels,
                                &prof.dpi_stages[..prof.dpi_levels],
                                prof.lighting_effect
                            );
                        }
                        Err(e) => eprintln!("Sync error: {e}"),
                    }
                    i += 1;
                }
                _ => i += 1,
            }
        }
        return;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Legion Accessory Central")
            .with_inner_size([1180.0, 720.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Legion Accessory Central",
        options,
        Box::new(move |_cc| Box::new(LegionApp::new(device_path))),
    );
}
