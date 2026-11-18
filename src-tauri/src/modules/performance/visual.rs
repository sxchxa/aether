//! Effets visuels de Windows (fenêtre « Options de performances » de Windows), pilotés par l'API
//! officielle `SystemParametersInfoW` : appliqués à chaud, persistés dans le profil utilisateur,
//! sans redémarrage. Le lissage des polices (ClearType) n'est volontairement jamais modifié.

use std::ffi::c_void;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, ANIMATIONINFO, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETANIMATION,
    SPI_GETCLIENTAREAANIMATION, SPI_GETCOMBOBOXANIMATION, SPI_GETCURSORSHADOW, SPI_GETDRAGFULLWINDOWS,
    SPI_GETDROPSHADOW, SPI_GETMENUANIMATION, SPI_GETSELECTIONFADE, SPI_GETTOOLTIPANIMATION, SPI_SETANIMATION,
    SPI_SETCLIENTAREAANIMATION, SPI_SETCOMBOBOXANIMATION, SPI_SETCURSORSHADOW, SPI_SETDRAGFULLWINDOWS,
    SPI_SETDROPSHADOW, SPI_SETMENUANIMATION, SPI_SETSELECTIONFADE, SPI_SETTOOLTIPANIMATION,
};

use crate::core::history::UndoAction;
use crate::core::state::{AppState, MISC_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::{AetherError, AetherResult};

/// Façon dont Windows attend la valeur d'un paramètre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Booléen passé dans `pvParam` (lecture via un pointeur `BOOL`).
    Flag,
    /// Booléen passé dans `uiParam` (glisser le contenu des fenêtres).
    UiParam,
    /// Structure `ANIMATIONINFO` (animation de réduction / agrandissement).
    Animation,
}

/// Effet visuel désactivable.
#[derive(Debug, Clone, Copy)]
pub struct VisualEffect {
    /// Identifiant de l'option.
    pub id: &'static str,
    get: u32,
    set: u32,
    kind: Kind,
}

/// Effets proposés (ceux de « Options de performances », hors lissage des polices et
/// défilement doux des listes, conservés pour le confort visuel).
pub const EFFECTS: &[VisualEffect] = &[
    VisualEffect {
        id: "fx.minMax",
        get: SPI_GETANIMATION,
        set: SPI_SETANIMATION,
        kind: Kind::Animation,
    },
    VisualEffect {
        id: "fx.controls",
        get: SPI_GETCLIENTAREAANIMATION,
        set: SPI_SETCLIENTAREAANIMATION,
        kind: Kind::Flag,
    },
    VisualEffect {
        id: "fx.menus",
        get: SPI_GETMENUANIMATION,
        set: SPI_SETMENUANIMATION,
        kind: Kind::Flag,
    },
    VisualEffect {
        id: "fx.tooltips",
        get: SPI_GETTOOLTIPANIMATION,
        set: SPI_SETTOOLTIPANIMATION,
        kind: Kind::Flag,
    },
    VisualEffect {
        id: "fx.combo",
        get: SPI_GETCOMBOBOXANIMATION,
        set: SPI_SETCOMBOBOXANIMATION,
        kind: Kind::Flag,
    },
    VisualEffect {
        id: "fx.selectionFade",
        get: SPI_GETSELECTIONFADE,
        set: SPI_SETSELECTIONFADE,
        kind: Kind::Flag,
    },
    VisualEffect {
        id: "fx.cursorShadow",
        get: SPI_GETCURSORSHADOW,
        set: SPI_SETCURSORSHADOW,
        kind: Kind::Flag,
    },
    VisualEffect {
        id: "fx.windowShadow",
        get: SPI_GETDROPSHADOW,
        set: SPI_SETDROPSHADOW,
        kind: Kind::Flag,
    },
    VisualEffect {
        id: "fx.dragContents",
        get: SPI_GETDRAGFULLWINDOWS,
        set: SPI_SETDRAGFULLWINDOWS,
        kind: Kind::UiParam,
    },
];

fn last_error() -> AetherError {
    // SAFETY : lecture du code d'erreur du thread courant.
    AetherError::Win32 {
        api: "SystemParametersInfoW",
        code: unsafe { windows_sys::Win32::Foundation::GetLastError() },
    }
}

fn find(id: &str) -> AetherResult<&'static VisualEffect> {
    EFFECTS
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AetherError::InvalidInput(format!("effet visuel inconnu : {id}")))
}

/// Lit l'état d'un effet visuel.
///
/// Impact Windows : lecture seule (`SystemParametersInfoW`, action `SPI_GET…`).
pub fn get(effect: &VisualEffect) -> AetherResult<bool> {
    // SAFETY : les tampons passés ont la taille et le type attendus par chaque action SPI_GET.
    let ok = unsafe {
        match effect.kind {
            Kind::Animation => {
                let mut info = ANIMATIONINFO {
                    cbSize: size_of::<ANIMATIONINFO>() as u32,
                    iMinAnimate: 0,
                };
                let r = SystemParametersInfoW(
                    effect.get,
                    info.cbSize,
                    (&mut info as *mut ANIMATIONINFO).cast::<c_void>(),
                    0,
                );
                (r != 0).then_some(info.iMinAnimate != 0)
            }
            Kind::Flag | Kind::UiParam => {
                let mut value: i32 = 0;
                let r = SystemParametersInfoW(effect.get, 0, (&mut value as *mut i32).cast::<c_void>(), 0);
                (r != 0).then_some(value != 0)
            }
        }
    };
    ok.ok_or_else(last_error)
}

/// Active ou désactive un effet visuel, immédiatement et de façon persistante.
///
/// Impact Windows : `SystemParametersInfoW` (`SPI_SET…`) avec `SPIF_UPDATEINIFILE | SPIF_SENDCHANGE`
/// (enregistré dans le profil de l'utilisateur et notifié aux applications ouvertes).
pub fn set(effect: &VisualEffect, enabled: bool) -> AetherResult<()> {
    let flags = SPIF_UPDATEINIFILE | SPIF_SENDCHANGE;
    // SAFETY : chaque action reçoit les paramètres décrits par la documentation Microsoft.
    let r = unsafe {
        match effect.kind {
            Kind::Animation => {
                let mut info = ANIMATIONINFO {
                    cbSize: size_of::<ANIMATIONINFO>() as u32,
                    iMinAnimate: i32::from(enabled),
                };
                SystemParametersInfoW(
                    effect.set,
                    info.cbSize,
                    (&mut info as *mut ANIMATIONINFO).cast::<c_void>(),
                    flags,
                )
            }
            Kind::UiParam => SystemParametersInfoW(effect.set, u32::from(enabled), std::ptr::null_mut(), flags),
            // Pour ces actions, la valeur booléenne est transmise directement dans `pvParam`.
            Kind::Flag => SystemParametersInfoW(effect.set, 0, usize::from(enabled) as *mut c_void, flags),
        }
    };
    if r == 0 {
        return Err(last_error());
    }
    Ok(())
}

/// Rejoue une action d'annulation sur un effet visuel.
pub fn set_by_id(id: &str, enabled: bool) -> AetherResult<()> {
    set(find(id)?, enabled)
}

fn label(on: bool) -> String {
    if on {
        "activé".into()
    } else {
        "désactivé".into()
    }
}

/// Option « effet visuel désactivé » (état d'origine sauvegardé à la première écriture).
pub struct VisualEffectTweak(pub &'static VisualEffect);

impl VisualEffectTweak {
    fn backup_key(&self) -> String {
        format!("spi.{}", self.0.id)
    }
}

impl Tweak for VisualEffectTweak {
    fn id(&self) -> &'static str {
        self.0.id
    }
    fn section(&self) -> Section {
        Section::Performance
    }
    fn group(&self) -> &'static str {
        "fx"
    }
    fn risk(&self) -> Risk {
        Risk::Safe
    }
    fn technical(&self) -> String {
        format!("SystemParametersInfoW(0x{:04X}, désactivé)", self.0.set)
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let on = get(self.0)?;
        Ok(Probe {
            applied: !on,
            available: true,
            current: label(on),
            target: label(false),
        })
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let on = get(self.0)?;
        if !on {
            return Ok(Vec::new());
        }
        state.backups(MISC_BACKUP).record_first(&self.backup_key(), &on)?;
        set(self.0, false)?;
        Ok(vec![UndoAction::VisualEffect {
            id: self.0.id.to_owned(),
            enabled: on,
        }])
    }

    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let backups = state.backups(MISC_BACKUP);
        // Sans sauvegarde, la valeur par défaut de Windows (effet activé) est rétablie.
        let original = backups.get::<bool>(&self.backup_key())?.unwrap_or(true);
        let current = get(self.0)?;
        backups.remove(&self.backup_key())?;
        if current == original {
            return Ok(Vec::new());
        }
        set(self.0, original)?;
        Ok(vec![UndoAction::VisualEffect {
            id: self.0.id.to_owned(),
            enabled: current,
        }])
    }
}

/// Options « effets visuels » du catalogue.
pub fn tweaks() -> impl Iterator<Item = VisualEffectTweak> {
    EFFECTS.iter().map(VisualEffectTweak)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_are_unique_and_readable() -> AetherResult<()> {
        let mut ids: Vec<_> = EFFECTS.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), EFFECTS.len());
        // Lecture seule : ne modifie rien sur la machine de test.
        for e in EFFECTS {
            get(e)?;
        }
        Ok(())
    }
}
