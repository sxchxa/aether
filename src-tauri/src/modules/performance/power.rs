//! Modes de gestion de l'alimentation (`powercfg`) et profil « Performances Optimales ».

use serde::Serialize;

use crate::core::command;
use crate::core::history::UndoAction;
use crate::core::state::{AppState, MISC_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::{AetherError, AetherResult};

/// GUID du modèle Microsoft « Performances optimales » (Ultimate Performance).
pub const ULTIMATE_BASE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
/// GUID fixe de la copie créée par Aether (garantit l'idempotence : jamais de doublon).
pub const AETHER_ULTIMATE: &str = "ae7be7a0-e9a4-4b02-a5df-448daa000001";
/// GUID du mode « Utilisation normale » (Balanced).
pub const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

const ULTIMATE_NAMES: &[&str] = &[
    "ultimate performance",
    "performances optimales",
    "máximo rendimiento",
    "ultimative leistung",
    "prestazioni eccellenti",
];

/// Mode d'alimentation installé.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PowerScheme {
    /// GUID du mode.
    pub guid: String,
    /// Nom localisé.
    pub name: String,
    /// Mode actuellement actif.
    pub active: bool,
}

fn find_guid(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    if bytes.len() < 36 {
        return None;
    }
    (0..=bytes.len() - 36).find_map(|i| {
        let cand = line.get(i..i + 36)?;
        let ok = cand.char_indices().all(|(j, c)| {
            if matches!(j, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        });
        ok.then(|| cand.to_ascii_lowercase())
    })
}

/// Analyse la sortie de `powercfg /list`.
pub fn parse_list(output: &str) -> Vec<PowerScheme> {
    output
        .lines()
        .filter_map(|line| {
            let guid = find_guid(line)?;
            let name = match (line.find('('), line.rfind(')')) {
                (Some(a), Some(b)) if b > a => line[a + 1..b].trim().to_owned(),
                _ => guid.clone(),
            };
            Some(PowerScheme {
                guid,
                name,
                active: line.trim_end().ends_with('*'),
            })
        })
        .collect()
}

/// Liste les modes d'alimentation installés.
///
/// Impact Windows : lecture seule (`powercfg /list`).
pub fn list_schemes() -> AetherResult<Vec<PowerScheme>> {
    Ok(parse_list(&command::run("powercfg.exe", &["/list"])?.stdout))
}

/// GUID du mode actif.
///
/// Impact Windows : lecture seule (`powercfg /getactivescheme`).
pub fn active_guid() -> AetherResult<String> {
    let out = command::run("powercfg.exe", &["/getactivescheme"])?;
    find_guid(&out.stdout).ok_or_else(|| AetherError::NotFound("mode d'alimentation actif".into()))
}

/// Active un mode d'alimentation.
///
/// Impact Windows : `powercfg /setactive <guid>`.
pub fn set_active(guid: &str) -> AetherResult<()> {
    if find_guid(guid).is_none() {
        return Err(AetherError::InvalidInput(format!("GUID invalide : {guid}")));
    }
    command::run("powercfg.exe", &["/setactive", guid])?;
    Ok(())
}

fn is_ultimate(scheme: &PowerScheme) -> bool {
    let name = scheme.name.to_lowercase();
    scheme.guid == ULTIMATE_BASE || scheme.guid == AETHER_ULTIMATE || ULTIMATE_NAMES.iter().any(|n| name.contains(n))
}

/// Débloque le mode « Performances optimales » s'il est absent et renvoie son GUID.
///
/// Impact Windows : `powercfg -duplicatescheme e9a42b02-… <GUID Aether>` (une seule fois).
pub fn ensure_ultimate() -> AetherResult<String> {
    if let Some(existing) = list_schemes()?.into_iter().find(is_ultimate) {
        return Ok(existing.guid);
    }
    command::run("powercfg.exe", &["-duplicatescheme", ULTIMATE_BASE, AETHER_ULTIMATE])?;
    Ok(AETHER_ULTIMATE.to_owned())
}

/// Option « Activer le mode Performances Optimales ».
pub struct UltimatePowerTweak;

impl Tweak for UltimatePowerTweak {
    fn id(&self) -> &'static str {
        "power.ultimate"
    }
    fn section(&self) -> Section {
        Section::Power
    }
    fn group(&self) -> &'static str {
        "plan"
    }
    fn risk(&self) -> Risk {
        Risk::Moderate
    }
    fn technical(&self) -> String {
        format!("powercfg -duplicatescheme {ULTIMATE_BASE} {AETHER_ULTIMATE}\npowercfg /setactive <GUID>")
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let schemes = list_schemes()?;
        let active = schemes.iter().find(|s| s.active);
        Ok(Probe {
            applied: active.is_some_and(is_ultimate),
            available: true,
            current: active.map(|s| s.name.clone()).unwrap_or_else(|| "inconnu".into()),
            target: "Performances optimales".into(),
        })
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let previous = active_guid()?;
        state.backups(MISC_BACKUP).record_first("power.active", &previous)?;
        let guid = ensure_ultimate()?;
        set_active(&guid)?;
        Ok(vec![UndoAction::PowerScheme { guid: previous }])
    }

    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let backups = state.backups(MISC_BACKUP);
        let original = backups
            .get::<String>("power.active")?
            .unwrap_or_else(|| BALANCED.to_owned());
        let current = active_guid()?;
        set_active(&original)?;
        backups.remove("power.active")?;
        Ok(vec![UndoAction::PowerScheme { guid: current }])
    }
}

/// Extrait la valeur « sur secteur » de la sortie de `powercfg /query <mode> <sous-groupe> <paramètre>`.
///
/// Les deux dernières valeurs hexadécimales du bloc sont l'index actuel sur secteur puis sur
/// batterie (format identique quelle que soit la langue de Windows).
pub fn parse_ac_value(output: &str) -> Option<u32> {
    let values: Vec<u32> = output
        .lines()
        .filter_map(|l| {
            let hex = l.split_once("0x")?.1.trim();
            u32::from_str_radix(hex.get(..8).unwrap_or(hex), 16).ok()
        })
        .collect();
    if values.len() < 2 {
        return None;
    }
    values.get(values.len() - 2).copied()
}

fn valid_guid(value: &str) -> AetherResult<()> {
    if find_guid(value).is_none() {
        return Err(AetherError::InvalidInput(format!("GUID invalide : {value}")));
    }
    Ok(())
}

/// Lit la valeur « sur secteur » d'un paramètre d'un mode d'alimentation (`None` si absent).
///
/// Impact Windows : lecture seule (`powercfg /query`).
pub fn query_ac_value(scheme: &str, subgroup: &str, setting: &str) -> AetherResult<Option<u32>> {
    let out = command::run_raw("powercfg.exe", &["/query", scheme, subgroup, setting])?;
    Ok(if out.success() {
        parse_ac_value(&out.stdout)
    } else {
        None
    })
}

/// Définit la valeur « sur secteur » d'un paramètre puis réapplique le mode.
///
/// Impact Windows : `powercfg /setacvalueindex <mode> <sous-groupe> <paramètre> <valeur>` puis
/// `powercfg /setactive <mode>` si ce mode est actif.
pub fn set_ac_value(scheme: &str, subgroup: &str, setting: &str, value: u32) -> AetherResult<()> {
    valid_guid(scheme)?;
    valid_guid(subgroup)?;
    valid_guid(setting)?;
    command::run(
        "powercfg.exe",
        &["/setacvalueindex", scheme, subgroup, setting, &value.to_string()],
    )?;
    if active_guid()?.eq_ignore_ascii_case(scheme) {
        command::run("powercfg.exe", &["/setactive", scheme])?;
    }
    Ok(())
}

/// Option « paramètre d'alimentation sur secteur » (mode actif).
pub struct PowercfgTweak {
    /// Identifiant.
    pub id: &'static str,
    /// GUID du sous-groupe.
    pub subgroup: &'static str,
    /// GUID du paramètre.
    pub setting: &'static str,
    /// Valeur optimisée.
    pub target: u32,
    /// Valeur par défaut de Windows (mode « Utilisation normale »), utilisée sans sauvegarde.
    pub default: u32,
    /// Unité affichée.
    pub unit: &'static str,
    /// Risque.
    pub risk: Risk,
}

impl PowercfgTweak {
    fn backup_key(&self, scheme: &str) -> String {
        format!("powercfg.{scheme}.{}", self.setting)
    }
    fn show(&self, v: u32) -> String {
        format!("{v}{}", self.unit)
    }
}

impl Tweak for PowercfgTweak {
    fn id(&self) -> &'static str {
        self.id
    }
    fn section(&self) -> Section {
        Section::Power
    }
    fn group(&self) -> &'static str {
        "settings"
    }
    fn risk(&self) -> Risk {
        self.risk
    }
    fn technical(&self) -> String {
        format!(
            "powercfg /setacvalueindex SCHEME_CURRENT {} {} {}\npowercfg /setactive SCHEME_CURRENT",
            self.subgroup, self.setting, self.target
        )
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let scheme = active_guid()?;
        match query_ac_value(&scheme, self.subgroup, self.setting)? {
            None => Ok(Probe::unavailable("Paramètre absent de ce mode d'alimentation")),
            Some(v) => Ok(Probe {
                applied: v == self.target,
                available: true,
                current: self.show(v),
                target: self.show(self.target),
            }),
        }
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let scheme = active_guid()?;
        let previous = query_ac_value(&scheme, self.subgroup, self.setting)?.unwrap_or(self.default);
        state
            .backups(MISC_BACKUP)
            .record_first(&self.backup_key(&scheme), &previous)?;
        set_ac_value(&scheme, self.subgroup, self.setting, self.target)?;
        Ok(vec![UndoAction::PowerSetting {
            scheme,
            subgroup: self.subgroup.to_owned(),
            setting: self.setting.to_owned(),
            value: previous,
        }])
    }

    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let scheme = active_guid()?;
        let backups = state.backups(MISC_BACKUP);
        let key = self.backup_key(&scheme);
        let original = backups.get::<u32>(&key)?.unwrap_or(self.default);
        let current = query_ac_value(&scheme, self.subgroup, self.setting)?.unwrap_or(self.target);
        set_ac_value(&scheme, self.subgroup, self.setting, original)?;
        backups.remove(&key)?;
        Ok(vec![UndoAction::PowerSetting {
            scheme,
            subgroup: self.subgroup.to_owned(),
            setting: self.setting.to_owned(),
            value: current,
        }])
    }
}

/// Paramètres d'alimentation documentés par Microsoft (`powercfg /aliases`).
pub fn powercfg_tweaks() -> Vec<PowercfgTweak> {
    vec![
        PowercfgTweak {
            id: "power.usbSuspend",
            subgroup: "2a737441-1930-4402-8d77-b2bebba308a3",
            setting: "48e6b7a6-50f5-4782-a5d4-53bb8f07e226",
            target: 0,
            default: 1,
            unit: "",
            risk: Risk::Safe,
        },
        PowercfgTweak {
            id: "power.pcieAspm",
            subgroup: "501a4d13-42af-4429-9fd1-a8218c268e20",
            setting: "ee12f906-d277-404b-b6da-e5fa1a576df5",
            target: 0,
            default: 1,
            unit: "",
            risk: Risk::Moderate,
        },
        PowercfgTweak {
            id: "power.cpuMin",
            subgroup: "54533251-82be-4824-96c1-47b60b740d00",
            setting: "893dee8e-2bef-41e0-89c6-b55d0929964c",
            target: 100,
            default: 5,
            unit: " %",
            risk: Risk::Moderate,
        },
        PowercfgTweak {
            id: "power.diskIdle",
            subgroup: "0012ee47-9041-4b5d-9b77-535fba8b1442",
            setting: "6738e2c4-e8a5-4a42-b16a-e040e769756e",
            target: 0,
            default: 1200,
            unit: " s",
            risk: Risk::Safe,
        },
    ]
}

const POWER_KEY: &str = "SYSTEM\\CurrentControlSet\\Control\\Power";

/// Active ou désactive l'hibernation.
///
/// Impact Windows : `powercfg /hibernate on|off` (supprime ou recrée `hiberfil.sys`).
pub fn set_hibernate(enabled: bool) -> AetherResult<()> {
    command::run("powercfg.exe", &["/hibernate", if enabled { "on" } else { "off" }])?;
    Ok(())
}

/// Le PC possède une batterie (portable, tablette).
///
/// Impact Windows : lecture seule (`GetSystemPowerStatus`).
pub fn has_battery() -> bool {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    // SAFETY : structure POD initialisée à zéro, remplie par l'API.
    unsafe {
        let mut st: SYSTEM_POWER_STATUS = std::mem::zeroed();
        // BatteryFlag : 128 = aucune batterie, 255 = état inconnu (traité comme « pas de batterie »).
        GetSystemPowerStatus(&mut st) != 0 && st.BatteryFlag != 128 && st.BatteryFlag != 255
    }
}

/// Réglages réservés aux PC fixes : sur batterie, ils chauffent et vident la batterie sans bénéfice.
pub const DESKTOP_ONLY: &[&str] = &["power.cpuMin", "power.pcieAspm"];

/// Indique si une option doit être écartée d'un profil automatique sur ce PC.
pub fn skip_on_this_pc(id: &str) -> bool {
    DESKTOP_ONLY.contains(&id) && has_battery()
}

/// Indique si le matériel prend en charge la mise en veille prolongée (état S4).
///
/// Impact Windows : lecture seule (`GetPwrCapabilities`).
pub fn hibernate_supported() -> bool {
    use windows_sys::Win32::System::Power::{GetPwrCapabilities, SYSTEM_POWER_CAPABILITIES};
    // SAFETY : structure POD initialisée à zéro, remplie par l'API.
    unsafe {
        let mut caps: SYSTEM_POWER_CAPABILITIES = std::mem::zeroed();
        GetPwrCapabilities(&mut caps) && caps.SystemS4
    }
}

fn hibernate_enabled() -> AetherResult<bool> {
    Ok(!matches!(
        crate::core::registry::read(crate::core::registry::Hive::Hklm, POWER_KEY, "HibernateEnabled")?,
        Some(crate::core::registry::RegData::Dword(0))
    ))
}

/// Option « Désactiver l'hibernation » (libère l'espace de `hiberfil.sys`).
pub struct HibernateTweak;

impl Tweak for HibernateTweak {
    fn id(&self) -> &'static str {
        "power.hibernate"
    }
    fn section(&self) -> Section {
        Section::Power
    }
    fn group(&self) -> &'static str {
        "settings"
    }
    fn risk(&self) -> Risk {
        Risk::Moderate
    }
    fn technical(&self) -> String {
        format!("powercfg /hibernate off\nHKLM\\{POWER_KEY}\\HibernateEnabled")
    }
    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        if !hibernate_supported() {
            return Ok(Probe::unavailable(
                "Mise en veille prolongée non prise en charge par ce PC",
            ));
        }
        if has_battery() {
            // Sur portable, l'hibernation sauve le travail en cours quand la batterie est critique.
            return Ok(Probe::unavailable(
                "Conservée sur PC portable : protège votre travail en cas de batterie critique",
            ));
        }
        let enabled = hibernate_enabled()?;
        Ok(Probe {
            applied: !enabled,
            available: true,
            current: if enabled { "Activée" } else { "Désactivée" }.into(),
            target: "Désactivée".into(),
        })
    }
    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        state.backups(MISC_BACKUP).record_first("power.hibernate", &true)?;
        set_hibernate(false)?;
        Ok(vec![UndoAction::Hibernate { enabled: true }])
    }
    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        set_hibernate(true)?;
        state.backups(MISC_BACKUP).remove("power.hibernate")?;
        Ok(vec![UndoAction::Hibernate { enabled: false }])
    }
}

/// Stratégie officielle « Désactiver la limitation de puissance » (ADMX_Power).
pub static THROTTLING_TARGETS: &[crate::core::reg_tweak::RegTarget] = &[crate::core::reg_tweak::RegTarget {
    hive: crate::core::registry::Hive::Hklm,
    path: "SYSTEM\\CurrentControlSet\\Control\\Power\\PowerThrottling",
    name: "PowerThrottlingOff",
    target: crate::core::reg_tweak::RegConst::Dword(1),
    default: None,
}];

/// Option « Désactiver Power Throttling ».
pub fn throttling_tweak() -> crate::core::reg_tweak::RegistryTweak {
    crate::core::reg_tweak::RegistryTweak {
        id: "power.throttling",
        section: Section::Power,
        group: "settings",
        risk: Risk::Moderate,
        targets: THROTTLING_TARGETS,
        reboot: true,
        refresh_mouse: false,
    }
}

/// Options du profil « Performance maximale (PC fixe) ».
pub const POWER_MAX: &[&str] = &[
    "power.ultimate",
    "power.throttling",
    "power.usbSuspend",
    "power.pcieAspm",
    "power.cpuMin",
    "power.diskIdle",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ac_value_from_localized_query() {
        let sample = "    GUID du paramètre d’alimentation : ee12f906-d277-404b-b6da-e5fa1a576df5  (Gestion)\n\
            Index possible du paramètre : 000\n\
            Index actuel du paramètre de courant alternatif : 0x00000001\n\
            Index actuel du paramètre de courant continu : 0x00000002\n";
        assert_eq!(parse_ac_value(sample), Some(1));
        let range = "Valeur minimale possible : 0x00000000\nValeur maximale possible : 0x00000064\n\
            Current AC Power Setting Index: 0x00000064\nCurrent DC Power Setting Index: 0x00000005\n";
        assert_eq!(parse_ac_value(range), Some(100));
        assert_eq!(parse_ac_value("rien"), None);
    }

    #[test]
    fn reads_real_processor_minimum_state() -> AetherResult<()> {
        let scheme = active_guid()?;
        let v = query_ac_value(
            &scheme,
            "54533251-82be-4824-96c1-47b60b740d00",
            "893dee8e-2bef-41e0-89c6-b55d0929964c",
        )?;
        assert!(v.is_some_and(|v| v <= 100));
        Ok(())
    }

    #[test]
    fn parses_localized_powercfg_output() {
        let sample = "Modes de gestion de l'alimentation existants (* Actif)\n\
            -----------------------------------\n\
            GUID du mode de gestion de l'alimentation : 381b4222-f694-41f0-9685-ff5bb260df2e  (Utilisation normale) *\n\
            GUID du mode de gestion de l'alimentation : ae7be7a0-e9a4-4b02-a5df-448daa000001  (Performances optimales)\n";
        let list = parse_list(sample);
        assert_eq!(list.len(), 2);
        assert!(list[0].active);
        assert_eq!(list[0].name, "Utilisation normale");
        assert!(is_ultimate(&list[1]));
        assert!(!is_ultimate(&list[0]));
    }

    #[test]
    fn reads_active_scheme() {
        assert!(active_guid().map(|g| g.len() == 36).unwrap_or(false));
    }
}
