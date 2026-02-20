use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player's permission is checked.
///
/// Plugins can modify the `result` field to override permission checks.
#[derive(Event, Clone)]
pub struct PlayerPermissionCheckEvent {
    /// The player whose permission is being checked.
    pub player: Arc<Player>,
    /// The permission node being checked.
    pub permission: String,
    /// The result of the permission check. Plugins can override this.
    pub result: bool,
}

impl PlayerPermissionCheckEvent {
    pub const fn new(player: Arc<Player>, permission: String, result: bool) -> Self {
        Self {
            player,
            permission,
            result,
        }
    }
}

impl PlayerEvent for PlayerPermissionCheckEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
