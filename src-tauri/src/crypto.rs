//! Trae CN / TRAE SOLO CN storage.json 值加解密
//!
//! 从 Trae CN 客户端 `out-build/vs/base/common/byteCrypto.js` 逆向所得。
//! 存储格式：base64( header(6B: "tc\x05\x10\x00\x00") + randomKey(32B) + AES-128-CBC(SHA512(plain) + plain) )
//! 密钥派生：key = SHA512(randomKey)，buf[64..128] = ZTE ^ KTE 常量盐，
//!           buf[0..64] = SHA512(buf)，AES key = buf[0..16]，IV = buf[16..32]。
//! 自包含加密：随机密钥内嵌在数据中，与 machineid / Keychain 无关。

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sha2::{Digest, Sha512};

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

const HEADER: [u8; 6] = [116, 99, 5, 16, 0, 0]; // 't' 'c' 5 16 0 0
const KEY_LEN: usize = 32; // 内嵌随机密钥长度
const CHECKSUM_LEN: usize = 64; // SHA-512 校验长度

/// zte ^ kte 常量盐（64 字节）
const SALT: [u8; 64] = [
    82 ^ 31, 9 ^ 221, 106 ^ 168, 213 ^ 51, 48 ^ 136, 54 ^ 7, 165 ^ 199, 56 ^ 49, 191 ^ 177,
    64 ^ 18, 163 ^ 16, 158 ^ 89, 129 ^ 39, 243 ^ 128, 215 ^ 236, 251 ^ 95, 124 ^ 96, 227 ^ 81,
    57 ^ 127, 130 ^ 169, 155 ^ 25, 47 ^ 181, 255 ^ 74, 135 ^ 13, 52 ^ 45, 142 ^ 229, 67 ^ 122,
    68 ^ 159, 196 ^ 147, 222 ^ 201, 233 ^ 156, 203 ^ 239, 84 ^ 160, 123 ^ 224, 148 ^ 59,
    50 ^ 77, 166 ^ 174, 194 ^ 42, 35 ^ 245, 61 ^ 176, 238 ^ 200, 76 ^ 235, 149 ^ 187, 11 ^ 60,
    66 ^ 131, 250 ^ 83, 195 ^ 153, 78 ^ 97, 8 ^ 23, 46 ^ 43, 161 ^ 4, 102 ^ 126, 40 ^ 186,
    217 ^ 119, 36 ^ 214, 178 ^ 38, 118 ^ 225, 91 ^ 105, 162 ^ 20, 73 ^ 99, 109 ^ 85, 139 ^ 33,
    209 ^ 12, 37 ^ 125,
];

/// 从 32 字节随机密钥派生 AES-128 key + IV
fn derive_key_iv(random_key: &[u8; KEY_LEN]) -> ([u8; 16], [u8; 16]) {
    let mut buf = [0u8; 128];
    buf[0..64].copy_from_slice(&Sha512::digest(random_key));
    buf[64..128].copy_from_slice(&SALT);
    let hash = Sha512::digest(&buf);
    buf[0..64].copy_from_slice(&hash);

    let mut aes_key = [0u8; 16];
    let mut iv = [0u8; 16];
    aes_key.copy_from_slice(&buf[0..16]);
    iv.copy_from_slice(&buf[16..32]);
    (aes_key, iv)
}

/// 判断字符串是否为 Trae 加密存储值（base64 且解码头 6 字节匹配）
pub fn is_encrypted(value: &str) -> bool {
    let trimmed = value.trim();
    if !trimmed.starts_with("dGMFEA") {
        // base64("tc\x05\x10...") 的固定前缀
        return false;
    }
    match BASE64.decode(trimmed) {
        Ok(bytes) => bytes.len() > HEADER.len() + KEY_LEN && bytes[..HEADER.len()] == HEADER,
        Err(_) => false,
    }
}

/// 解密存储值。输入为 base64 加密串，返回明文（通常是 JSON 字符串）
pub fn decrypt_value(value: &str) -> Result<String> {
    let bytes = BASE64
        .decode(value.trim())
        .map_err(|e| anyhow!("base64 解码失败: {}", e))?;

    if bytes.len() < HEADER.len() + KEY_LEN + 16 {
        return Err(anyhow!("加密数据长度不足"));
    }
    if bytes[..HEADER.len()] != HEADER {
        return Err(anyhow!("加密数据头不匹配"));
    }

    let mut random_key = [0u8; KEY_LEN];
    random_key.copy_from_slice(&bytes[HEADER.len()..HEADER.len() + KEY_LEN]);
    let (aes_key, iv) = derive_key_iv(&random_key);

    let ciphertext = &bytes[HEADER.len() + KEY_LEN..];
    let plaintext = Aes128CbcDec::new(&aes_key.into(), &iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|e| anyhow!("AES 解密失败: {}", e))?;

    if plaintext.len() < CHECKSUM_LEN {
        return Err(anyhow!("解密后数据长度不足"));
    }
    let (checksum, data) = plaintext.split_at(CHECKSUM_LEN);
    if Sha512::digest(data).as_slice() != checksum {
        return Err(anyhow!("SHA-512 校验失败"));
    }

    String::from_utf8(data.to_vec()).map_err(|e| anyhow!("明文非 UTF-8: {}", e))
}

/// 加密存储值：返回 base64 加密串（与 Trae CN 客户端格式一致）
pub fn encrypt_value(plain: &str) -> Result<String> {
    use rand::RngCore;

    let mut random_key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut random_key);
    let (aes_key, iv) = derive_key_iv(&random_key);

    // 明文 = SHA512(data) + data
    let data = plain.as_bytes();
    let mut payload = Vec::with_capacity(CHECKSUM_LEN + data.len());
    payload.extend_from_slice(&Sha512::digest(data));
    payload.extend_from_slice(data);

    let ciphertext = Aes128CbcEnc::new(&aes_key.into(), &iv.into())
        .encrypt_padded_vec_mut::<Pkcs7>(&payload);

    let mut out = Vec::with_capacity(HEADER.len() + KEY_LEN + ciphertext.len());
    out.extend_from_slice(&HEADER);
    out.extend_from_slice(&random_key);
    out.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&out))
}

/// 读取存储值：加密则解密，否则原样返回（兼容旧版明文格式）
pub fn read_storage_value(value: &str) -> String {
    if is_encrypted(value) {
        match decrypt_value(value) {
            Ok(plain) => plain,
            Err(e) => {
                println!("[WARN] 解密存储值失败: {}", e);
                value.to_string()
            }
        }
    } else {
        value.to_string()
    }
}

/// 写入存储值：统一加密为 Trae CN 格式
pub fn write_storage_value(plain: &str) -> Result<String> {
    encrypt_value(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let plain = r#"{"token":"abc","userId":"123"}"#;
        let enc = encrypt_value(plain).unwrap();
        assert!(is_encrypted(&enc));
        let dec = decrypt_value(&enc).unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn test_plaintext_passthrough() {
        assert!(!is_encrypted(r#"{"token":"abc"}"#));
        assert_eq!(read_storage_value(r#"{"a":1}"#), r#"{"a":1}"#);
    }
}
