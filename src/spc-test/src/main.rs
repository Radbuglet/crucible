use space_time::zorder::{z_3::Z3, z_n::ZN};
use typed_glam::glam::{Mat4, UVec3, Vec3};

fn main() {
	const EDGE: u64 = 40;
	const DIM: u64 = EDGE * EDGE * EDGE;

	let mut cuts_accum = 0u64;
	let mut vis_accum = 0u64;
	let mut times = 0;

	loop {
		// Generate random state
		let origin = Vec3::new(
			fastrand::f32() * EDGE as f32,
			fastrand::f32() * EDGE as f32,
			fastrand::f32() * EDGE as f32,
		);

		let center = Vec3::new(EDGE as f32 / 2., EDGE as f32 / 2., EDGE as f32 / 2.);

		let proj = Mat4::perspective_lh(90f32.to_radians(), 1., 0.1, 1000.)
			* Mat4::look_at_lh(origin, center, Vec3::Y);

		// Count cuts
		let mut was_in_curve = None;
		let mut cuts = 0;
		let mut vis = 0;

		for i in 0..DIM {
			let point =
				UVec3::new(Z3::combine(i), Z3::combine(i >> 1), Z3::combine(i >> 2)).as_vec3();

			let transformed = proj.project_point3(point);

			let in_curve = (-1.0..1.0).contains(&transformed.x)
				&& (-1.0..1.0).contains(&transformed.y)
				&& (0.0..1.0).contains(&transformed.z);

			if in_curve {
				vis += 1;
			}

			if was_in_curve.map_or(true, |was_in_curve| was_in_curve != in_curve) {
				cuts += 1;
				was_in_curve = Some(in_curve);
			}
		}

		// Accumulate average
		cuts_accum += cuts;
		vis_accum += vis;
		times += 1;

		// Occasionally display results
		if times % 1000 == 0 && times != 0 {
			println!(
				"Average cuts: {}. Average visible: {}. Sample size: {}",
				cuts_accum as f64 / times as f64,
				vis_accum as f64 / times as f64,
				times
			);
		}
	}
}
