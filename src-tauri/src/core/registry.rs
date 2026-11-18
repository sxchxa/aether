//! Accès typé et sûr au Registre Windows (lecture d'état, écriture, suppression).

use serde::{Deserialize, Serialize};
use winreg::enums::{RegType, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WRITE};
use winreg::{RegKey, RegValue};

use crate::error::{AetherError, AetherResult};

/// Ruche de Registre ciblée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hive {
    /// `HKEY_CURRENT_USER`
    #[serde(rename = "HKCU")]
    Hkcu,
    /// `HKEY_LOCAL_MACHINE`
    #[serde(rename = "HKLM")]
    Hklm,
}

impl Hive {
    fn root(self) -> RegKey {
        match self {
            Hive::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
            Hive::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }

    /// Préfixe textuel (`HKCU` / `HKLM`) utilisé pour l'affichage et `reg.exe`.
    pub fn label(self) -> &'static str {
        match self {
            Hive::Hkcu => "HKCU",
            Hive::Hklm => "HKLM",
        }
    }
}

/// Donnée de Registre typée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum RegData {
    /// `REG_DWORD`
    Dword(u32),
    /// `REG_SZ`
    Sz(String),
    /// `REG_BINARY`
    Binary(Vec<u8>),
}

impl RegData {
    /// Représentation lisible (ex. `0xffffffff`, `"0"`).
    pub fn display(&self) -> String {
        match self {
            RegData::Dword(v) => format!("0x{v:08x} ({v})"),
            RegData::Sz(s) => format!("\"{s}\""),
            RegData::Binary(b) => b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" "),
        }
    }
}

fn full_path(hive: Hive, path: &str, name: &str) -> String {
    let value = if name.is_empty() { "(par défaut)" } else { name };
    format!("{}\\{}\\{}", hive.label(), path, value)
}

fn utf16_to_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units).trim_end_matches('\0').to_owned()
}

fn is_missing(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::NotFound
}

/// Lit une valeur de Registre. Renvoie `None` si la clé ou la valeur n'existe pas.
///
/// Impact Windows : lecture seule.
pub fn read(hive: Hive, path: &str, name: &str) -> AetherResult<Option<RegData>> {
    let key = match hive.root().open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => k,
        Err(e) if is_missing(&e) => return Ok(None),
        Err(e) => return Err(AetherError::registry(full_path(hive, path, name), &e)),
    };
    let raw = match key.get_raw_value(name) {
        Ok(v) => v,
        Err(e) if is_missing(&e) => return Ok(None),
        Err(e) => return Err(AetherError::registry(full_path(hive, path, name), &e)),
    };
    let bytes: &[u8] = &raw.bytes;
    let data = match raw.vtype {
        RegType::REG_DWORD if bytes.len() >= 4 => {
            RegData::Dword(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        RegType::REG_SZ | RegType::REG_EXPAND_SZ => RegData::Sz(utf16_to_string(bytes)),
        _ => RegData::Binary(bytes.to_vec()),
    };
    Ok(Some(data))
}

/// Lit une valeur `REG_MULTI_SZ` sous forme de liste de chaînes (vide si absente).
///
/// Impact Windows : lecture seule.
pub fn read_multi_sz(hive: Hive, path: &str, name: &str) -> AetherResult<Vec<String>> {
    let key = match hive.root().open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => k,
        Err(e) if is_missing(&e) => return Ok(Vec::new()),
        Err(e) => return Err(AetherError::registry(full_path(hive, path, name), &e)),
    };
    match key.get_raw_value(name) {
        Ok(raw) => Ok(utf16_to_string(&raw.bytes)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()),
        Err(e) if is_missing(&e) => Ok(Vec::new()),
        Err(e) => Err(AetherError::registry(full_path(hive, path, name), &e)),
    }
}

/// Liste les noms de valeurs d'une clé (vide si la clé n'existe pas).
///
/// Impact Windows : lecture seule.
pub fn list_values(hive: Hive, path: &str) -> AetherResult<Vec<(String, RegData)>> {
    let key = match hive.root().open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => k,
        Err(e) if is_missing(&e) => return Ok(Vec::new()),
        Err(e) => return Err(AetherError::registry(format!("{}\\{path}", hive.label()), &e)),
    };
    let mut out = Vec::new();
    for item in key.enum_values() {
        let (name, raw) = item.map_err(|e| AetherError::registry(format!("{}\\{path}", hive.label()), &e))?;
        let bytes: &[u8] = &raw.bytes;
        let data = match raw.vtype {
            RegType::REG_DWORD if bytes.len() >= 4 => {
                RegData::Dword(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            }
            RegType::REG_SZ | RegType::REG_EXPAND_SZ => RegData::Sz(utf16_to_string(bytes)),
            _ => RegData::Binary(bytes.to_vec()),
        };
        out.push((name, data));
    }
    Ok(out)
}

/// Liste les noms des sous-clés directes (vide si la clé n'existe pas ; sous-clés illisibles ignorées).
///
/// Impact Windows : lecture seule.
pub fn list_subkeys(hive: Hive, path: &str) -> Vec<String> {
    match hive.root().open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => key.enum_keys().filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

/// Lit une valeur et la renvoie sous forme de texte (`REG_SZ` tel quel, `REG_DWORD` en décimal).
///
/// Impact Windows : lecture seule.
pub fn read_text(hive: Hive, path: &str, name: &str) -> AetherResult<Option<String>> {
    Ok(match read(hive, path, name)? {
        Some(RegData::Sz(s)) => Some(s),
        Some(RegData::Dword(v)) => Some(v.to_string()),
        _ => None,
    })
}

/// Écrit une valeur de Registre (crée la clé si nécessaire).
///
/// Impact Windows : modifie le Registre ; certaines valeurs HKLM exigent l'élévation.
pub fn write(hive: Hive, path: &str, name: &str, data: &RegData) -> AetherResult<()> {
    let (key, _) = hive
        .root()
        .create_subkey_with_flags(path, KEY_WRITE)
        .map_err(|e| AetherError::registry(full_path(hive, path, name), &e))?;
    let res = match data {
        RegData::Dword(v) => key.set_value(name, v),
        RegData::Sz(s) => key.set_value(name, s),
        RegData::Binary(b) => key.set_raw_value(
            name,
            &RegValue {
                bytes: b.clone().into(),
                vtype: RegType::REG_BINARY,
            },
        ),
    };
    res.map_err(|e| AetherError::registry(full_path(hive, path, name), &e))
}

/// Supprime une valeur de Registre (aucune erreur si elle est déjà absente).
///
/// Impact Windows : modifie le Registre.
pub fn delete_value(hive: Hive, path: &str, name: &str) -> AetherResult<()> {
    let key = match hive.root().open_subkey_with_flags(path, KEY_SET_VALUE) {
        Ok(k) => k,
        Err(e) if is_missing(&e) => return Ok(()),
        Err(e) => return Err(AetherError::registry(full_path(hive, path, name), &e)),
    };
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(e) if is_missing(&e) => Ok(()),
        Err(e) => Err(AetherError::registry(full_path(hive, path, name), &e)),
    }
}

/// Indique si une clé existe.
///
/// Impact Windows : lecture seule.
pub fn key_exists(hive: Hive, path: &str) -> bool {
    hive.root().open_subkey_with_flags(path, KEY_READ).is_ok()
}

/// Supprime une clé et toutes ses sous-clés (aucune erreur si elle est absente).
///
/// Impact Windows : modifie le Registre de façon récursive.
pub fn delete_tree(hive: Hive, path: &str) -> AetherResult<()> {
    match hive.root().delete_subkey_all(path) {
        Ok(()) => Ok(()),
        Err(e) if is_missing(&e) => Ok(()),
        Err(e) => Err(AetherError::registry(format!("{}\\{path}", hive.label()), &e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "Software\\AetherTests\\Registry";

    #[test]
    fn roundtrip_values_in_sandbox_key() -> AetherResult<()> {
        write(Hive::Hkcu, TEST_KEY, "Dw", &RegData::Dword(0xffff_ffff))?;
        write(Hive::Hkcu, TEST_KEY, "Sz", &RegData::Sz("0".into()))?;
        write(Hive::Hkcu, TEST_KEY, "Bin", &RegData::Binary(vec![3, 0, 0, 0]))?;
        assert_eq!(read(Hive::Hkcu, TEST_KEY, "Dw")?, Some(RegData::Dword(0xffff_ffff)));
        assert_eq!(read(Hive::Hkcu, TEST_KEY, "Sz")?, Some(RegData::Sz("0".into())));
        assert_eq!(
            read(Hive::Hkcu, TEST_KEY, "Bin")?,
            Some(RegData::Binary(vec![3, 0, 0, 0]))
        );
        assert_eq!(list_values(Hive::Hkcu, TEST_KEY)?.len(), 3);
        delete_value(Hive::Hkcu, TEST_KEY, "Dw")?;
        assert_eq!(read(Hive::Hkcu, TEST_KEY, "Dw")?, None);
        delete_tree(Hive::Hkcu, "Software\\AetherTests\\Registry")?;
        assert!(!key_exists(Hive::Hkcu, TEST_KEY));
        Ok(())
    }

    #[test]
    fn missing_values_are_none() -> AetherResult<()> {
        assert_eq!(read(Hive::Hkcu, "Software\\AetherTests\\Absent", "X")?, None);
        delete_value(Hive::Hkcu, "Software\\AetherTests\\Absent", "X")?;
        Ok(())
    }
}
