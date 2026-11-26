//! AES-ECB / AES-CTR through Windows' own CNG (bcrypt.dll): no crypto crate needed.
use windows_sys::Win32::Security::Cryptography::*;

fn check(status: i32, what: &str) -> Result<(), String> {
    if status != 0 { Err(format!("{what} falló (0x{:08X}).", status as u32)) } else { Ok(()) }
}

/// The AES provider (ECB mode) is opened once; CNG algorithm handles may be shared between threads.
fn provider() -> Result<BCRYPT_ALG_HANDLE, String> {
    static HANDLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    if let Some(h) = HANDLE.get() { return Ok(*h as BCRYPT_ALG_HANDLE); }
    let mode: Vec<u16> = "ChainingModeECB\0".encode_utf16().collect();
    unsafe {
        let mut alg: BCRYPT_ALG_HANDLE = std::ptr::null_mut();
        check(BCryptOpenAlgorithmProvider(&mut alg, BCRYPT_AES_ALGORITHM, std::ptr::null(), 0), "BCryptOpenAlgorithmProvider")?;
        check(BCryptSetProperty(alg, BCRYPT_CHAINING_MODE, mode.as_ptr() as *const u8, (mode.len() * 2) as u32, 0), "BCryptSetProperty")?;
        Ok(*HANDLE.get_or_init(|| alg as usize) as BCRYPT_ALG_HANDLE)
    }
}

fn ecb(key: &[u8], data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    if data.len() % 16 != 0 { return Err("Los datos AES deben ser múltiplo de 16 bytes.".into()); }
    if data.is_empty() { return Ok(Vec::new()); }
    let alg = provider()?;
    unsafe {
        let mut handle: BCRYPT_KEY_HANDLE = std::ptr::null_mut();
        check(BCryptGenerateSymmetricKey(alg, &mut handle, std::ptr::null_mut(), 0, key.as_ptr(), key.len() as u32, 0), "BCryptGenerateSymmetricKey")?;
        let mut out = vec![0u8; data.len()];
        let mut done = 0u32;
        let status = if encrypt {
            BCryptEncrypt(handle, data.as_ptr(), data.len() as u32, std::ptr::null(), std::ptr::null_mut(), 0, out.as_mut_ptr(), out.len() as u32, &mut done, 0)
        } else {
            BCryptDecrypt(handle, data.as_ptr(), data.len() as u32, std::ptr::null(), std::ptr::null_mut(), 0, out.as_mut_ptr(), out.len() as u32, &mut done, 0)
        };
        BCryptDestroyKey(handle);
        check(status, if encrypt { "BCryptEncrypt" } else { "BCryptDecrypt" })?;
        out.truncate(done as usize);
        Ok(out)
    }
}

pub fn ecb_encrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> { ecb(key, data, true) }
pub fn ecb_decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> { ecb(key, data, false) }

/// AES-CTR with a 16-byte initial counter block, incremented as one big-endian 128-bit number.
pub fn ctr(key: &[u8], counter: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, String> { ctr_at(key, counter, 0, data) }

/// CTR keystream starting `block` blocks after the initial counter (decrypt part of a stream).
pub fn ctr_at(key: &[u8], counter: &[u8; 16], block: u128, data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() { return Ok(Vec::new()); }
    let start = u128::from_be_bytes(*counter).wrapping_add(block);
    let blocks = data.len().div_ceil(16);
    let counters: Vec<u8> = (0..blocks).flat_map(|i| start.wrapping_add(i as u128).to_be_bytes()).collect();
    let stream = ecb_encrypt(key, &counters)?;
    Ok(data.iter().zip(stream).map(|(a, b)| a ^ b).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> { (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect() }
    #[test]
    fn fips197_aes256() {
        let key = hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let plain = hex("00112233445566778899aabbccddeeff");
        let cipher = ecb_encrypt(&key, &plain).unwrap();
        assert_eq!(cipher, hex("8ea2b7ca516745bfeafc49904b496089"));
        assert_eq!(ecb_decrypt(&key, &cipher).unwrap(), plain);
    }
    #[test]
    fn ctr_roundtrip_and_counter_carry() {
        let key = [7u8; 32];
        let counter = [0xffu8; 16];
        let data: Vec<u8> = (0..40u8).collect();
        let once = ctr(&key, &counter, &data).unwrap();
        assert_eq!(ctr(&key, &counter, &once).unwrap(), data);
        // second block uses counter 0 after wrapping
        let second = ecb_encrypt(&key, &[0u8; 16]).unwrap();
        assert_eq!(once[16] ^ data[16], second[0]);
        // a later slice decrypts on its own
        assert_eq!(ctr_at(&key, &counter, 1, &once[16..]).unwrap(), data[16..]);
    }
}
