fn main() {
    // Ensure icons/ directory exists and required PNG files are present.
    // generate_context!() macro requires icons/icon.png at compile time.
    // iconAsTemplate in tauri.conf.json adapts tray-icon.png to menu bar color scheme.
    std::fs::create_dir_all("icons").ok();

    let tray_icon = std::path::Path::new("icons/tray-icon.png");
    if !tray_icon.exists() {
        let png = generate_icon_png(22, 22, false);
        std::fs::write(tray_icon, &png).expect("failed to write tray-icon.png");
    }

    // icons/icon.png is required by tauri::generate_context!() macro
    let app_icon = std::path::Path::new("icons/icon.png");
    if !app_icon.exists() {
        let png = generate_icon_png(512, 512, true);
        std::fs::write(app_icon, &png).expect("failed to write icon.png");
    }

    tauri_build::build()
}

/// Generate a minimal monochrome PNG icon.
/// `scale_to_22` = false → draw clipboard outline scaled to the given dimensions.
/// Uses pure-Rust PNG encoding (no external crates needed at build time).
fn generate_icon_png(w: u32, h: u32, filled: bool) -> Vec<u8> {
    // Build raw RGBA pixel rows (Tauri's generate_context!() requires RGBA)
    let mut raw: Vec<u8> = Vec::with_capacity((w * h * 4 + h) as usize);
    for y in 0..h {
        raw.push(0); // filter byte: None
        for x in 0..w {
            let lit = if filled {
                // Solid white square for app icon placeholder (no transparency)
                let margin = w / 8;
                x >= margin && x < w - margin && y >= margin && y < h - margin
            } else {
                is_lit(x, y, w, h)
            };
            let v = if lit { 255u8 } else { 0u8 };
            let a = if lit { 255u8 } else { 0u8 };
            raw.extend_from_slice(&[v, v, v, a]);
        }
    }

    // DEFLATE compress the raw data
    let compressed = deflate_no_compression(&raw);

    // Assemble PNG
    let mut out = Vec::new();
    // Signature
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    // IHDR
    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&w.to_be_bytes());
    ihdr[4..8].copy_from_slice(&h.to_be_bytes());
    ihdr[8] = 8;  // bit depth
    ihdr[9] = 6;  // color type: RGBA
    // ihdr[10..13] = 0 (compression, filter, interlace)
    write_chunk(&mut out, b"IHDR", &ihdr);
    write_chunk(&mut out, b"IDAT", &compressed);
    write_chunk(&mut out, b"IEND", b"");

    out
}

fn is_lit(x: u32, y: u32, _w: u32, _h: u32) -> bool {
    // Clipboard body outline: rect x=3..18, y=4..20
    let body = x >= 3 && x <= 18 && y >= 4 && y <= 20;
    let body_border = body && (x == 3 || x == 18 || y == 4 || y == 20);
    // Clip at top: x=8..13, y=1..5
    let clip = x >= 8 && x <= 13 && y >= 1 && y <= 5;
    let clip_border = clip && (x == 8 || x == 13 || y == 1 || y == 5);
    // Content lines
    let line1 = y == 8 && x >= 6 && x <= 15;
    let line2 = y == 11 && x >= 6 && x <= 15;
    let line3 = y == 14 && x >= 6 && x <= 12;
    body_border || clip_border || line1 || line2 || line3
}

/// Minimal DEFLATE encoding using stored (non-compressed) blocks.
/// This produces a valid zlib stream without any compression library dependency.
fn deflate_no_compression(data: &[u8]) -> Vec<u8> {
    // zlib header: CMF=0x78 (deflate, window 32K), FLG computed for no dict, level 0
    let cmf: u8 = 0x78;
    let flg: u8 = 0x01; // 0x7801 % 31 == 0
    let mut out = vec![cmf, flg];

    // Store blocks of up to 65535 bytes each
    let max_block = 65535usize;
    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + max_block).min(data.len());
        let block = &data[offset..end];
        let bfinal = if end == data.len() { 1u8 } else { 0u8 };
        let btype = 0u8; // BTYPE=00: no compression
        out.push(bfinal | (btype << 1));
        let len = block.len() as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(block);
        offset = end;
    }

    // Adler-32 checksum (big-endian)
    let (s1, s2) = adler32(data);
    let checksum = ((s2 as u32) << 16) | (s1 as u32);
    out.extend_from_slice(&checksum.to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> (u32, u32) {
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s1, s2)
}

fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    // CRC32 over chunk_type || data (chunk_type first, then data, chained)
    let crc_after_type = crc32(chunk_type, 0xFFFFFFFF);
    let crc_final = crc32(data, crc_after_type) ^ 0xFFFFFFFF;
    out.extend_from_slice(&crc_final.to_be_bytes());
}

fn crc32(data: &[u8], init: u32) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for i in 0..256u32 {
            let mut c = i;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
            }
            t[i as usize] = c;
        }
        t
    });
    let mut crc = init;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}
