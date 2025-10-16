use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=DOPPLER_PROGRAM_ID");
    println!("cargo:rerun-if-env-changed=DOPPLER_ADMIN_KEY");

    let default_program_id =
        "fastRQJt3nLdY3QA7n8eZ8ETEVefy56ryfUGVkfZokm".to_string();
    let program_id = env::var("DOPPLER_PROGRAM_ID").unwrap_or(default_program_id);

    let default_admin_key = "admnz5UvRa93HM5nTrxXmsJ1rw2tvXMBFGauvCgzQhE".to_string();
    let admin_key = env::var("DOPPLER_ADMIN_KEY").unwrap_or(default_admin_key);

    let program_bytes = decode_base58(&program_id, "DOPPLER_PROGRAM_ID");
    let admin_bytes = decode_base58(&admin_key, "DOPPLER_ADMIN_KEY");

    let contents = format!(
        "pub const PROGRAM_ID_BYTES: [u8; 32] = [{}];\n\
         pub const PROGRAM_ID_BASE58: &str = \"{}\";\n\
         pub const ADMIN_BYTES: [u8; 32] = [{}];\n\
         pub const ADMIN_BASE58: &str = \"{}\";\n",
        fmt_bytes(&program_bytes),
        program_id,
        fmt_bytes(&admin_bytes),
        admin_key
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    fs::write(out_dir.join("doppler_constants.rs"), contents)
        .expect("failed to write doppler_constants.rs");
}

fn decode_base58(value: &str, var_name: &str) -> [u8; 32] {
    let bytes = bs58::decode(value).into_vec().unwrap_or_else(|err| {
        panic!("invalid {} base58 string: {err}", var_name);
    });
    if bytes.len() != 32 {
        panic!(
            "{} must decode to 32 bytes, got {}",
            var_name,
            bytes.len()
        );
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    array
}

fn fmt_bytes(bytes: &[u8; 32]) -> String {
    bytes
        .iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ")
}
