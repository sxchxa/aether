//! Carte réseau active et propriétés avancées.
//!
//! Lecture native (API IP Helper + Registre des pilotes : instantanée) ; écriture via
//! `Set-NetAdapterAdvancedProperty`, qui redémarre proprement la carte.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use windows_sys::core::GUID;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    ConvertInterfaceIndexToLuid, ConvertInterfaceLuidToGuid, GetBestInterface,
};
use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;

use crate::core::command::{self, ps_quote};
use crate::core::history::UndoAction;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, NIC_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::{AetherError, AetherResult};

/// Mots-clés NDIS gérés par Aether (liste blanche).
pub const MANAGED_KEYWORDS: &[&str] = &["*WakeOnMagicPacket", "*WakeOnPattern", "*EEE", "*InterruptModeration"];

/// Carte réseau active (portant la route par défaut).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    /// Nom (alias) de la carte, ex. `Ethernet`.
    #[serde(alias = "Name")]
    pub name: String,
    /// Description du pilote.
    #[serde(alias = "InterfaceDescription")]
    pub description: String,
    /// Index d'interface.
    #[serde(alias = "ifIndex")]
    pub index: u32,
    /// Débit négocié.
    #[serde(alias = "LinkSpeed", default)]
    pub link_speed: String,
    /// Adresse MAC.
    #[serde(alias = "MacAddress", default)]
    pub mac: String,
    /// GUID d'interface.
    #[serde(alias = "InterfaceGuid", default)]
    pub guid: String,
    /// Type de média physique (802.3, Native 802.11…).
    #[serde(alias = "PhysicalMediaType", default)]
    pub media: String,
}

/// Propriété avancée lue sur la carte.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NicProperty {
    /// Mot-clé NDIS (ex. `*EEE`).
    #[serde(alias = "RegistryKeyword")]
    pub keyword: String,
    /// Valeur brute.
    #[serde(alias = "Value", default)]
    pub value: String,
    /// Nom affiché par le pilote.
    #[serde(alias = "DisplayName", default)]
    pub display_name: String,
    /// Valeur affichée par le pilote.
    #[serde(alias = "DisplayValue", default)]
    pub display_value: String,
}

/// Instantané de l'état réseau.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NicSnapshot {
    /// Carte active (absente si hors ligne).
    pub adapter: Option<Adapter>,
    /// Propriétés gérées présentes sur la carte.
    pub properties: Vec<NicProperty>,
}

static CACHE: Mutex<Option<(Instant, NicSnapshot)>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(5);

/// Classe de périphériques « Carte réseau » (clé des pilotes et de leurs propriétés avancées).
const NET_CLASS: &str = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e972-e325-11ce-bfc1-08002be10318}";
const NET_CONNECTIONS: &str = "SYSTEM\\CurrentControlSet\\Control\\Network\\{4D36E972-E325-11CE-BFC1-08002BE10318}";

fn format_guid(g: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    )
}

/// Interface utilisée pour joindre Internet : (index, GUID).
fn best_interface() -> Option<(u32, String)> {
    let destination = u32::from_ne_bytes([8, 8, 8, 8]);
    let mut index = 0u32;
    // SAFETY : pointeurs vers des variables locales valides pendant les appels ; structures POD.
    unsafe {
        if GetBestInterface(destination, &mut index) != 0 {
            return None;
        }
        let mut luid: NET_LUID_LH = std::mem::zeroed();
        if ConvertInterfaceIndexToLuid(index, &mut luid) != 0 {
            return None;
        }
        let mut guid: GUID = std::mem::zeroed();
        if ConvertInterfaceLuidToGuid(&luid, &mut guid) != 0 {
            return None;
        }
        Some((index, format_guid(&guid)))
    }
}

/// Clé de pilote (`...\Class\{...}\00NN`) correspondant à une interface.
fn driver_key(guid: &str) -> Option<String> {
    registry::list_subkeys(Hive::Hklm, NET_CLASS)
        .into_iter()
        .find_map(|sub| {
            let key = format!("{NET_CLASS}\\{sub}");
            match registry::read_text(Hive::Hklm, &key, "NetCfgInstanceId") {
                Ok(Some(id)) if id.eq_ignore_ascii_case(guid) => Some(key),
                _ => None,
            }
        })
}

/// Détecte la carte réseau active.
///
/// Impact Windows : lecture seule (`GetBestInterface` + Registre des pilotes), sans PowerShell.
pub fn active_adapter() -> AetherResult<Option<Adapter>> {
    let Some((index, guid)) = best_interface() else {
        return Ok(None);
    };
    let Some(key) = driver_key(&guid) else {
        return Ok(None);
    };
    let name = registry::read_text(Hive::Hklm, &format!("{NET_CONNECTIONS}\\{guid}\\Connection"), "Name")?
        .unwrap_or_else(|| guid.clone());
    let media = match registry::read(Hive::Hklm, &key, "*PhysicalMediaType")? {
        Some(RegData::Dword(9)) => "Native 802.11",
        Some(RegData::Dword(14)) => "802.3",
        _ => "",
    };
    Ok(Some(Adapter {
        name,
        description: registry::read_text(Hive::Hklm, &key, "DriverDesc")?.unwrap_or_default(),
        index,
        link_speed: String::new(),
        mac: String::new(),
        guid,
        media: media.to_owned(),
    }))
}

fn read_properties(guid: &str) -> AetherResult<Vec<NicProperty>> {
    let Some(key) = driver_key(guid) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for kw in MANAGED_KEYWORDS {
        let ndi = format!("{key}\\Ndi\\params\\{kw}");
        if !registry::key_exists(Hive::Hklm, &ndi) {
            continue; // Propriété non prise en charge par ce pilote.
        }
        let value = match registry::read_text(Hive::Hklm, &key, kw)? {
            Some(v) => v,
            None => registry::read_text(Hive::Hklm, &ndi, "default")?.unwrap_or_default(),
        };
        let display_value =
            registry::read_text(Hive::Hklm, &format!("{ndi}\\enum"), &value)?.unwrap_or_else(|| value.clone());
        let display_name = registry::read_text(Hive::Hklm, &ndi, "ParamDesc")?
            .filter(|d| !d.starts_with('@'))
            .unwrap_or_else(|| kw.trim_start_matches('*').to_owned());
        out.push(NicProperty {
            keyword: (*kw).to_owned(),
            value,
            display_name,
            display_value,
        });
    }
    Ok(out)
}

/// Lit (avec cache de 5 s) la carte active et ses propriétés avancées.
///
/// Impact Windows : lecture seule.
pub fn snapshot(force: bool) -> AetherResult<NicSnapshot> {
    if !force {
        if let Some((at, snap)) = CACHE.lock()?.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return Ok(snap.clone());
            }
        }
    }
    let adapter = active_adapter()?;
    let properties = match &adapter {
        Some(a) => read_properties(&a.guid)?,
        None => Vec::new(),
    };
    let snap = NicSnapshot { adapter, properties };
    *CACHE.lock()? = Some((Instant::now(), snap.clone()));
    Ok(snap)
}

fn invalidate() {
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
}

/// Modifie une propriété avancée gérée (liste blanche) de la carte `adapter`.
///
/// Impact Windows : `Set-NetAdapterAdvancedProperty` — la carte redémarre brièvement
/// (coupure réseau de quelques secondes).
pub fn set_property(adapter: &str, keyword: &str, value: &str) -> AetherResult<()> {
    if !MANAGED_KEYWORDS.contains(&keyword) {
        return Err(AetherError::Forbidden(format!(
            "propriété {keyword} non gérée par Aether"
        )));
    }
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
        return Err(AetherError::InvalidInput(format!("valeur invalide : {value}")));
    }
    command::powershell(&format!(
        "Set-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -RegistryValue {}",
        ps_quote(adapter),
        ps_quote(keyword),
        ps_quote(value)
    ))?;
    invalidate();
    Ok(())
}

/// Option « mettre la propriété NIC `keyword` à 0 ».
pub struct NicTweak {
    /// Identifiant de l'option.
    pub id: &'static str,
    /// Mot-clé NDIS.
    pub keyword: &'static str,
    /// Risque.
    pub risk: Risk,
}

impl NicTweak {
    fn locate(&self) -> AetherResult<(Adapter, Option<NicProperty>)> {
        let snap = snapshot(false)?;
        let adapter = snap
            .adapter
            .ok_or_else(|| AetherError::NotFound("carte réseau active".into()))?;
        let prop = snap.properties.into_iter().find(|p| p.keyword == self.keyword);
        Ok((adapter, prop))
    }
}

impl Tweak for NicTweak {
    fn id(&self) -> &'static str {
        self.id
    }
    fn section(&self) -> Section {
        Section::Network
    }
    fn group(&self) -> &'static str {
        "nic"
    }
    fn risk(&self) -> Risk {
        self.risk
    }
    fn technical(&self) -> String {
        format!(
            "Set-NetAdapterAdvancedProperty -Name <carte active> -RegistryKeyword '{}' -RegistryValue 0",
            self.keyword
        )
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let snap = snapshot(false)?;
        let Some(adapter) = snap.adapter else {
            return Ok(Probe::unavailable("Aucune carte réseau active"));
        };
        match snap.properties.iter().find(|p| p.keyword == self.keyword) {
            None => Ok(Probe::unavailable(format!(
                "Non pris en charge par {}",
                adapter.description
            ))),
            Some(p) => Ok(Probe {
                applied: p.value == "0",
                available: true,
                current: format!("{} ({})", p.display_value, p.value),
                target: "Désactivé (0)".into(),
            }),
        }
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let (adapter, prop) = self.locate()?;
        let prop = prop.ok_or_else(|| AetherError::NotFound(self.keyword.into()))?;
        state
            .backups(NIC_BACKUP)
            .record_first(&format!("{}|{}", adapter.name, self.keyword), &prop.value)?;
        set_property(&adapter.name, self.keyword, "0")?;
        Ok(vec![UndoAction::NicProperty {
            adapter: adapter.name,
            keyword: self.keyword.to_owned(),
            value: prop.value,
        }])
    }

    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let (adapter, prop) = self.locate()?;
        let prop = prop.ok_or_else(|| AetherError::NotFound(self.keyword.into()))?;
        let key = format!("{}|{}", adapter.name, self.keyword);
        let backups = state.backups(NIC_BACKUP);
        let original = backups.get::<String>(&key)?.unwrap_or_else(|| "1".to_owned());
        set_property(&adapter.name, self.keyword, &original)?;
        backups.remove(&key)?;
        Ok(vec![UndoAction::NicProperty {
            adapter: adapter.name,
            keyword: self.keyword.to_owned(),
            value: prop.value,
        }])
    }
}

const PNP: &str = "PnPCapabilities";
/// Valeur documentée par Microsoft : « Autoriser l'ordinateur à éteindre ce périphérique » décoché.
const PNP_NO_POWER_SAVE: u32 = 24;

/// Option « la carte réseau n'est jamais mise hors tension par Windows » (gestion de l'alimentation).
pub struct NicPowerTweak;

impl NicPowerTweak {
    fn target() -> AetherResult<Option<(Adapter, String)>> {
        let Some(adapter) = active_adapter()? else {
            return Ok(None);
        };
        Ok(driver_key(&adapter.guid).map(|key| (adapter, key)))
    }
    fn backup_key(adapter: &Adapter) -> String {
        format!("{}|{PNP}", adapter.name)
    }
}

fn show_pnp(v: &Option<RegData>) -> String {
    v.as_ref().map(RegData::display).unwrap_or_else(|| "par défaut".into())
}

impl Tweak for NicPowerTweak {
    fn id(&self) -> &'static str {
        "nic.powerSaving"
    }
    fn section(&self) -> Section {
        Section::Network
    }
    fn group(&self) -> &'static str {
        "nic"
    }
    fn risk(&self) -> Risk {
        Risk::Safe
    }
    fn needs_reboot(&self) -> bool {
        true
    }
    fn technical(&self) -> String {
        format!("HKLM\\{NET_CLASS}\\<carte active>\\{PNP} = {PNP_NO_POWER_SAVE}")
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let Some((_, key)) = Self::target()? else {
            return Ok(Probe::unavailable("Aucune carte réseau active"));
        };
        let value = registry::read(Hive::Hklm, &key, PNP)?;
        Ok(Probe {
            applied: value == Some(RegData::Dword(PNP_NO_POWER_SAVE)),
            available: true,
            current: show_pnp(&value),
            target: PNP_NO_POWER_SAVE.to_string(),
        })
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let (adapter, key) = Self::target()?.ok_or_else(|| AetherError::NotFound("carte réseau active".into()))?;
        let current = registry::read(Hive::Hklm, &key, PNP)?;
        state
            .backups(NIC_BACKUP)
            .record_first(&Self::backup_key(&adapter), &current)?;
        registry::write(Hive::Hklm, &key, PNP, &RegData::Dword(PNP_NO_POWER_SAVE))?;
        Ok(vec![UndoAction::Registry {
            hive: Hive::Hklm,
            path: key,
            name: PNP.into(),
            previous: current,
        }])
    }

    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let (adapter, key) = Self::target()?.ok_or_else(|| AetherError::NotFound("carte réseau active".into()))?;
        let backups = state.backups(NIC_BACKUP);
        let original: Option<RegData> = backups.get::<Option<RegData>>(&Self::backup_key(&adapter))?.flatten();
        let current = registry::read(Hive::Hklm, &key, PNP)?;
        backups.remove(&Self::backup_key(&adapter))?;
        if current == original {
            return Ok(Vec::new());
        }
        match &original {
            Some(data) => registry::write(Hive::Hklm, &key, PNP, data)?,
            None => registry::delete_value(Hive::Hklm, &key, PNP)?,
        }
        Ok(vec![UndoAction::Registry {
            hive: Hive::Hklm,
            path: key,
            name: PNP.into(),
            previous: current,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_detection_is_consistent() -> AetherResult<()> {
        // Lecture seule : si une carte est active, son GUID et son nom sont résolus.
        if let Some(a) = active_adapter()? {
            assert!(a.guid.starts_with('{') && a.guid.len() == 38);
            assert!(!a.name.is_empty());
        }
        Ok(())
    }

    #[test]
    fn rejects_unmanaged_keywords_and_values() {
        assert!(set_property("Ethernet", "*SpeedDuplex", "0").is_err());
        assert!(set_property("Ethernet", "*EEE", "0; Remove-Item").is_err());
    }

    #[test]
    fn parses_powershell_adapter_json() -> AetherResult<()> {
        let json = r#"{"Name":"Ethernet","InterfaceDescription":"Intel(R) I225-V","ifIndex":12,"LinkSpeed":"1 Gbps","MacAddress":"AA-BB","InterfaceGuid":"{X}","PhysicalMediaType":"802.3"}"#;
        let a: Adapter = serde_json::from_str(json)?;
        assert_eq!(a.index, 12);
        assert_eq!(a.description, "Intel(R) I225-V");
        Ok(())
    }
}
