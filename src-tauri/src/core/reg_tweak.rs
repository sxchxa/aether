//! Option générique basée sur des valeurs de Registre (souris, Xbox DVR, TCP, Edge…).

use crate::core::history::UndoAction;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, REGISTRY_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::AetherResult;

/// Valeur de Registre constante (définissable statiquement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegConst {
    /// `REG_DWORD`
    Dword(u32),
    /// `REG_SZ`
    Sz(&'static str),
}

impl RegConst {
    /// Conversion vers une donnée de Registre.
    pub fn to_data(self) -> RegData {
        match self {
            RegConst::Dword(v) => RegData::Dword(v),
            RegConst::Sz(s) => RegData::Sz(s.to_owned()),
        }
    }
}

/// Valeur cible d'une option de Registre.
#[derive(Debug, Clone, Copy)]
pub struct RegTarget {
    /// Ruche.
    pub hive: Hive,
    /// Chemin de la clé.
    pub path: &'static str,
    /// Nom de la valeur.
    pub name: &'static str,
    /// Valeur optimisée.
    pub target: RegConst,
    /// Valeur par défaut de Windows (`None` = valeur absente) utilisée sans sauvegarde.
    pub default: Option<RegConst>,
}

impl RegTarget {
    fn backup_key(&self) -> String {
        format!("{}\\{}\\{}", self.hive.label(), self.path, self.name)
    }
}

/// Option composée d'une ou plusieurs valeurs de Registre.
pub struct RegistryTweak {
    /// Identifiant.
    pub id: &'static str,
    /// Section.
    pub section: Section,
    /// Groupe.
    pub group: &'static str,
    /// Risque.
    pub risk: Risk,
    /// Valeurs ciblées.
    pub targets: &'static [RegTarget],
    /// Redémarrage conseillé.
    pub reboot: bool,
    /// Réappliquer à chaud les paramètres souris après écriture.
    pub refresh_mouse: bool,
}

fn show(v: &Option<RegData>) -> String {
    v.as_ref().map(RegData::display).unwrap_or_else(|| "absente".into())
}

impl RegistryTweak {
    fn post_write(&self, actions: &mut Vec<UndoAction>) -> AetherResult<()> {
        if self.refresh_mouse {
            crate::modules::performance::gaming::refresh_mouse()?;
            // Rejouée en dernier lors d'une annulation (ordre inverse).
            actions.insert(0, UndoAction::RefreshMouse);
        }
        // Couleurs Windows (mode sombre, transparence) : les applications ouvertes et la barre des
        // tâches sont prévenues, comme le fait l’application Paramètres (aucun redémarrage).
        if self.targets.iter().any(|t| t.path.ends_with("Themes\\Personalize")) {
            notify_theme_change();
        }
        Ok(())
    }
}

/// Diffuse `WM_SETTINGCHANGE` (« ImmersiveColorSet ») pour appliquer immédiatement un changement de thème.
///
/// Impact Windows : message de notification aux fenêtres (délai maximal de 2 s par fenêtre, ignore les
/// fenêtres qui ne répondent pas). Aucune modification.
pub fn notify_theme_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    // Diffusion en arrière-plan : une fenêtre lente à répondre ne doit jamais faire attendre le bouton.
    std::thread::spawn(|| {
        let area: Vec<u16> = "ImmersiveColorSet".encode_utf16().chain(std::iter::once(0)).collect();
        let mut result = 0usize;
        // SAFETY : chaîne UTF-16 terminée par zéro, valide pendant tout l’appel ; résultat dans une variable locale.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                area.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                2000,
                &mut result,
            );
        }
    });
}

impl Tweak for RegistryTweak {
    fn id(&self) -> &'static str {
        self.id
    }
    fn section(&self) -> Section {
        self.section
    }
    fn group(&self) -> &'static str {
        self.group
    }
    fn risk(&self) -> Risk {
        self.risk
    }
    fn needs_reboot(&self) -> bool {
        self.reboot
    }
    fn technical(&self) -> String {
        self.targets
            .iter()
            .map(|t| format!("{} = {}", t.backup_key(), t.target.to_data().display()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let mut applied = true;
        let mut current = Vec::new();
        for t in self.targets {
            let value = registry::read(t.hive, t.path, t.name)?;
            if value.as_ref() != Some(&t.target.to_data()) {
                applied = false;
            }
            current.push(format!("{} = {}", t.name, show(&value)));
        }
        Ok(Probe {
            applied,
            available: true,
            current: current.join(", "),
            target: self
                .targets
                .iter()
                .map(|t| format!("{} = {}", t.name, t.target.to_data().display()))
                .collect::<Vec<_>>()
                .join(", "),
        })
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let backups = state.backups(REGISTRY_BACKUP);
        let mut actions = Vec::new();
        let mut write_all = || -> AetherResult<()> {
            for t in self.targets {
                let current = registry::read(t.hive, t.path, t.name)?;
                let target = t.target.to_data();
                // L'état d'origine est mémorisé même pour une valeur déjà optimale : la restauration
                // la laisse alors telle quelle au lieu de la remplacer par le défaut de Windows.
                backups.record_first(&t.backup_key(), &current)?;
                if current.as_ref() == Some(&target) {
                    continue; // Valeur déjà optimale : on n'y touche pas.
                }
                registry::write(t.hive, t.path, t.name, &target)?;
                actions.push(UndoAction::Registry {
                    hive: t.hive,
                    path: t.path.to_owned(),
                    name: t.name.to_owned(),
                    previous: current,
                });
            }
            Ok(())
        };
        if let Err(e) = write_all() {
            // Échec en cours de route (lecture ou écriture) : les valeurs déjà écrites sont remises
            // en état, l'option n'est jamais laissée à moitié appliquée.
            for done in actions.iter().rev() {
                let _ = crate::core::undo::execute(done);
            }
            return Err(e);
        }
        self.post_write(&mut actions)?;
        Ok(actions)
    }
    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let backups = state.backups(REGISTRY_BACKUP);
        let mut actions = Vec::new();
        for t in self.targets {
            let key = t.backup_key();
            let original: Option<RegData> = match backups.get::<Option<RegData>>(&key)? {
                Some(saved) => saved,
                None => t.default.map(RegConst::to_data),
            };
            let current = registry::read(t.hive, t.path, t.name)?;
            if current == original {
                backups.remove(&key)?;
                continue;
            }
            match &original {
                Some(data) => registry::write(t.hive, t.path, t.name, data)?,
                None => registry::delete_value(t.hive, t.path, t.name)?,
            }
            actions.push(UndoAction::Registry {
                hive: t.hive,
                path: t.path.to_owned(),
                name: t.name.to_owned(),
                previous: current,
            });
            backups.remove(&key)?;
        }
        self.post_write(&mut actions)?;
        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tweak::{optimize, restore, Outcome};

    static TARGETS: &[RegTarget] = &[
        RegTarget {
            hive: Hive::Hkcu,
            path: "Software\\AetherTests\\RegTweak",
            name: "Throttling",
            target: RegConst::Dword(0xffff_ffff),
            default: Some(RegConst::Dword(10)),
        },
        RegTarget {
            hive: Hive::Hkcu,
            path: "Software\\AetherTests\\RegTweak",
            name: "Speed",
            target: RegConst::Sz("0"),
            default: None,
        },
    ];

    #[test]
    fn registry_tweak_backs_up_once_and_restores_original() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        state.set_dry_run(false)?;
        registry::delete_tree(Hive::Hkcu, "Software\\AetherTests\\RegTweak")?;
        registry::write(Hive::Hkcu, TARGETS[0].path, "Throttling", &RegData::Dword(42))?;

        let tweak = RegistryTweak {
            id: "test.reg",
            section: Section::Network,
            group: "test",
            risk: Risk::Safe,
            targets: TARGETS,
            reboot: false,
            refresh_mouse: false,
        };
        assert_eq!(optimize(&tweak, &state)?, Outcome::Applied);
        assert_eq!(optimize(&tweak, &state)?, Outcome::AlreadyDone);
        assert!(tweak.check_status(&state)?.applied);

        // La valeur d'origine (42) est restaurée, pas la valeur par défaut (10).
        assert_eq!(restore(&tweak, &state)?, Outcome::Applied);
        assert_eq!(
            registry::read(Hive::Hkcu, TARGETS[0].path, "Throttling")?,
            Some(RegData::Dword(42))
        );
        assert_eq!(registry::read(Hive::Hkcu, TARGETS[0].path, "Speed")?, None);
        registry::delete_tree(Hive::Hkcu, "Software\\AetherTests\\RegTweak")?;
        Ok(())
    }

    /// Une option à deux valeurs dont la seconde échoue : la première est remise en état.
    #[test]
    fn partial_failure_is_rolled_back() -> AetherResult<()> {
        static TARGETS: &[RegTarget] = &[
            RegTarget { hive: Hive::Hkcu, path: "Software\\AetherTests\\Partial", name: "First", target: RegConst::Dword(1), default: None },
            // Nom de clé de plus de 255 caractères : refusé par le Registre.
            RegTarget {
                hive: Hive::Hkcu,
                path: "Software\\AetherTests\\Partial\\xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                name: "Second",
                target: RegConst::Dword(1),
                default: None,
            },
        ];
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        let tweak = RegistryTweak {
            id: "test.partial",
            section: Section::System,
            group: "test",
            risk: Risk::Safe,
            targets: TARGETS,
            reboot: false,
            refresh_mouse: false,
        };
        let res = tweak.apply(&state);
        let first = registry::read(Hive::Hkcu, "Software\\AetherTests\\Partial", "First")?;
        registry::delete_tree(Hive::Hkcu, "Software\\AetherTests\\Partial")?;
        assert!(res.is_err());
        assert_eq!(first, None, "la première valeur aurait dû être retirée");
        Ok(())
    }
}
