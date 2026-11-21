// SPDX-License-Identifier: GPL-3.0-or-later
//! Criptarea jurnalului (opțională).
//!
//! O cheie a jurnalului (age X25519, generată la întâmplare) criptează fiecare fișier
//! în parte; numele fișierelor rămân aceleași. Cheia stă în rădăcina jurnalului de
//! două ori, încuiată cu parola și cu cheia de recuperare:
//! `lanruj-cheie-parola.age` și `lanruj-cheie-recuperare.age`.
//! Un fișier criptat se recunoaște după începutul lui (`age-encryption.org/v1`),
//! deci un jurnal pe jumătate criptat se citește în continuare.

use age::secrecy::{ExposeSecret, SecretString};
use std::io;
use std::path::{Path, PathBuf};

pub const MAGIC: &[u8] = b"age-encryption.org/v1\n";
const BY_PASSWORD: &str = "lanruj-cheie-parola.age";
const BY_RECOVERY: &str = "lanruj-cheie-recuperare.age";
/// Literele cheii de recuperare: fără 0/O și 1/I, ca să nu se încurce la copiere.
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const BECH32: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// E criptat conținutul acesta?
pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

/// Jurnalul din `root` are criptarea pornită?
pub fn enabled(root: &Path) -> bool {
    root.join(BY_PASSWORD).is_file()
}

/// Fișierele cheii (nu se criptează și nu se trec prin paginile jurnalului).
pub fn key_files(root: &Path) -> [PathBuf; 2] {
    [root.join(BY_PASSWORD), root.join(BY_RECOVERY)]
}

pub fn is_key_file(name: &str) -> bool {
    name == BY_PASSWORD || name == BY_RECOVERY
}

/// De ce nu s-a putut deschide jurnalul.
#[derive(Debug)]
pub enum Unlock {
    /// Parola sau cheia de recuperare nu e bună.
    Wrong,
    Io(io::Error),
}

impl std::fmt::Display for Unlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unlock::Wrong => write!(f, "parola nu e bună"),
            Unlock::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Cheia jurnalului, deschisă (stă doar în memorie).
pub struct Key {
    id: age::x25519::Identity,
    to: age::x25519::Recipient,
}

impl Key {
    fn new(id: age::x25519::Identity) -> Key {
        let to = id.to_public();
        Key { id, to }
    }

    pub fn encrypt(&self, plain: &[u8]) -> io::Result<Vec<u8>> {
        age::encrypt(&self.to, plain).map_err(io::Error::other)
    }

    pub fn decrypt(&self, data: &[u8]) -> io::Result<Vec<u8>> {
        age::decrypt(&self.id, data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

fn lock_with(secret: &str, id: &age::x25519::Identity) -> io::Result<Vec<u8>> {
    #[allow(unused_mut)]
    let mut r = age::scrypt::Recipient::new(SecretString::from(secret.to_string()));
    // La teste, scrypt mai ușor (altfel fiecare test durează o secundă).
    #[cfg(test)]
    r.set_work_factor(10);
    age::encrypt(&r, id.to_string().expose_secret().as_bytes()).map_err(io::Error::other)
}

fn open_with(secret: &str, path: &Path) -> Result<Key, Unlock> {
    let data = std::fs::read(path).map_err(Unlock::Io)?;
    let id = age::scrypt::Identity::new(SecretString::from(secret.to_string()));
    let plain = age::decrypt(&id, &data).map_err(|_| Unlock::Wrong)?;
    let text = String::from_utf8(plain).map_err(|_| Unlock::Wrong)?;
    let id = text.trim().parse::<age::x25519::Identity>().map_err(|_| Unlock::Wrong)?;
    Ok(Key::new(id))
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("age.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

/// Cheia de recuperare, curățată: litere mari, fără spații și liniuțe.
pub fn clean_code(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

/// Cheia de recuperare în grupuri de câte 5: „ABCDE-FGHJK-…”.
pub fn format_code(code: &str) -> String {
    let c = clean_code(code);
    c.as_bytes().chunks(5).map(|g| String::from_utf8_lossy(g).to_string()).collect::<Vec<_>>().join("-")
}

/// 30 de caractere întâmplătoare (150 de biți), luate din partea secretă a unei chei age noi.
fn new_code() -> String {
    let fresh = age::x25519::Identity::generate().to_string();
    let s = fresh.expose_secret().to_ascii_lowercase();
    let data = s.strip_prefix("age-secret-key-1").unwrap_or(&s);
    data.bytes()
        .take(30)
        .map(|b| ALPHABET[BECH32.iter().position(|x| *x == b).unwrap_or(0)] as char)
        .collect()
}

/// O cheie nouă a jurnalului și o cheie de recuperare nouă, doar în memorie
/// (nimic nu se scrie până nu confirmi că ai păstrat cheia de recuperare).
pub fn generate() -> (Key, String) {
    (Key::new(age::x25519::Identity::generate()), format_code(&new_code()))
}

/// Scrie cheia, încuiată cu parola și cu cheia de recuperare: de acum jurnalul e criptat.
/// Fișierele jurnalului se criptează separat (vezi `Journal::convert`).
pub fn store(root: &Path, key: &Key, password: &str, code: &str) -> io::Result<()> {
    std::fs::create_dir_all(root)?;
    write_atomic(&root.join(BY_RECOVERY), &lock_with(&clean_code(code), &key.id)?)?;
    write_atomic(&root.join(BY_PASSWORD), &lock_with(password, &key.id)?)
}

/// `generate` și `store` dintr-o dată.
pub fn create(root: &Path, password: &str) -> io::Result<(Key, String)> {
    let (key, code) = generate();
    store(root, &key, password, &code)?;
    Ok((key, code))
}

/// Deschide jurnalul cu parola.
pub fn unlock(root: &Path, password: &str) -> Result<Key, Unlock> {
    open_with(password, &root.join(BY_PASSWORD))
}

/// Deschide jurnalul cu cheia de recuperare (când parola e uitată).
pub fn recover(root: &Path, code: &str) -> Result<Key, Unlock> {
    open_with(&clean_code(code), &root.join(BY_RECOVERY))
}

/// Parolă nouă pentru aceeași cheie (fișierele jurnalului nu se schimbă).
pub fn set_password(root: &Path, key: &Key, password: &str) -> io::Result<()> {
    write_atomic(&root.join(BY_PASSWORD), &lock_with(password, &key.id)?)
}

/// Oprește criptarea: șterge cheile (după ce fișierele au fost decriptate).
pub fn remove(root: &Path) -> io::Result<()> {
    for p in key_files(root) {
        match std::fs::remove_file(&p) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parola_si_recuperare() {
        let root = std::env::temp_dir().join(format!("lanruj-crypt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert!(!enabled(&root));
        let (key, code) = create(&root, "parola bună").unwrap();
        assert!(enabled(&root));
        assert_eq!(clean_code(&code).len(), 30);
        assert_eq!(code.matches('-').count(), 5, "6 grupuri de câte 5: {code}");
        assert!(clean_code(&code).bytes().all(|b| ALPHABET.contains(&b)));

        let data = key.encrypt("- [ ] Sună la bancă\n".as_bytes()).unwrap();
        assert!(is_encrypted(&data));
        assert!(!is_encrypted(b"# zi\n"));

        assert!(matches!(unlock(&root, "greșită"), Err(Unlock::Wrong)));
        let k2 = unlock(&root, "parola bună").unwrap();
        assert_eq!(k2.decrypt(&data).unwrap(), "- [ ] Sună la bancă\n".as_bytes());
        // Cheia de recuperare merge și scrisă cu litere mici și fără liniuțe.
        let k3 = recover(&root, &clean_code(&code).to_lowercase()).unwrap();
        assert_eq!(k3.decrypt(&data).unwrap(), "- [ ] Sună la bancă\n".as_bytes());
        assert!(matches!(recover(&root, "AAAAA-BBBBB"), Err(Unlock::Wrong)));

        set_password(&root, &k3, "nouă").unwrap();
        assert!(matches!(unlock(&root, "parola bună"), Err(Unlock::Wrong)));
        assert_eq!(unlock(&root, "nouă").unwrap().decrypt(&data).unwrap(), "- [ ] Sună la bancă\n".as_bytes());

        remove(&root).unwrap();
        assert!(!enabled(&root));
        std::fs::remove_dir_all(root).unwrap();
    }
}
