use bort::{Entity, EventTarget, HasGlobalManagedTag};

#[non_exhaustive]
pub struct HealthUpdated;

#[derive(Debug, Clone)]
pub struct HealthState {
	health: f32,
	max_health: f32,
}

impl HasGlobalManagedTag for HealthState {
	type Component = Self;
}

impl HealthState {
	pub fn new(health: f32, max_health: f32) -> Self {
		let max_health = max_health.max(0.0);
		Self {
			health: health.clamp(0.0, max_health),
			max_health,
		}
	}

	pub fn health(&self) -> f32 {
		self.health
	}

	pub fn health_percent(&self) -> f32 {
		self.health / self.max_health
	}

	pub fn max_health(&self) -> f32 {
		self.max_health
	}

	#[clippy::dangerous(direct_health_setting, reason = "send an event instead")]
	pub fn set_health(
		&mut self,
		me: Entity,
		on_health_change: &mut impl EventTarget<HealthUpdated>,
		health: f32,
	) {
		let old_health = self.health;
		self.health = health.clamp(0.0, self.max_health);

		if old_health != health {
			on_health_change.fire(me, HealthUpdated);
		}
	}

	#[clippy::dangerous(direct_health_setting, reason = "send an event instead")]
	pub fn set_max_health(
		&mut self,
		me: Entity,
		on_health_change: &mut impl EventTarget<HealthUpdated>,
		max_health: f32,
	) {
		self.max_health = max_health.max(0.0);
		self.set_health(
			me,
			on_health_change,
			self.health.clamp(0.0, self.max_health),
		);
	}
}
