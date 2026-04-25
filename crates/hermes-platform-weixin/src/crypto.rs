use base64::{engine::general_purpose, Engine as _};

/// AES-128-ECB 加密
pub fn aes128_ecb_encrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 16 {
        return Err("密钥必须为 16 字节".to_string());
    }

    use aes::Aes128;
    use aes::cipher::{BlockEncrypt, KeyInit};

    let cipher = Aes128::new_from_slice(key)
        .map_err(|_| "Failed to create cipher")?;

    // PKCS7 padding
    let block_size = 16;
    let padding = block_size - (data.len() % block_size);
    let mut padded = data.to_vec();
    padded.extend(vec![padding as u8; padding]);

    // ECB 模式加密 - 手动处理每个块
    let mut result = Vec::new();
    for chunk in padded.chunks(16) {
        let mut block = aes::cipher::Block::<Aes128>::from_slice(chunk).clone();
        cipher.encrypt_block(&mut block);
        result.extend_from_slice(&block);
    }

    Ok(result)
}

/// Base64 编码
pub fn base64_encode(data: &[u8]) -> String {
    general_purpose::STANDARD.encode(data)
}

/// Base64 解码
pub fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())
}
