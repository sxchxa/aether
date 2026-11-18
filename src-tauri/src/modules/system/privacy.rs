//! Confidentialité et activité en arrière-plan (stratégies officielles « Privacy » du Policy CSP).

use crate::core::reg_tweak::{RegConst, RegTarget, RegistryTweak};
use crate::core::registry::Hive;
use crate::core::tweak::{Risk, Section};

/// Stratégie « Désactiver l'identifiant de publicité » (Privacy/DisableAdvertisingId).
pub static ADVERTISING_TARGETS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Policies\\Microsoft\\Windows\\AdvertisingInfo",
    name: "DisabledByGroupPolicy",
    target: RegConst::Dword(1),
    default: None,
}];

/// Stratégie « Laisser les applications Windows s'exécuter en arrière-plan » = Refuser (2).
pub static BACKGROUND_TARGETS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Policies\\Microsoft\\Windows\\AppPrivacy",
    name: "LetAppsRunInBackground",
    target: RegConst::Dword(2),
    default: None,
}];

/// Option « Désactiver l'identifiant de publicité ».
pub fn advertising_tweak() -> RegistryTweak {
    RegistryTweak {
        id: "privacy.advertisingId",
        section: Section::System,
        group: "privacy",
        risk: Risk::Safe,
        targets: ADVERTISING_TARGETS,
        reboot: false,
        refresh_mouse: false,
    }
}

/// Option « Bloquer les applications en arrière-plan ».
pub fn background_apps_tweak() -> RegistryTweak {
    RegistryTweak {
        id: "privacy.backgroundApps",
        section: Section::System,
        group: "privacy",
        risk: Risk::Advanced,
        targets: BACKGROUND_TARGETS,
        reboot: false,
        refresh_mouse: false,
    }
}
