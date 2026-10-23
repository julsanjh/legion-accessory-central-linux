use crate::models::{
    ParsedKbProfile, ParsedProfile, DEFAULT_PROFILE_RAW, SUPPORTED_MODELS,
};

pub fn checksum64(mut pkt: [u8; 64]) -> [u8; 64] {
    let sum: u8 = pkt[..63].iter().fold(0u8, |a, &b| a.wrapping_add(b));
    pkt[63] = sum;
    pkt
}

pub fn build_chunked_packets(cmd: u8, sub: u8, data: &[u8]) -> Vec<[u8; 64]> {
    let max_chunk = 0x3Ausize; // 58 bytes
    let total = ((data.len() + max_chunk - 1) / max_chunk).max(1);
    let mut out = Vec::with_capacity(total);
    for idx in 0..total {
        let start = idx * max_chunk;
        let end = ((idx + 1) * max_chunk).min(data.len());
        let chunk = &data[start..end];
        let mut pkt = [0u8; 64];
        pkt[0] = cmd;
        pkt[1] = sub;
        pkt[2] = (idx + 1) as u8;
        pkt[3] = total as u8;
        pkt[4] = chunk.len() as u8;
        pkt[5..5 + chunk.len()].copy_from_slice(chunk);
        out.push(checksum64(pkt));
    }
    out
}

pub fn build_lighting_data(effect: &str, c1: [u8; 3], c2: Option<[u8; 3]>) -> [u8; 12] {
    let mut buf = [0u8; 12];
    match effect.to_lowercase().as_str() {
        "off" => {
            buf[0] = 0x03;
        }
        "rainbow" => {
            buf[0] = 0x01;
            buf[1] = 0x64;
            buf[2] = 0x00;
            buf[3] = 0x0A;
        }
        "random" => {
            buf[0] = 0x03;
            buf[1] = 0x64;
            buf[2] = 0x01;
        }
        "static" => {
            buf[0] = 0x03;
            buf[1] = 0x64;
            buf[2] = 0x00;
            buf[3] = c1[0];
            buf[4] = c1[1];
            buf[5] = c1[2];
        }
        "breathing" => {
            buf[0] = 0x02;
            buf[1] = 0x64;
            buf[2] = 0x00;
            buf[3] = 0x02;
            if let Some(rgb2) = c2 {
                buf[4] = 0x03;
                buf[6..9].copy_from_slice(&c1);
                buf[9..12].copy_from_slice(&rgb2);
            } else {
                buf[4] = 0x01;
                buf[6..9].copy_from_slice(&c1);
            }
        }
        _ => {
            buf[0] = 0x01;
            buf[1] = 0x64;
            buf[3] = 0x0A;
        }
    }
    buf
}

pub fn build_lighting_packet(effect: &str, c1: [u8; 3], c2: Option<[u8; 3]>) -> [u8; 64] {
    let d12 = build_lighting_data(effect, c1, c2);
    build_chunked_packets(0x25, 0x00, &d12)[0]
}

pub fn build_dpi_value_packet(index: u8, dpi: u32) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    let val = (dpi / 100) as u16;
    pkt[0] = 0x04;
    pkt[1] = 0x01;
    pkt[2] = 0x00;
    pkt[3] = index;
    pkt[4..6].copy_from_slice(&val.to_le_bytes());
    pkt[6..8].copy_from_slice(&val.to_le_bytes());
    pkt
}

pub fn build_dpi_active_packet(index: u8) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = 0x04;
    pkt[1] = 0x01;
    pkt[2] = 0x01;
    pkt[3] = index;
    pkt
}

pub fn build_dpi_max_level_packet(levels: u8) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = 0x04;
    pkt[1] = 0x01;
    pkt[2] = 0x02;
    pkt[3] = levels.saturating_sub(1);
    pkt
}

pub fn build_report_rate_packet(hz: u32) -> [u8; 64] {
    let code = match hz {
        125 => 0x08,
        250 => 0x04,
        500 => 0x02,
        _ => 0x00, // 1000 Hz
    };
    let mut pkt = [0u8; 64];
    pkt[0] = 0x04;
    pkt[1] = 0x02;
    pkt[2] = 0x00;
    pkt[3] = code;
    pkt
}

pub fn build_key_action_packet(key_index: u8, action_code: u8) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = 0x02;
    pkt[1] = 0x00;
    pkt[2] = key_index;
    pkt[3] = action_code;
    checksum64(pkt)
}

pub fn parse_profile(raw: &[u8]) -> Option<ParsedProfile> {
    if raw.len() < 129 {
        return None;
    }
    let name_slice = &raw[2..32];
    let name_end = name_slice.iter().position(|&b| b == 0).unwrap_or(name_slice.len());
    let name = String::from_utf8_lossy(&name_slice[..name_end]).to_string();

    let mut button_codes = [0u8; 10];
    for i in 0..10 {
        button_codes[i] = raw[32 + i * 5];
    }

    let mut dpi_stages = [800u32; 5];
    for i in 0..5 {
        let vx = u16::from_le_bytes([raw[82 + i * 4], raw[83 + i * 4]]) as u32;
        dpi_stages[i] = (vx * 100).max(200);
    }

    let report_rate_hz = match raw[102] {
        0x08 => 125,
        0x04 => 250,
        0x02 => 500,
        _ => 1000,
    };
    let active_dpi_index = (raw[103] as usize).min(4);
    let dpi_levels = ((raw[105] as usize) + 1).clamp(1, 5);

    let light = &raw[117..129];
    let (lighting_effect, color1, color2) = match light[0] {
        1 => ("rainbow".to_string(), [0x25, 0x7E, 0xFF], [0xFF, 0x00, 0x55]),
        2 => (
            "breathing".to_string(),
            [light[6], light[7], light[8]],
            [light[9], light[10], light[11]],
        ),
        3 => {
            if light[1] == 0 {
                ("off".to_string(), [0, 0, 0], [0, 0, 0])
            } else if light[2] != 0 {
                ("random".to_string(), [0x25, 0x7E, 0xFF], [0, 0, 0])
            } else {
                ("static".to_string(), [light[3], light[4], light[5]], [0, 0, 0])
            }
        }
        _ => ("rainbow".to_string(), [0x25, 0x7E, 0xFF], [0, 0, 0]),
    };

    Some(ParsedProfile {
        name,
        report_rate_hz,
        active_dpi_index,
        dpi_levels,
        dpi_stages,
        lighting_effect,
        color1,
        color2,
        button_codes,
    })
}

// --- Keyboard (K300 / K310 / K500) Protocol ---

/// Build 9-byte Feature Report for sync / reading 448-byte profile (cmd 0x88, magic 0x5a)
pub fn build_kb_feature_sync(profile_idx: u8) -> [u8; 9] {
    let mut pkt = [0u8; 9];
    pkt[0] = 0x00;
    pkt[1] = 0x88;
    pkt[2] = profile_idx;
    pkt[3] = 0x5A;
    pkt
}

/// Build 9-byte Feature Report to unlock lighting updates (cmd 0x14, magic 0x5a)
pub fn build_kb_feature_unlock_lighting() -> [u8; 9] {
    let mut pkt = [0u8; 9];
    pkt[0] = 0x00;
    pkt[1] = 0x14;
    pkt[2] = 0x00;
    pkt[3] = 0x5A;
    pkt
}

/// Build 9-byte Feature Report to unlock Game Mode updates (cmd 0x15, magic 0x5a)
pub fn build_kb_feature_unlock_gamemode() -> [u8; 9] {
    let mut pkt = [0u8; 9];
    pkt[0] = 0x00;
    pkt[1] = 0x15;
    pkt[2] = 0x00;
    pkt[3] = 0x5A;
    pkt
}

/// Build 65-byte Output Report for Game Mode (sent via write() after feature report unlock)
pub fn build_kb_gamemode_output_report(master_enable: bool, win_lock: bool) -> [u8; 65] {
    let mut pkt = [0u8; 65];
    pkt[0] = 0x00; // Report ID
    pkt[1] = if master_enable { 0x01 } else { 0x00 }; // Master Game Mode Enable (indicator LED)
    pkt[2] = if win_lock { 0x01 } else { 0x00 }; // Disable Windows logo key
    pkt
}

/// Build 65-byte Output Report for Keyboard lighting (sent via write() after feature report unlock)
pub fn build_kb_lighting_output_report(
    effect_id: u8,
    brightness: u8,
    speed: u8,
    c1: [u8; 3],
    c2: Option<[u8; 3]>,
) -> [u8; 65] {
    let mut pkt = [0u8; 65];
    pkt[0] = 0x00; // Report ID
    pkt[1] = 0x01; // payload[0]: non-zero active marker / profile
    pkt[2] = effect_id; // payload[1]
    pkt[3] = brightness.min(2); // payload[2]: 0=off, 1=dim, 2=bright
    pkt[4] = speed.min(3); // payload[3]: frequency 0..3
    pkt[17] = c1[0]; // payload[16]
    pkt[18] = c1[1]; // payload[17]
    pkt[19] = c1[2]; // payload[18]
    if let Some(aux) = c2 {
        pkt[21] = aux[0]; // payload[20]
        pkt[22] = aux[1]; // payload[21]
        pkt[23] = aux[2]; // payload[22]
    }
    pkt
}

/// Parse 448-byte Keyboard Profile read from K300/K500
pub fn parse_kb_profile(raw: &[u8]) -> Option<ParsedKbProfile> {
    // Need the lighting block through auxiliary blue byte at offset 342.
    if raw.len() < 343 {
        return None;
    }
    // Profile layout: Name [0..64), key map [64..320), lighting [320..384),
    // Game Mode [384..448), with independent master and Win-lock flags.
    let base = 320;
    let game_mode = raw.get(384).map(|&b| b != 0).unwrap_or(false);
    let win_lock = raw.get(385).map(|&b| b != 0).unwrap_or(false);
    let effect_id = raw[base + 1];
    let brightness = raw[base + 2].min(2);
    let speed = raw[base + 3].min(3);
    let color1 = [raw[base + 16], raw[base + 17], raw[base + 18]];
    let color2 = [raw[base + 20], raw[base + 21], raw[base + 22]];

    Some(ParsedKbProfile {
        effect_id,
        brightness,
        speed,
        color1,
        color2,
        game_mode,
        win_lock,
        raw: raw.to_vec(),
    })
}

pub fn parse_hex_rgb(s: &str) -> Result<[u8; 3], String> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return Err(format!("Expected RRGGBB hex color, got '{s}'"));
    }
    let r = u8::from_str_radix(&h[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&h[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&h[4..6], 16).map_err(|e| e.to_string())?;
    Ok([r, g, b])
}

pub fn run_self_test() {
    let prof = parse_profile(&DEFAULT_PROFILE_RAW).expect("DefaultProfile.Raw must parse");
    assert_eq!(prof.name, "配置文件1");
    assert_eq!(prof.report_rate_hz, 1000);
    assert_eq!(prof.active_dpi_index, 1);
    assert_eq!(prof.dpi_levels, 3);
    assert_eq!(&prof.dpi_stages[..3], &[800, 1600, 3200]);

    assert_eq!(
        build_lighting_data("rainbow", [0, 0, 0], None),
        [0x01, 0x64, 0x00, 0x0A, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        build_lighting_data("random", [0, 0, 0], None),
        [0x03, 0x64, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        build_lighting_data("off", [0, 0, 0], None),
        [0x03, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        build_lighting_data("static", [0x25, 0x7E, 0xFF], None),
        [0x03, 0x64, 0x00, 0x25, 0x7E, 0xFF, 0, 0, 0, 0, 0, 0]
    );

    let lpkt = build_lighting_packet("static", [0xFF, 0x00, 0x00], None);
    assert_eq!(
        &lpkt[..11],
        &[0x25, 0x00, 0x01, 0x01, 0x0C, 0x03, 0x64, 0x00, 0xFF, 0x00, 0x00]
    );

    let dpkt = build_dpi_value_packet(1, 1600);
    assert_eq!(&dpkt[..8], &[0x04, 0x01, 0x00, 0x01, 16, 0, 16, 0]);
    assert_eq!(&build_dpi_active_packet(2)[..4], &[0x04, 0x01, 0x01, 0x02]);
    assert_eq!(&build_dpi_max_level_packet(3)[..4], &[0x04, 0x01, 0x02, 0x02]);
    assert_eq!(&build_report_rate_packet(500)[..4], &[0x04, 0x02, 0x00, 0x02]);

    let kpkt = build_key_action_packet(5, 0xB3);
    assert_eq!(&kpkt[..8], &[0x02, 0x00, 0x05, 0xB3, 0, 0, 0, 0]);
    assert_eq!(kpkt[63], 0x02u8.wrapping_add(0x05).wrapping_add(0xB3));

    // Keyboard K500/K300 protocol test vectors
    assert_eq!(
        build_kb_feature_sync(0),
        [0x00, 0x88, 0x00, 0x5A, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
    assert_eq!(
        build_kb_feature_unlock_lighting(),
        [0x00, 0x14, 0x00, 0x5A, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
    assert_eq!(
        build_kb_feature_unlock_gamemode(),
        [0x00, 0x15, 0x00, 0x5A, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
    let gm_on = build_kb_gamemode_output_report(true, true);
    assert_eq!(gm_on[0], 0x00);
    assert_eq!(gm_on[1], 0x01);
    assert_eq!(gm_on[2], 0x01);
    let gm_indicator_only = build_kb_gamemode_output_report(true, false);
    assert_eq!(gm_indicator_only[1], 0x01);
    assert_eq!(gm_indicator_only[2], 0x00);
    let gm_off = build_kb_gamemode_output_report(false, false);
    assert_eq!(gm_off[0], 0x00);
    assert_eq!(gm_off[1], 0x00);
    assert_eq!(gm_off[2], 0x00);

    let kb_static = build_kb_lighting_output_report(1, 2, 2, [0xFF, 0x00, 0x00], None);
    assert_eq!(kb_static[0], 0x00);
    assert_eq!(kb_static[1], 0x01);
    assert_eq!(kb_static[2], 1);
    assert_eq!(kb_static[3], 2);
    assert_eq!(kb_static[4], 2);
    assert_eq!(&kb_static[17..20], &[0xFF, 0x00, 0x00]);
    assert_eq!(&kb_static[21..24], &[0x00, 0x00, 0x00]);

    let kb_dual = build_kb_lighting_output_report(7, 2, 2, [0xFF, 0x00, 0x00], Some([0x00, 0xFF, 0x00]));
    assert_eq!(&kb_dual[17..20], &[0xFF, 0x00, 0x00]);
    assert_eq!(&kb_dual[21..24], &[0x00, 0xFF, 0x00]);
    let mut fake_448 = vec![0u8; 448];
    fake_448[320] = 0x01; // marker
    fake_448[321] = 1;    // effect_id static
    fake_448[322] = 2;    // brightness
    fake_448[323] = 1;    // speed
    fake_448[336] = 0x12; // R
    fake_448[337] = 0x34; // G
    fake_448[338] = 0x56; // B
    fake_448[384] = 0x01; // master enable
    fake_448[385] = 0x01; // win_lock
    let parsed = parse_kb_profile(&fake_448).expect("parse 448-byte profile");
    assert_eq!(parsed.effect_id, 1);
    assert_eq!(parsed.brightness, 2);
    assert_eq!(parsed.speed, 1);
    assert_eq!(parsed.color1, [0x12, 0x34, 0x56]);
    assert!(parsed.game_mode);
    assert!(parsed.win_lock);
    assert!(parse_kb_profile(&fake_448[..342]).is_none());
    assert_eq!(parse_kb_profile(&fake_448[..343]).unwrap().effect_id, 1);
    println!(
        "Self-test passed: all Rust M300/Legion protocol vectors verified ({} models supported).",
        SUPPORTED_MODELS.len()
    );
}
