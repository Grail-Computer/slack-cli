use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{bail, Context, Result};
use regex::bytes::Regex;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io::Write, os::unix::fs::PermissionsExt, path::PathBuf};

#[derive(Clone, Deserialize, Serialize)]
pub struct Credentials {
    pub token: String,
    pub cookie: String,
}
pub fn home() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("HOME").context("HOME is not set")?,
    ))
}
pub fn cache_path() -> Result<PathBuf> {
    Ok(home()?.join(".local/slk/token-cache.json"))
}
pub fn cached() -> Option<Credentials> {
    let path = cache_path().ok()?;
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).ok()?;
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
pub fn save(c: &Credentials) -> Result<()> {
    let path = cache_path()?;
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)?;
    if fs::symlink_metadata(dir)?.file_type().is_symlink() {
        bail!("Session cache directory must not be a symlink");
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    tmp.write_all(&serde_json::to_vec(c)?)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)
        .map_err(|_| anyhow::anyhow!("Could not save owner-only session cache"))?;
    Ok(())
}
pub fn decrypt(encrypted: &[u8], password: &[u8]) -> Result<String> {
    if !encrypted.starts_with(b"v10") {
        bail!("Unsupported Slack cookie encryption format");
    }
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1003, &mut key);
    let plain = cbc::Decryptor::<aes::Aes128>::new(&key.into(), &[b' '; 16].into())
        .decrypt_padded_vec_mut::<Pkcs7>(&encrypted[3..])
        .map_err(|_| anyhow::anyhow!("Slack cookie decryption failed"))?;
    let start = plain
        .windows(5)
        .position(|s| s == b"xoxd-")
        .context("No Slack session cookie in decrypted data")?;
    String::from_utf8(plain[start..].to_vec()).context("Invalid Slack cookie encoding")
}
fn keychain() -> Result<Vec<u8>> {
    #[cfg(target_os = "macos")]
    for account in ["Slack Key", "Slack", "Slack App Store Key"] {
        if let Ok(p) =
            security_framework::passwords::get_generic_password("Slack Safe Storage", account)
        {
            return Ok(p);
        }
    }
    bail!("Cannot access Slack Safe Storage. Sign into Slack Desktop; approve the macOS Keychain dialog with your Mac password and Always Allow.")
}
fn varint(data: &[u8], pos: &mut usize) -> Option<usize> {
    let mut n = 0usize;
    for shift in (0..63).step_by(7) {
        let b = *data.get(*pos)?;
        *pos += 1;
        n |= ((b & 127) as usize) << shift;
        if b < 128 {
            return Some(n);
        }
    }
    None
}
fn handle(data: &[u8], pos: &mut usize) -> Option<(usize, usize)> {
    Some((varint(data, pos)?, varint(data, pos)?))
}
fn block(file: &[u8], offset: usize, size: usize) -> Option<Vec<u8>> {
    if size > 32 * 1024 * 1024 {
        return None;
    }
    let end = offset.checked_add(size)?;
    let raw = file.get(offset..end)?;
    match file.get(end)? {
        0 => Some(raw.to_vec()),
        1 => {
            if snap::raw::decompress_len(raw).ok()? > 32 * 1024 * 1024 {
                return None;
            }
            snap::raw::Decoder::new().decompress_vec(raw).ok()
        }
        _ => None,
    }
}
fn entries(data: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut result = Vec::new();
    if data.len() < 4 {
        return result;
    }
    let count = u32::from_le_bytes(data[data.len() - 4..].try_into().unwrap()) as usize;
    let Some(end) = count
        .checked_add(1)
        .and_then(|c| c.checked_mul(4))
        .and_then(|n| data.len().checked_sub(n))
    else {
        return result;
    };
    let mut pos = 0;
    let mut previous = Vec::new();
    while pos < end {
        let Some(shared) = varint(data, &mut pos) else {
            break;
        };
        let Some(nonshared) = varint(data, &mut pos) else {
            break;
        };
        let Some(value_len) = varint(data, &mut pos) else {
            break;
        };
        let Some(key_end) = pos.checked_add(nonshared) else {
            break;
        };
        let Some(value_end) = key_end.checked_add(value_len) else {
            break;
        };
        if shared > previous.len() || value_end > end {
            break;
        }
        previous.truncate(shared);
        previous.extend_from_slice(&data[pos..key_end]);
        result.push((previous.clone(), data[key_end..value_end].to_vec()));
        pos = value_end;
    }
    result
}
pub fn tokens(data: &[u8]) -> BTreeSet<String> {
    let re = Regex::new(r"xoxc-[0-9]+-[0-9]+-[0-9]+-[a-f0-9]{64}").unwrap();
    let mut found = BTreeSet::new();
    let mut scan = |bytes: &[u8]| {
        for m in re.find_iter(bytes) {
            found.insert(String::from_utf8_lossy(m.as_bytes()).into_owned());
        }
    };
    scan(data);
    if data.len() >= 48 && data.ends_with(&[0x57, 0xfb, 0x80, 0x8b, 0x24, 0x75, 0x47, 0xdb]) {
        let footer = &data[data.len() - 48..];
        let mut pos = 0;
        if handle(footer, &mut pos).is_some() {
            if let Some((o, s)) = handle(footer, &mut pos) {
                if let Some(index) = block(data, o, s) {
                    for (_, value) in entries(&index) {
                        if let Some((o, s)) = handle(&value, &mut 0) {
                            if let Some(b) = block(data, o, s) {
                                for (k, v) in entries(&b) {
                                    scan(&k);
                                    scan(&v);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    found
}
pub fn fresh() -> Result<(String, Vec<String>)> {
    let h = home()?;
    let dirs = [
        h.join("Library/Application Support/Slack"),
        h.join(
            "Library/Containers/com.tinyspeck.slackmacgap/Data/Library/Application Support/Slack",
        ),
    ];
    let dir = dirs
        .iter()
        .find(|p| p.join("Cookies").is_file())
        .context("Slack Desktop data not found")?;
    let conn = Connection::open_with_flags(dir.join("Cookies"), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("Cannot read Slack cookie database")?;
    let encrypted: Vec<u8> = conn
        .query_row(
            "SELECT encrypted_value FROM cookies WHERE name='d' AND host_key='.slack.com' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .context("Slack session cookie not found")?;
    let cookie = decrypt(&encrypted, &keychain()?)?;
    let mut found = BTreeSet::new();
    for f in fs::read_dir(dir.join("Local Storage/leveldb"))? {
        let p = f?.path();
        if matches!(
            p.extension().and_then(|s| s.to_str()),
            Some("ldb" | "log" | "sst")
        ) {
            if let Ok(raw) = fs::read(&p) {
                found.extend(tokens(&raw));
            }
        }
    }
    if found.is_empty() {
        bail!("No Slack session token found. Open Slack Desktop and retry.");
    }
    Ok((cookie, found.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncryptMut;
    #[test]
    fn cookie_round_trip_and_bad_padding() {
        let mut key = [0u8; 16];
        pbkdf2::pbkdf2_hmac::<sha1::Sha1>(b"fixture", b"saltysalt", 1003, &mut key);
        let mut encrypted = b"v10".to_vec();
        encrypted.extend(
            cbc::Encryptor::<aes::Aes128>::new(&key.into(), &[b' '; 16].into())
                .encrypt_padded_vec_mut::<Pkcs7>(b"prefixxoxd-fixture"),
        );
        assert_eq!(decrypt(&encrypted, b"fixture").unwrap(), "xoxd-fixture");
        assert!(decrypt(&encrypted, b"wrong").is_err());
    }
    #[test]
    fn strict_token_scanning() {
        let s = format!("xoxc-1-2-3-{}", "a".repeat(64));
        assert!(tokens(s.as_bytes()).contains(&s));
        assert!(tokens(b"xoxc-truncated").is_empty());
    }
    #[test]
    fn token_in_snappy_sstable_block() {
        fn vi(mut n: usize, out: &mut Vec<u8>) {
            while n >= 128 {
                out.push((n as u8 & 127) | 128);
                n >>= 7;
            }
            out.push(n as u8);
        }
        fn entry(key: &[u8], value: &[u8]) -> Vec<u8> {
            let mut out = vec![0];
            vi(key.len(), &mut out);
            vi(value.len(), &mut out);
            out.extend(key);
            out.extend(value);
            out.extend(0u32.to_le_bytes());
            out.extend(1u32.to_le_bytes());
            out
        }
        let token = format!("xoxc-1-2-3-{}", "a".repeat(64));
        let raw = entry(b"session", token.as_bytes());
        let data = snap::raw::Encoder::new().compress_vec(&raw).unwrap();
        let mut file = data.clone();
        file.extend([1, 0, 0, 0, 0]);
        let mut handle = vec![0];
        vi(data.len(), &mut handle);
        let index = entry(b"session", &handle);
        let offset = file.len();
        file.extend(&index);
        file.extend([0, 0, 0, 0, 0]);
        let mut footer = vec![0, 0];
        vi(offset, &mut footer);
        vi(index.len(), &mut footer);
        footer.resize(40, 0);
        footer.extend([0x57, 0xfb, 0x80, 0x8b, 0x24, 0x75, 0x47, 0xdb]);
        file.extend(footer);
        assert!(tokens(&file).contains(&token));
    }
    #[test]
    fn malformed_leveldb_is_bounded() {
        assert!(entries(&[255; 8]).is_empty());
        assert!(block(&[1, 2], usize::MAX, 10).is_none());
    }
}
