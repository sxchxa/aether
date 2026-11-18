//! Optimisations gaming : accélération souris, Xbox Game DVR, fréquence d'écran.

use std::ffi::c_void;

use serde::Serialize;
use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETMOUSE};

use crate::core::reg_tweak::{RegConst, RegTarget, RegistryTweak};
use crate::core::registry::{self, Hive, RegData};
use crate::core::tweak::{Risk, Section};
use crate::error::{AetherError, AetherResult};

const MOUSE_KEY: &str = "Control Panel\\Mouse";

/// Valeurs ciblées pour désactiver l'accélération souris (« Améliorer la précision du pointeur »).
pub static MOUSE_TARGETS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: MOUSE_KEY,
        name: "MouseSpeed",
        target: RegConst::Sz("0"),
        default: Some(RegConst::Sz("1")),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: MOUSE_KEY,
        name: "MouseThreshold1",
        target: RegConst::Sz("0"),
        default: Some(RegConst::Sz("6")),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: MOUSE_KEY,
        name: "MouseThreshold2",
        target: RegConst::Sz("0"),
        default: Some(RegConst::Sz("10")),
    },
];

/// Valeurs ciblées pour désactiver l'enregistrement en arrière-plan Xbox Game DVR.
pub static XBOX_DVR_TARGETS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: "System\\GameConfigStore",
        name: "GameDVR_Enabled",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(1)),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: "Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR",
        name: "AppCaptureEnabled",
        target: RegConst::Dword(0),
        default: None,
    },
];

/// Option « Désactiver l'accélération souris ».
pub fn mouse_tweak() -> RegistryTweak {
    RegistryTweak {
        id: "gaming.mouse",
        section: Section::Performance,
        group: "gaming",
        risk: Risk::Safe,
        targets: MOUSE_TARGETS,
        reboot: false,
        refresh_mouse: true,
    }
}

/// Option « Désactiver Xbox Game DVR ».
pub fn xbox_dvr_tweak() -> RegistryTweak {
    RegistryTweak {
        id: "gaming.xboxDvr",
        section: Section::Performance,
        group: "gaming",
        risk: Risk::Safe,
        targets: XBOX_DVR_TARGETS,
        reboot: false,
        refresh_mouse: false,
    }
}

fn read_mouse_value(name: &str, fallback: i32) -> AetherResult<i32> {
    Ok(match registry::read(Hive::Hkcu, MOUSE_KEY, name)? {
        Some(RegData::Sz(s)) => s.trim().parse().unwrap_or(fallback),
        Some(RegData::Dword(v)) => i32::try_from(v).unwrap_or(fallback),
        _ => fallback,
    })
}

/// Réapplique immédiatement les paramètres souris du Registre (sans fermer la session).
///
/// Impact Windows : `SystemParametersInfoW(SPI_SETMOUSE)` avec diffusion `WM_SETTINGCHANGE`.
pub fn refresh_mouse() -> AetherResult<()> {
    let mut values: [i32; 3] = [
        read_mouse_value("MouseThreshold1", 6)?,
        read_mouse_value("MouseThreshold2", 10)?,
        read_mouse_value("MouseSpeed", 1)?,
    ];
    // SAFETY : `values` est un tableau de 3 entiers valide pendant tout l'appel.
    let ok = unsafe { SystemParametersInfoW(SPI_SETMOUSE, 0, values.as_mut_ptr().cast::<c_void>(), SPIF_SENDCHANGE) };
    if ok == 0 {
        return Err(AetherError::Win32 {
            api: "SystemParametersInfoW",
            code: std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32,
        });
    }
    Ok(())
}

/// Informations sur la fréquence de rafraîchissement de l'écran principal.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    /// Largeur en pixels.
    pub width: u32,
    /// Hauteur en pixels.
    pub height: u32,
    /// Fréquence actuelle (Hz).
    pub current_hz: u32,
    /// Fréquence maximale disponible à cette résolution (Hz).
    pub max_hz: u32,
    /// Un taux supérieur est disponible.
    pub upgrade_available: bool,
}

fn blank_devmode() -> DEVMODEW {
    // SAFETY : DEVMODEW est une structure C « plain old data » ; zéro est une valeur valide.
    let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
    dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    dm
}

/// Détecte la fréquence actuelle et maximale de l'écran principal.
///
/// Impact Windows : lecture seule (`EnumDisplaySettingsW`).
pub fn display_info() -> AetherResult<DisplayInfo> {
    let mut current = blank_devmode();
    // SAFETY : pointeur valide vers une DEVMODEW initialisée ; périphérique NULL = écran principal.
    let ok = unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut current) };
    if ok == 0 {
        return Err(AetherError::Win32 {
            api: "EnumDisplaySettingsW",
            code: 0,
        });
    }
    let mut max_hz = current.dmDisplayFrequency;
    let mut index = 0u32;
    loop {
        let mut mode = blank_devmode();
        // SAFETY : idem ci-dessus ; l'énumération s'arrête au premier index invalide.
        let more = unsafe { EnumDisplaySettingsW(std::ptr::null(), index, &mut mode) };
        if more == 0 {
            break;
        }
        if mode.dmPelsWidth == current.dmPelsWidth
            && mode.dmPelsHeight == current.dmPelsHeight
            && mode.dmDisplayFrequency > max_hz
        {
            max_hz = mode.dmDisplayFrequency;
        }
        index += 1;
    }
    Ok(DisplayInfo {
        width: current.dmPelsWidth,
        height: current.dmPelsHeight,
        current_hz: current.dmDisplayFrequency,
        max_hz,
        upgrade_available: max_hz > current.dmDisplayFrequency,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_targets_disable_acceleration() {
        assert!(MOUSE_TARGETS.iter().all(|t| t.target == RegConst::Sz("0")));
    }

    #[test]
    fn display_info_is_consistent() {
        if let Ok(info) = display_info() {
            assert!(info.max_hz >= info.current_hz);
        }
    }
}
