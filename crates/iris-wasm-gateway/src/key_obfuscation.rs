//! 密钥混淆与隐藏
//!
//! 实现密钥分散存储和运行时重建机制，防止静态分析提取密钥

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::Rng;
use crate::Result;

const KEY_SIZE: usize = 32;
const NUM_SHARDS: usize = 4;

#[derive(Debug, Clone)]
pub struct KeyShard {
    pub index: usize,
    pub data: Vec<u8>,
    pub xor_mask: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ObfuscatedKey {
    pub shards: Vec<KeyShard>,
    pub checksum: u32,
    pub version: u32,
}

impl ObfuscatedKey {
    pub fn from_key(key: &[u8]) -> Result<Self> {
        if key.len() != KEY_SIZE {
            return Err(crate::Error::Key("Invalid key length".to_string()));
        }

        let mut rng = rand::thread_rng();

        let mut shards = Vec::with_capacity(NUM_SHARDS);
        let shard_size = KEY_SIZE / NUM_SHARDS;

        for i in 0..NUM_SHARDS {
            let start = i * shard_size;
            let end = if i == NUM_SHARDS - 1 { KEY_SIZE } else { (i + 1) * shard_size };
            let shard_data = key[start..end].to_vec();

            let xor_mask: Vec<u8> = (0..shard_data.len())
                .map(|_| rng.gen::<u8>())
                .collect();

            let obfuscated: Vec<u8> = shard_data.iter()
                .zip(xor_mask.iter())
                .map(|(d, m)| d ^ m)
                .collect();

            shards.push(KeyShard {
                index: i,
                data: obfuscated,
                xor_mask,
            });
        }

        let checksum = Self::calculate_checksum(key);

        Ok(ObfuscatedKey {
            shards,
            checksum,
            version: 1,
        })
    }

    pub fn reconstruct_key(&self) -> Result<Vec<u8>> {
        let mut key = Vec::with_capacity(KEY_SIZE);

        let mut sorted_shards = self.shards.clone();
        sorted_shards.sort_by_key(|s| s.index);

        for shard in sorted_shards {
            let original: Vec<u8> = shard.data.iter()
                .zip(shard.xor_mask.iter())
                .map(|(d, m)| d ^ m)
                .collect();
            key.extend_from_slice(&original);
        }

        if Self::calculate_checksum(&key) != self.checksum {
            return Err(crate::Error::Key("Key reconstruction failed - checksum mismatch".to_string()));
        }

        Ok(key)
    }

    fn calculate_checksum(key: &[u8]) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as u32
    }

    pub fn to_obfuscated_code(&self) -> String {
        let mut code = String::new();

        code.push_str("// Key shards (obfuscated)\n");
        for shard in &self.shards {
            let data_hex = hex::encode(&shard.data);
            let mask_hex = hex::encode(&shard.xor_mask);
            code.push_str(&format!(
                "const SHARD_{}_DATA: &[u8] = &hex_decode(\"{}\").unwrap();\n",
                shard.index, data_hex
            ));
            code.push_str(&format!(
                "const SHARD_{}_MASK: &[u8] = &hex_decode(\"{}\").unwrap();\n",
                shard.index, mask_hex
            ));
        }

        code.push_str(&format!(
            "\nconst KEY_CHECKSUM: u32 = {};\n",
            self.checksum
        ));
        code.push_str(&format!(
            "const KEY_VERSION: u32 = {};\n",
            self.version
        ));

        code
    }
}

pub struct KeyObfuscator;

impl KeyObfuscator {
    pub fn obfuscate_key(key: &[u8]) -> Result<ObfuscatedKey> {
        ObfuscatedKey::from_key(key)
    }

    pub fn generate_dummy_operations() -> Vec<String> {
        let mut ops = Vec::new();

        ops.push("let _dummy1 = (0x45 ^ 0x23) as u8;".to_string());
        ops.push("let _dummy2 = [0x12, 0x34, 0x56].iter().sum::<u8>();".to_string());
        ops.push("let _dummy3 = \"decoy\".len() ^ 0xFF;".to_string());
        ops.push("let _dummy4 = (0xAB + 0xCD) & 0xEF;".to_string());

        ops
    }

    pub fn generate_key_reconstruction_code() -> String {
        r#"
fn reconstruct_key() -> Vec<u8> {
    let mut key = Vec::with_capacity(32);

    let shards = [
        (SHARD_0_DATA, SHARD_0_MASK),
        (SHARD_1_DATA, SHARD_1_MASK),
        (SHARD_2_DATA, SHARD_2_MASK),
        (SHARD_3_DATA, SHARD_3_MASK),
    ];

    let _dummy1 = (0x45 ^ 0x23) as u8;
    let _dummy2 = [0x12, 0x34, 0x56].iter().sum::<u8>();

    for (data, mask) in shards.iter() {
        let original: Vec<u8> = data.iter()
            .zip(mask.iter())
            .map(|(d, m)| d ^ m)
            .collect();
        key.extend_from_slice(original);
    }

    let _dummy3 = "decoy".len() ^ 0xFF;

    let checksum = calculate_checksum(&key);
    if checksum != KEY_CHECKSUM {
        panic!("Key validation failed");
    }

    key
}

fn calculate_checksum(key: &[u8]) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as u32
}
"#.to_string()
    }
}

pub fn hex_decode(s: &str) -> Result<Vec<u8>> {
    hex::decode(s).map_err(|e| crate::Error::Key(format!("Hex decode failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_obfuscation() {
        let key: Vec<u8> = (0..32).collect();

        let obfuscated = ObfuscatedKey::from_key(&key).unwrap();

        assert_eq!(obfuscated.shards.len(), NUM_SHARDS);

        let reconstructed = obfuscated.reconstruct_key().unwrap();
        assert_eq!(key, reconstructed);
    }

    #[test]
    fn test_shard_xor() {
        let original_data = vec![0x12, 0x34, 0x56, 0x78];
        let xor_mask = vec![0xAB, 0xCD, 0xEF, 0x01];

        let obfuscated: Vec<u8> = original_data.iter()
            .zip(xor_mask.iter())
            .map(|(d, m)| d ^ m)
            .collect();

        let reconstructed: Vec<u8> = obfuscated.iter()
            .zip(xor_mask.iter())
            .map(|(d, m)| d ^ m)
            .collect();

        assert_eq!(original_data, reconstructed);
    }

    #[test]
    fn test_checksum() {
        let key1: Vec<u8> = (0..32).collect();
        let key2: Vec<u8> = (1..33).collect();

        let checksum1 = ObfuscatedKey::calculate_checksum(&key1);
        let checksum2 = ObfuscatedKey::calculate_checksum(&key2);

        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_obfuscated_code_generation() {
        let key: Vec<u8> = (0..32).collect();
        let obfuscated = ObfuscatedKey::from_key(&key).unwrap();

        let code = obfuscated.to_obfuscated_code();

        assert!(code.contains("SHARD_0_DATA"));
        assert!(code.contains("SHARD_1_DATA"));
        assert!(code.contains("SHARD_2_DATA"));
        assert!(code.contains("SHARD_3_DATA"));
        assert!(code.contains("KEY_CHECKSUM"));
    }
}
