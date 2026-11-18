//! Socle transverse d'Aether : erreurs, commandes silencieuses, Registre, sauvegardes,
//! historique d'annulation, journal et moteur d'optimisation idempotent.

pub mod backup;
pub mod command;
pub mod history;
pub mod logger;
pub mod reg_tweak;
pub mod registry;
pub mod state;
pub mod tweak;
pub mod undo;
