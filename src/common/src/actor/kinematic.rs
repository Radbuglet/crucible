use std::f64::consts::E;

use crate::math::EntityVec;

// === Parameters === //

/// A conversion coefficient from `(Minecraft tick)^-1` to `second^-1` (`20 ticks` = `1 sec`).
pub const MC_TICKS_TO_SECS: f64 = 20.0;

/// A conversion coefficient from `(Minecraft tick)^-2` to `second^-2` (`20 ticks` = `1 sec`).
pub const MC_TICKS_TO_SECS_SQUARED: f64 = MC_TICKS_TO_SECS * MC_TICKS_TO_SECS;

// === Kinematic core === //

// All equations used in this section can be experimented with in the following Desmos document:
// https://www.desmos.com/calculator/lfueduuihe

/// The minimum friction coefficient required to use the more accurate friction-affected kinematic
/// model.
const LOW_FRICTION_CUTOFF: f64 = 0.001;

/// Converts a traditional friction coefficient between `0` and `1` that would be applied `tps` times
/// per second into a percent of velocity subtracted per second (measured in `second^-1`), as is expected
/// by [`update_velocity_axis`] and company.
pub fn tick_friction_coef_to_coef_qty_axis(coef: f64, tps: f64) -> f64 {
	// Imagine we started with the simple loop...
	//
	// ```rust
	// for _ in 0..tps {
	//     velocity *= coef;
	// }
	// ```
	//
	// ...and would like to achieve a similar result given the new differential computation:
	//
	// - `V(t) = V(t-dt) - V(t - dt) * new_coef * dt`
	// - `V'(t) = V(t) * (-new_coef)`
	//
	// We know that the effect of the first loop is the multiply `velocity` by `coef^tps` so, in order
	// to solve for what our `new_coef` should be to achieve the same result, we have to solve for
	// `V(1)` and compare that to our intended solution.
	//
	// Solving the differential equation, we find that:
	//
	// - `V(t) = velocity * e ^(-new_coef * time)`
	//
	// Now, to make these two forms equivalent at `t = 1`...
	//
	// - `velocity * e ^ (-new_coef) = velocity * coef^tps`
	// - `e ^ (-new_coef) = coef^tps`
	// - `-new_coef = ln(coef^tps)`
	// - `new_coef = -ln(coef^tps)`
	//
	// Huzzah!

	-(coef.powf(tps)).ln()
}

/// A three-dimensional version of [`tick_friction_coef_to_coef_qty_axis`].
pub fn tick_friction_coef_to_coef_qty(coef: EntityVec, tps: f64) -> EntityVec {
	EntityVec::new(
		tick_friction_coef_to_coef_qty_axis(coef.x(), tps),
		tick_friction_coef_to_coef_qty_axis(coef.y(), tps),
		tick_friction_coef_to_coef_qty_axis(coef.z(), tps),
	)
}

/// Applies environmental acceleration (measured in `blocks * second^-2`) and friction (measured in
/// `second^-1` and representing the percent of the velocity we subtract from the velocity per second)
/// to the entity's velocity (measured in `blocks * second^-1`) over the specified time step
/// (measured in `seconds`).
///
/// `time` can be made arbitrarily large (modulo floating-point precision concerns) without causing
/// update discrepancies, although this may break instantaneous logic external to this function.
///
/// Environmental acceleration refers to acceleration that is applied to the object for the entire
/// specified duration, e.g. gravity or some other field attractor. Jumps should not be passed as
/// environmental acceleration, but rather as a one-time impulse force onto the velocity, such that
/// they get applied once rather than integrated over time. This matters, for example, when `time`
/// is less than `1 / EXPECTED_TICK_RATE`, which will cause only a portion of the intended
/// acceleration to be applied to the object if the jump impulse only exists during one tick.
#[must_use]
pub fn update_velocity_axis(
	velocity: f64,
	acceleration: f64,
	friction_coef: f64,
	time: f64,
) -> f64 {
	// Figuring out how to make this routine behave the same for arbitrarily small `time` deltas is
	// non trivial.
	//
	// Here is fixed-tick routine...
	//
	// ```rust
	// velocity += acceleration;
	// velocity *= (1 - friction_coef);
	// ```
	//
	// ...expressed as a recursive function where `dt` is a small delta in time, where all values are
	// assumed to be scalars:
	//
	// - `V(0) = velocity`
	// - `V(t) = V(t-dt) + acceleration * dt - V(t - dt) * friction_coef * dt`
	//
	// Rearranging, we can re-express this formula as a first-order linear differential equation:
	//
	// - `V(t) = V(t-dt) + (acceleration - V(t-dt) * friction_coef) * dt`
	// - `V'(t) = acceleration - V(t) * friction_coef` where `V(0) = velocity`.
	//
	// Solving through the method of integrating factor, we find that:
	//
	// - `V(t) = (acceleration - acceleration * e^(-friction_coef * time)) / (friction_coef)
	//         + velocity * e^(-friction_coef * time)`
	//
	// For the case where friction is close to zero, we simply use the regular integral
	//
	// - `V(t) = velocity + acceleration * time`
	//
	if friction_coef < LOW_FRICTION_CUTOFF {
		velocity + acceleration * time
	} else {
		let e_term = E.powf(-friction_coef * time);

		(acceleration - acceleration * e_term) / friction_coef + velocity * e_term
	}
}

/// Integrates the velocity kinematics described by [`update_velocity_axis`] to return a position
/// delta over the `time` specified. All the same kinematic considerations as documented in
/// [`update_velocity_axis`] apply here as well.
pub fn update_position_delta_axis(
	velocity: f64,
	acceleration: f64,
	friction_coef: f64,
	time: f64,
) -> f64 {
	if friction_coef < LOW_FRICTION_CUTOFF {
		// If friction is negligible, we use the classic kinematic equation for position.
		velocity * time + (acceleration * time * time) / 2.0
	} else {
		// Otherwise, we ...we take the integral of our raw `update_velocity_axis` equation, like so:
		//
		// - `V(t) = (acceleration - acceleration * e^(-friction * time)) / (friction) + velocity * e^(-friction * time)`
		// - `P(t) = position + int(low=0, high=t, integrand=V)
		//
		// ...giving us the equation:
		let e_term = E.powf(-friction_coef * time) - 1.0;
		(acceleration * (e_term / friction_coef + time) - velocity * e_term) / friction_coef
	}
}

/// A three-dimensional version of [`update_velocity_axis`].
#[must_use]
pub fn update_velocity(
	velocity: EntityVec,
	acceleration: EntityVec,
	friction_coef: EntityVec,
	time: f64,
) -> EntityVec {
	EntityVec::new(
		update_velocity_axis(velocity.x(), acceleration.x(), friction_coef.x(), time),
		update_velocity_axis(velocity.y(), acceleration.y(), friction_coef.y(), time),
		update_velocity_axis(velocity.z(), acceleration.z(), friction_coef.z(), time),
	)
}

/// A three-dimensional version of [`update_position_delta_axis`].
#[must_use]
pub fn update_position_delta(
	velocity: EntityVec,
	acceleration: EntityVec,
	friction_coef: EntityVec,
	time: f64,
) -> EntityVec {
	EntityVec::new(
		update_position_delta_axis(velocity.x(), acceleration.x(), friction_coef.x(), time),
		update_position_delta_axis(velocity.y(), acceleration.y(), friction_coef.y(), time),
		update_position_delta_axis(velocity.z(), acceleration.z(), friction_coef.z(), time),
	)
}

/// A function combining [`update_velocity_axis`] and [`update_position_delta_axis`].
#[must_use]
pub fn update_kinematic(
	velocity: EntityVec,
	acceleration: EntityVec,
	friction_coef: EntityVec,
	time: f64,
) -> (EntityVec, EntityVec) {
	(
		update_position_delta(velocity, acceleration, friction_coef, time),
		update_velocity(velocity, acceleration, friction_coef, time),
	)
}
