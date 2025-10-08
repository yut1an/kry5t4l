use rc4::{KeyInit, StreamCipher};
use rc4::{Rc4};

pub struct Rc4Cipher;
impl Rc4Cipher {

    pub fn encrypt(data: &[u8]) -> Vec<u8> {
        let mut cipher = Rc4::new(b"Kdog3208".into());
        let mut out = vec![0u8; data.len()];
        let _ = cipher.apply_keystream_b2b(data, &mut out);
        out
    }

    pub fn decrypt(data: &[u8]) -> Vec<u8> {
        Self::encrypt(data)
    }
}