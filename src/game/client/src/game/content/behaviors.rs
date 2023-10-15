use bort::{event::EventGroupMarkerExtends, EventGroup, EventGroupMarkerWith, VecEventList};

use crate::game::base::behaviors::GameBaseEventGroupMarker;

use super::player::PlayerInteractEvent;

pub type GameContentEventGroup = EventGroup<dyn GameContentEventGroupMarker>;

pub trait GameContentEventGroupMarker:
	// Inherits
	GameBaseEventGroupMarker + EventGroupMarkerExtends<dyn GameBaseEventGroupMarker>
	// Events
	+ EventGroupMarkerWith<VecEventList<PlayerInteractEvent>>
{
}
