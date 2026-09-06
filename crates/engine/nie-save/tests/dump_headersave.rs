use nie_save::{BlobSubtype, parse};

#[test]
fn dump_headersave_body() {
    for (path, name) in &[
        ("data/saves/002AB8F4-USERDATALIVE", "002AB8F4-USERDATALIVE"),
        ("data/saves/002AB8F4-SYSTEMLIVE", "002AB8F4-SYSTEMLIVE"),
    ] {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        println!("\n=== {} ===", name);
        let data = std::fs::read(path).unwrap();
        let container = parse(&data, name).unwrap();

        for blob in &container.blobs {
            println!(
                "blob subtype={:?} field8=0x{:08X} payload_size=0x{:08X} body_len={}",
                blob.header.subtype,
                blob.header.field8,
                blob.header.payload_size,
                blob.body.len()
            );

            if blob.header.subtype == BlobSubtype::Headersave {
                println!("--- HEADERSAVE body ---");
                let body = &blob.body;
                for (i, chunk) in body.chunks(16).enumerate() {
                    let hex: String = chunk.iter().map(|b| format!("{b:02x} ")).collect();
                    let ascii: String = chunk
                        .iter()
                        .map(|&b| {
                            if (0x20..0x7F).contains(&b) {
                                b as char
                            } else {
                                '.'
                            }
                        })
                        .collect();
                    println!("{:04x}: {:<48}|{}|", i * 16, hex, ascii);
                }
            }
        }
    }
}
