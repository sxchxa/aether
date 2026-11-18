//! Réglages populaires des guides d'optimisation 2026, retenus uniquement lorsqu'ils sont
//! documentés par Microsoft ou correspondent exactement à un interrupteur des Paramètres Windows.

use crate::core::history::UndoAction;
use crate::core::reg_tweak::{RegConst, RegTarget, RegistryTweak};
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, REGISTRY_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::AetherResult;

/// Mode Jeu (Paramètres > Jeux > Mode Jeu) — activé explicitement.
pub static GAME_MODE: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\GameBar",
    name: "AutoGameModeEnabled",
    target: RegConst::Dword(1),
    default: None,
}];

/// Raccourci clavier des touches rémanentes (5 × Maj) désactivé — évite la pop-up en jeu.
pub static STICKY_KEYS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Control Panel\\Accessibility\\StickyKeys",
    name: "Flags",
    target: RegConst::Sz("506"),
    default: Some(RegConst::Sz("510")),
}];

/// Mode sombre de Windows et des applications (Paramètres > Personnalisation > Couleurs).
pub static DARK_MODE: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        name: "AppsUseLightTheme",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(1)),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        name: "SystemUsesLightTheme",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(1)),
    },
];

/// Priorité des jeux (planificateur multimédia MMCSS, tâche « Games » documentée par Microsoft).
pub static MMCSS_GAMES: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games",
        name: "GPU Priority",
        target: RegConst::Dword(8),
        default: Some(RegConst::Dword(8)),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games",
        name: "Priority",
        target: RegConst::Dword(6),
        default: Some(RegConst::Dword(2)),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games",
        name: "Scheduling Category",
        target: RegConst::Sz("High"),
        default: Some(RegConst::Sz("Medium")),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games",
        name: "SFIO Priority",
        target: RegConst::Sz("High"),
        default: Some(RegConst::Sz("Normal")),
    },
];

/// Diffusion de jeux intégrée à Windows désactivée (stratégie GameDVR).
pub static GAME_BROADCAST: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: r"SOFTWARE\Policies\Microsoft\Windows\GameDVR",
    name: "AllowBroadcast",
    target: RegConst::Dword(0),
    default: None,
}];

/// Le bouton Xbox de la manette n'ouvre plus la Game Bar.
pub static GAMEBAR_BUTTON: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: r"SOFTWARE\Microsoft\GameBar",
    name: "UseNexusForGameBarEnabled",
    target: RegConst::Dword(0),
    default: None,
}];

/// Optimisations plein écran désactivées pour tous les jeux (comportement plein écran exclusif).
pub static FSO_OFF: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: r"System\GameConfigStore",
        name: "GameDVR_FSEBehaviorMode",
        target: RegConst::Dword(2),
        default: None,
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: r"System\GameConfigStore",
        name: "GameDVR_HonorUserFSEBehaviorMode",
        target: RegConst::Dword(1),
        default: None,
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: r"System\GameConfigStore",
        name: "GameDVR_DXGIHonorFSEWindowsCompatible",
        target: RegConst::Dword(1),
        default: None,
    },
];

/// Effets de transparence (Paramètres > Personnalisation > Couleurs).
pub static TRANSPARENCY: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
    name: "EnableTransparency",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

const CDM: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager";

/// Conseils, suggestions et contenus promotionnels de Windows (interrupteurs des Paramètres).
pub static TIPS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: CDM,
        name: "SubscribedContent-338389Enabled",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(1)),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: CDM,
        name: "SubscribedContent-338388Enabled",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(1)),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: CDM,
        name: "SubscribedContent-310093Enabled",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(1)),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: CDM,
        name: "SystemPaneSuggestionsEnabled",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(1)),
    },
];

/// Widgets (Windows 11, NewsAndInterests CSP) et Actualités et centres d'intérêt (Windows 10, Feeds CSP).
pub static WIDGETS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: "SOFTWARE\\Policies\\Microsoft\\Dsh",
        name: "AllowNewsAndInterests",
        target: RegConst::Dword(0),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: "SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Feeds",
        name: "EnableFeeds",
        target: RegConst::Dword(0),
        default: None,
    },
];

/// Recall : ne pas enregistrer de captures (WindowsAI CSP, DisableAIDataAnalysis).
pub static RECALL: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Policies\\Microsoft\\Windows\\WindowsAI",
    name: "DisableAIDataAnalysis",
    target: RegConst::Dword(1),
    default: None,
}];

const ADV: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced";

/// Délai d'ouverture des menus (Panneau de configuration\Bureau, MenuShowDelay : 400 ms par défaut).
pub static MENU_DELAY: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Control Panel\\Desktop",
    name: "MenuShowDelay",
    target: RegConst::Sz("100"),
    default: Some(RegConst::Sz("400")),
}];

/// « Fin de tâche » dans le clic droit de la barre des tâches (Paramètres > Système > Pour les développeurs).
pub static END_TASK: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced\\TaskbarDeveloperSettings",
    name: "TaskbarEndTask",
    target: RegConst::Dword(1),
    default: None,
}];

/// Extensions des fichiers visibles (option de l'Explorateur, recommandée pour repérer les faux documents).
pub static FILE_EXTENSIONS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "HideFileExt",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

/// Planification GPU à accélération matérielle (Paramètres > Affichage > Graphiques, HwSchMode = 2).
pub static HAGS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers",
    name: "HwSchMode",
    target: RegConst::Dword(2),
    default: None,
}];

/// Résultats web Bing dans la recherche du menu Démarrer (stratégie DisableSearchBoxSuggestions).
pub static BING_SEARCH: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Policies\\Microsoft\\Windows\\Explorer",
    name: "DisableSearchBoxSuggestions",
    target: RegConst::Dword(1),
    default: None,
}];

/// Copilot désactivé (stratégie WindowsAI/TurnOffWindowsCopilot).
pub static COPILOT: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Policies\\Microsoft\\Windows\\WindowsCopilot",
    name: "TurnOffWindowsCopilot",
    target: RegConst::Dword(1),
    default: None,
}];

const SYS_POLICY: &str = "SOFTWARE\\Policies\\Microsoft\\Windows\\System";

/// Historique des activités ni publié ni envoyé (stratégies PublishUserActivities / UploadUserActivities).
pub static ACTIVITY: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: SYS_POLICY,
        name: "PublishUserActivities",
        target: RegConst::Dword(0),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: SYS_POLICY,
        name: "UploadUserActivities",
        target: RegConst::Dword(0),
        default: None,
    },
];

/// Données de diagnostic limitées au strict nécessaire (stratégie AllowTelemetry = 1 « Requises »).
pub static TELEMETRY: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection",
    name: "AllowTelemetry",
    target: RegConst::Dword(1),
    default: None,
}];

/// Recommandations du menu Démarrer (Paramètres > Personnalisation > Démarrer, Windows 11).
pub static START_RECOMMENDATIONS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "Start_IrisRecommendations",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

/// Icônes au lieu des miniatures dans l'Explorateur (option « Toujours afficher les icônes »).
pub static THUMBNAILS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "IconsOnly",
    target: RegConst::Dword(1),
    default: Some(RegConst::Dword(0)),
}];

/// Animations de la barre des tâches (Options de performances).
pub static TASKBAR_ANIM: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "TaskbarAnimations",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

/// Aperçu du bureau « Peek » (Options de performances).
pub static PEEK: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Windows\\DWM",
    name: "EnablePeek",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

/// Ombre sous les noms des icônes du Bureau (Options de performances).
pub static ICON_SHADOW: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "ListviewShadow",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

/// Rectangle de sélection translucide (Options de performances).
pub static SELECTION_RECT: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "ListviewAlphaSelect",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

/// Délai de lancement des applications au démarrage de session supprimé (Explorer\Serialize).
pub static STARTUP_DELAY: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Serialize",
    name: "StartupDelayInMSec",
    target: RegConst::Dword(0),
    default: None,
}];

/// Fréquence des demandes de commentaires : « Jamais » (Paramètres > Confidentialité > Diagnostics).
pub static FEEDBACK: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Siuf\\Rules",
    name: "NumberOfSIUFInPeriod",
    target: RegConst::Dword(0),
    default: None,
}];

/// Assistant de stockage activé (Paramètres > Système > Stockage) : nettoyage automatique des fichiers temporaires.
pub static STORAGE_SENSE: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Windows\\CurrentVersion\\StorageSense\\Parameters\\StoragePolicy",
    name: "01",
    target: RegConst::Dword(1),
    default: None,
}];

/// Horodatage du dernier accès NTFS désactivé (`fsutil behavior set disablelastaccess 1`).
pub static LAST_ACCESS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SYSTEM\\CurrentControlSet\\Control\\FileSystem",
    name: "NtfsDisableLastAccessUpdate",
    target: RegConst::Dword(0x8000_0001),
    default: Some(RegConst::Dword(0x8000_0002)),
}];

/// L'Explorateur s'ouvre sur « Ce PC ».
pub static LAUNCH_THIS_PC: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "LaunchTo",
    target: RegConst::Dword(1),
    default: None,
}];

/// Fichiers et dossiers cachés visibles (option de l'Explorateur).
pub static HIDDEN_FILES: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "Hidden",
    target: RegConst::Dword(1),
    default: Some(RegConst::Dword(2)),
}];

/// « Secouer » une fenêtre ne réduit plus les autres (Paramètres > Système > Multitâche).
pub static AERO_SHAKE: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "DisallowShaking",
    target: RegConst::Dword(1),
    default: None,
}];

/// Messages détaillés au démarrage et à l'arrêt (stratégie VerboseStatus).
pub static VERBOSE_STATUS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
    name: "VerboseStatus",
    target: RegConst::Dword(1),
    default: None,
}];

/// Personnalisation de l'écriture manuscrite et de la saisie (Paramètres > Confidentialité).
pub static INKING: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: "Software\\Microsoft\\InputPersonalization",
        name: "RestrictImplicitTextCollection",
        target: RegConst::Dword(1),
        default: None,
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: "Software\\Microsoft\\InputPersonalization",
        name: "RestrictImplicitInkCollection",
        target: RegConst::Dword(1),
        default: None,
    },
];

/// Expériences personnalisées à partir des données de diagnostic (Paramètres > Confidentialité).
pub static TAILORED: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Windows\\CurrentVersion\\Privacy",
    name: "TailoredExperiencesWithDiagnosticDataEnabled",
    target: RegConst::Dword(0),
    default: None,
}];

/// Suivi des lancements d'applications pour Démarrer et la recherche (Paramètres > Confidentialité > Général).
pub static APP_TRACKING: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: ADV,
    name: "Start_TrackProgs",
    target: RegConst::Dword(0),
    default: Some(RegConst::Dword(1)),
}];

/// Reconnaissance vocale en ligne (Paramètres > Confidentialité > Voix).
pub static ONLINE_SPEECH: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: "Software\\Microsoft\\Speech_OneCore\\Settings\\OnlineSpeechPrivacy",
    name: "HasAccepted",
    target: RegConst::Dword(0),
    default: None,
}];
const WU: &str = "SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate";

/// Pilotes exclus des mises à jour Windows (stratégie ExcludeWUDriversInQualityUpdate).
pub static WU_NO_DRIVERS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: WU,
    name: "ExcludeWUDriversInQualityUpdate",
    target: RegConst::Dword(1),
    default: None,
}];

/// Mises à jour de fonctionnalités reportées d'un an (Windows Update for Business).
pub static WU_DEFER_FEATURE: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: WU,
        name: "DeferFeatureUpdates",
        target: RegConst::Dword(1),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: WU,
        name: "DeferFeatureUpdatesPeriodInDays",
        target: RegConst::Dword(365),
        default: None,
    },
];

/// Heures d'activité de 8 h à 2 h : aucun redémarrage automatique pendant cette plage.
pub static WU_ACTIVE_HOURS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: WU,
        name: "SetActiveHours",
        target: RegConst::Dword(1),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: WU,
        name: "ActiveHoursStart",
        target: RegConst::Dword(8),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: WU,
        name: "ActiveHoursEnd",
        target: RegConst::Dword(2),
        default: None,
    },
];

/// Notification avant chaque redémarrage requis (Paramètres > Windows Update > Options avancées).
pub static WU_RESTART_NOTIFY: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Microsoft\\WindowsUpdate\\UX\\Settings",
    name: "RestartNotificationsAllowed2",
    target: RegConst::Dword(1),
    default: None,
}];
fn reg(
    id: &'static str,
    section: Section,
    group: &'static str,
    risk: Risk,
    targets: &'static [RegTarget],
) -> RegistryTweak {
    RegistryTweak {
        id,
        section,
        group,
        risk,
        targets,
        reboot: false,
        refresh_mouse: false,
    }
}

/// Options ajoutées au catalogue.
pub fn registry_extras() -> Vec<RegistryTweak> {
    vec![
        reg("gaming.gameMode", Section::Performance, "gaming", Risk::Safe, GAME_MODE),
        reg(
            "gaming.stickyKeys",
            Section::Performance,
            "gaming",
            Risk::Safe,
            STICKY_KEYS,
        ),
        reg(
            "ui.transparency",
            Section::Performance,
            "visual",
            Risk::Safe,
            TRANSPARENCY,
        ),
        reg("ui.darkMode", Section::Performance, "visual", Risk::Safe, DARK_MODE),
        reg(
            "gaming.mmcss",
            Section::Performance,
            "gaming",
            Risk::Moderate,
            MMCSS_GAMES,
        ),
        reg(
            "gaming.broadcast",
            Section::Performance,
            "gaming",
            Risk::Safe,
            GAME_BROADCAST,
        ),
        reg(
            "gaming.gameBarButton",
            Section::Performance,
            "gaming",
            Risk::Safe,
            GAMEBAR_BUTTON,
        ),
        reg("gaming.fsoOff", Section::Performance, "gaming", Risk::Advanced, FSO_OFF),
        reg("privacy.tips", Section::System, "privacy", Risk::Safe, TIPS),
        reg("privacy.widgets", Section::System, "privacy", Risk::Safe, WIDGETS),
        reg("privacy.recall", Section::System, "privacy", Risk::Safe, RECALL),
        reg("ui.menuDelay", Section::Performance, "visual", Risk::Safe, MENU_DELAY),
        reg("ui.endTask", Section::Performance, "visual", Risk::Safe, END_TASK),
        reg(
            "ui.fileExtensions",
            Section::Performance,
            "visual",
            Risk::Safe,
            FILE_EXTENSIONS,
        ),
        RegistryTweak {
            reboot: true,
            ..reg("gaming.hags", Section::Performance, "gaming", Risk::Moderate, HAGS)
        },
        reg(
            "privacy.bingSearch",
            Section::System,
            "privacy",
            Risk::Safe,
            BING_SEARCH,
        ),
        reg("privacy.copilot", Section::System, "privacy", Risk::Safe, COPILOT),
        reg("privacy.activity", Section::System, "privacy", Risk::Safe, ACTIVITY),
        reg("privacy.telemetry", Section::System, "privacy", Risk::Safe, TELEMETRY),
        reg(
            "privacy.startRecommendations",
            Section::System,
            "privacy",
            Risk::Safe,
            START_RECOMMENDATIONS,
        ),
        reg("fx.thumbnails", Section::Performance, "fx", Risk::Safe, THUMBNAILS),
        reg("fx.taskbarAnim", Section::Performance, "fx", Risk::Safe, TASKBAR_ANIM),
        reg("fx.peek", Section::Performance, "fx", Risk::Safe, PEEK),
        reg("fx.iconShadow", Section::Performance, "fx", Risk::Safe, ICON_SHADOW),
        reg(
            "fx.selectionRect",
            Section::Performance,
            "fx",
            Risk::Safe,
            SELECTION_RECT,
        ),
        reg(
            "ui.startupDelay",
            Section::Performance,
            "visual",
            Risk::Safe,
            STARTUP_DELAY,
        ),
        reg("privacy.feedback", Section::System, "privacy", Risk::Safe, FEEDBACK),
        reg("fs.storageSense", Section::Performance, "fs", Risk::Safe, STORAGE_SENSE),
        RegistryTweak {
            reboot: true,
            ..reg("fs.lastAccess", Section::Performance, "fs", Risk::Safe, LAST_ACCESS)
        },
        reg(
            "ui.launchThisPc",
            Section::Performance,
            "visual",
            Risk::Safe,
            LAUNCH_THIS_PC,
        ),
        reg(
            "ui.hiddenFiles",
            Section::Performance,
            "visual",
            Risk::Safe,
            HIDDEN_FILES,
        ),
        reg("ui.aeroShake", Section::Performance, "visual", Risk::Safe, AERO_SHAKE),
        reg(
            "ui.verboseStatus",
            Section::Performance,
            "visual",
            Risk::Safe,
            VERBOSE_STATUS,
        ),
        reg("privacy.inking", Section::System, "privacy", Risk::Safe, INKING),
        reg("privacy.tailored", Section::System, "privacy", Risk::Safe, TAILORED),
        reg(
            "privacy.appTracking",
            Section::System,
            "privacy",
            Risk::Safe,
            APP_TRACKING,
        ),
        reg(
            "privacy.onlineSpeech",
            Section::System,
            "privacy",
            Risk::Safe,
            ONLINE_SPEECH,
        ),
        reg(
            "wu.restartNotify",
            Section::System,
            "update",
            Risk::Safe,
            WU_RESTART_NOTIFY,
        ),
        reg("wu.activeHours", Section::System, "update", Risk::Safe, WU_ACTIVE_HOURS),
        reg("wu.noDrivers", Section::System, "update", Risk::Moderate, WU_NO_DRIVERS),
        reg(
            "wu.deferFeature",
            Section::System,
            "update",
            Risk::Moderate,
            WU_DEFER_FEATURE,
        ),
    ]
}

const DX_KEY: &str = "Software\\Microsoft\\DirectX\\UserGpuPreferences";
const DX_VALUE: &str = "DirectXUserGlobalSettings";
const SWAP_KEY: &str = "SwapEffectUpgradeEnable";

/// Lit une valeur `clé=valeur;` dans la chaîne des préférences DirectX.
pub fn dx_get(settings: &str, key: &str) -> Option<String> {
    settings
        .split(';')
        .filter_map(|p| p.split_once('='))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case(key))
        .map(|(_, v)| v.trim().to_owned())
}

/// Définit (ou retire si `None`) une valeur dans la chaîne des préférences DirectX, en préservant les autres.
pub fn dx_set(settings: &str, key: &str, value: Option<&str>) -> String {
    let mut parts: Vec<String> = settings
        .split(';')
        .filter(|p| !p.trim().is_empty())
        .filter(|p| {
            p.split_once('=')
                .map(|(k, _)| !k.trim().eq_ignore_ascii_case(key))
                .unwrap_or(true)
        })
        .map(|p| p.trim().to_owned())
        .collect();
    if let Some(v) = value {
        parts.push(format!("{key}={v}"));
    }
    parts.iter().map(|p| format!("{p};")).collect()
}

/// « Optimisations pour les jeux en fenêtre » (Paramètres > Affichage > Graphiques), documenté par Microsoft.
pub struct WindowedGamesTweak;

impl WindowedGamesTweak {
    fn current() -> AetherResult<Option<RegData>> {
        registry::read(Hive::Hkcu, DX_KEY, DX_VALUE)
    }
    fn text(data: &Option<RegData>) -> String {
        match data {
            Some(RegData::Sz(s)) => s.clone(),
            _ => String::new(),
        }
    }
}

impl Tweak for WindowedGamesTweak {
    fn id(&self) -> &'static str {
        "gaming.windowedOpt"
    }
    fn section(&self) -> Section {
        Section::Performance
    }
    fn group(&self) -> &'static str {
        "gaming"
    }
    fn risk(&self) -> Risk {
        Risk::Safe
    }
    fn technical(&self) -> String {
        format!("HKCU\\{DX_KEY}\\{DX_VALUE} ⊃ \"{SWAP_KEY}=1;\" (autres préférences conservées)")
    }
    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let text = Self::text(&Self::current()?);
        let value = dx_get(&text, SWAP_KEY);
        Ok(Probe {
            applied: value.as_deref() == Some("1"),
            available: true,
            current: value
                .map(|v| format!("{SWAP_KEY}={v}"))
                .unwrap_or_else(|| "par défaut".into()),
            target: format!("{SWAP_KEY}=1"),
        })
    }
    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let previous = Self::current()?;
        state
            .backups(REGISTRY_BACKUP)
            .record_first(&format!("HKCU\\{DX_KEY}\\{DX_VALUE}"), &previous)?;
        let updated = dx_set(&Self::text(&previous), SWAP_KEY, Some("1"));
        registry::write(Hive::Hkcu, DX_KEY, DX_VALUE, &RegData::Sz(updated))?;
        Ok(vec![UndoAction::Registry {
            hive: Hive::Hkcu,
            path: DX_KEY.to_owned(),
            name: DX_VALUE.to_owned(),
            previous,
        }])
    }
    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let current = Self::current()?;
        let backups = state.backups(REGISTRY_BACKUP);
        let key = format!("HKCU\\{DX_KEY}\\{DX_VALUE}");
        match backups.get::<Option<RegData>>(&key)? {
            Some(Some(original)) => registry::write(Hive::Hkcu, DX_KEY, DX_VALUE, &original)?,
            Some(None) => registry::delete_value(Hive::Hkcu, DX_KEY, DX_VALUE)?,
            None => {
                let updated = dx_set(&Self::text(&current), SWAP_KEY, None);
                registry::write(Hive::Hkcu, DX_KEY, DX_VALUE, &RegData::Sz(updated))?;
            }
        }
        backups.remove(&key)?;
        Ok(vec![UndoAction::Registry {
            hive: Hive::Hkcu,
            path: DX_KEY.to_owned(),
            name: DX_VALUE.to_owned(),
            previous: current,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx_settings_merge_preserves_other_preferences() {
        let s = "VRROptimizeEnable=0;AutoHDREnable=1;";
        assert_eq!(dx_get(s, "AutoHDREnable").as_deref(), Some("1"));
        let set = dx_set(s, SWAP_KEY, Some("1"));
        assert_eq!(set, "VRROptimizeEnable=0;AutoHDREnable=1;SwapEffectUpgradeEnable=1;");
        assert_eq!(dx_set(&set, SWAP_KEY, None), s);
        assert_eq!(dx_set("", SWAP_KEY, Some("1")), "SwapEffectUpgradeEnable=1;");
    }

    #[test]
    fn extras_have_unique_ids() {
        let ids: Vec<&str> = registry_extras().iter().map(|t| t.id).collect();
        let mut dedup = ids.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len());
    }
}
