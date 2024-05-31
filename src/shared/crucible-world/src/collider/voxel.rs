use std::{iter, ops::ControlFlow};

use bevy_autoken::{random_component, Obj};
use crucible_math::{
    AaQuad, Aabb3, Axis3, BlockFace, EntityAabb, EntityVec, EntityVecExt, Line3, Sign, VecCompExt,
    WorldVecExt,
};

use newtypes::NumEnum;
use smallvec::SmallVec;
use std_traits::VecLike;
use typed_glam::{glam::DVec2, traits::NumericVector};

use crate::voxel::{BlockData, BlockMaterialCache, EntityPointer, WorldPointer, WorldVoxelData};

use super::{Collider, ColliderMaterial};

// === Block Colliders === //

#[derive(Debug, Clone)]
pub struct BlockColliderDescriptor(pub Collider);

random_component!(BlockColliderDescriptor);

pub fn occluding_volumes_in_block<B>(
    world: Obj<WorldVoxelData>,
    collider_mats: &mut BlockMaterialCache<BlockColliderDescriptor>,
    block: &mut WorldPointer,
    mut f: impl FnMut((EntityAabb, ColliderMaterial)) -> ControlFlow<B>,
) -> ControlFlow<B> {
    // Decode descriptor
    let Some(state) = block.block_data(world).filter(BlockData::is_not_air) else {
        return ControlFlow::Continue(());
    };

    let Some(descriptor) = collider_mats.get(state.material) else {
        return ControlFlow::Continue(());
    };

    // Determine collision volumes
    match &descriptor.0 {
        Collider::Transparent => {}
        Collider::Opaque(meta) => {
            f((
                Aabb3 {
                    origin: block.pos.negative_most_corner(),
                    size: EntityVec::ONE,
                },
                *meta,
            ))?;
        }
        Collider::Mesh { volumes, .. } => {
            for (aabb, meta) in volumes.iter_cloned() {
                f((
                    Aabb3 {
                        origin: aabb.origin.cast::<EntityVec>() + block.pos.negative_most_corner(),
                        size: aabb.size.cast(),
                    },
                    meta,
                ))?;
            }
        }
    }

    ControlFlow::Continue(())
}

pub fn occluding_faces_in_block<B>(
    world: Obj<WorldVoxelData>,
    collider_mats: &mut BlockMaterialCache<BlockColliderDescriptor>,
    block: &mut WorldPointer,
    face: BlockFace,
    mut f: impl FnMut((AaQuad<EntityVec>, ColliderMaterial)) -> ControlFlow<B>,
) -> ControlFlow<B> {
    // Decode descriptor
    let Some(state) = block.block_data(world).filter(BlockData::is_not_air) else {
        return ControlFlow::Continue(());
    };

    let Some(descriptor) = collider_mats.get(state.material) else {
        return ControlFlow::Continue(());
    };

    // Determine an offset from block-relative coordinates to world coordinates
    let quad_offset = block.pos.negative_most_corner();

    // Determine collision volumes
    match &descriptor.0 {
        Collider::Transparent => {}
        Collider::Opaque(meta) => {
            f((
                Aabb3 {
                    origin: quad_offset,
                    size: EntityVec::ONE,
                }
                .quad(face),
                *meta,
            ))?;
        }
        Collider::Mesh {
            volumes,
            extra_quads,
        } => {
            // Yield faces from volumes
            for (aabb, meta) in volumes.iter_cloned() {
                f((
                    Aabb3 {
                        origin: aabb.origin.cast::<EntityVec>() + quad_offset,
                        size: aabb.size.cast(),
                    }
                    .quad(face),
                    meta,
                ))?;
            }

            // Yield additional faces
            for (quad, material) in extra_quads
                .iter_cloned()
                .filter(|(quad, _)| quad.face == face)
            {
                let AaQuad {
                    origin,
                    size: (sx, sy),
                    ..
                } = quad;

                f((
                    AaQuad {
                        origin: origin.cast::<EntityVec>() + quad_offset,
                        face,
                        size: (sx.into(), sy.into()),
                    },
                    material,
                ))?;
            }
        }
    }

    ControlFlow::Continue(())
}

#[derive(Debug, Copy, Clone)]
pub struct IntersectingFaceInBlock {
    pub pos: EntityVec,
    pub face: BlockFace,
    pub dist_along_ray: f64,
    pub meta: ColliderMaterial,
}

pub fn intersecting_faces_in_block<B>(
    world: Obj<WorldVoxelData>,
    collider_mats: &mut BlockMaterialCache<BlockColliderDescriptor>,
    block: &mut WorldPointer,
    line: Line3,
    mut f: impl FnMut(IntersectingFaceInBlock) -> ControlFlow<B>,
) -> ControlFlow<B> {
    let delta = line.signed_delta();

    for axis in Axis3::variants() {
        // Determine the block face normal the line is passing through.
        let face = {
            let Some(sign) = Sign::of(delta.comp(axis)) else {
                continue;
            };

            // N.B. we invert this because, if the ray is traveling down, e.g., the +Z axis, only
            // quads facing towards the -Z axis can intercept it.
            BlockFace::compose(axis, sign).invert()
        };

        // For every occluding quad of the block pointing towards our ray...
        cbit::cbit! {
            for (quad, meta) in occluding_faces_in_block(
                world,
                collider_mats,
                block,
                face,
            ) {
                // Search for an intersection
                let Some((lerp, pos)) = quad.intersection(line) else {
                    continue;
                };

                // Yield the intersection
                f(IntersectingFaceInBlock {
                    pos,
                    face,
                    dist_along_ray: lerp * delta.length(),
                    meta,
                })?;
            }
        }
    }

    ControlFlow::Continue(())
}

// === Volumetric Collisions === //

pub const COLLISION_TOLERANCE: f64 = 0.0005;

pub fn cast_voxel_volume(
    world: Obj<WorldVoxelData>,
    collider_mats: &mut BlockMaterialCache<BlockColliderDescriptor>,
    quad: AaQuad<EntityVec>,
    delta: f64,
    tolerance: f64,
    mut filter: impl FnMut(ColliderMaterial) -> bool,
) -> f64 {
    // N.B. to ensure that `tolerance` is respected, we have to increase our check volume by
    // `tolerance` so that we catch blocks that are outside the check volume but may nonetheless
    // wish to enforce a tolerance margin of their own.
    //
    // We do this to prevent the following scenario:
    //
    // ```
    // 0   1   2
    // *---*---*
    // | %--->*|
    // *---*---*
    //       |--   Let's say this is the required tolerance...
    //   |----|    ...and this is the actual movement delta.
    //
    // Because our movement delta never hits the block face at `x = 2`, it never requires the face to
    // contribute to tolerance checking, allowing us to bypass its tolerance and eventually "tunnel"
    // through the occluder.
    // ```
    //
    // If these additional blocks don't contribute to collision detection with their tolerance, we'll
    // just ignore them.
    let check_aabb = quad.extrude_hv(delta + tolerance).as_blocks();
    let mut cached_loc = WorldPointer::new(check_aabb.origin);

    // Find the maximum allowed delta
    let mut max_allowed_delta = f64::INFINITY;

    // For every block in the volume of potential occluders...
    for pos in check_aabb.iter_blocks() {
        // For every occluder produced by that block...
        cbit::cbit! {
            for (occluder_quad, occluder_meta) in occluding_faces_in_block(
                world,
                collider_mats,
                cached_loc.move_to(pos),
                quad.face.invert(),
            ) {
                // Filter occluders by whether we are affected by them.
                if !filter(occluder_meta) {
                    continue;
                }

                if !occluder_quad
                    .as_rect::<DVec2>()
                    .intersects(quad.as_rect::<DVec2>())
                {
                    continue;
                }

                // Find its depth along the axis of movement
                let my_depth = occluder_quad.origin.comp(quad.face.axis());

                // And compare that to the starting depth to find the maximum allowable distance.
                // FIXME: We may be comparing against faces behind us!
                let rel_depth = (my_depth - quad.origin.comp(quad.face.axis())).abs();

                // Now, provide `tolerance`.
                // This step also ensures that, if the block plus its tolerance is outside of our delta,
                // it will have essentially zero effect on collision detection.
                let rel_depth = rel_depth - tolerance;

                // If `rel_depth` is negative, we were not within tolerance to begin with. We trace
                // this scenario for debug purposes.

                // It has to be a somewhat big bypass to be reported. Floating point errors are
                // expected—after all, that's why we have a tolerance in the first place.
                if rel_depth < -tolerance / 2.0 {
                    tracing::trace!(
                        "cast_volume(quad: {quad:?}, delta: {delta}, tolerance: {tolerance}) could \
                        not keep collider within tolerance. Bypass depth: {}",
                        -rel_depth,
                    );
                }

                // We still clamp this to `[0..)`.
                max_allowed_delta = max_allowed_delta.min(rel_depth.max(0.0));
            }
        }
    }

    max_allowed_delta.min(delta)
}

// === VoxelRayCast === //

#[derive(Debug, Clone)]
pub struct VoxelRayCast {
    loc: EntityPointer,
    dir: EntityVec,
    dist: f64,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VoxelRayCastIntersection {
    pub block: WorldPointer,
    pub face: BlockFace,
    pub pos: EntityVec,
    pub distance: f64,
}

impl VoxelRayCast {
    pub fn new_at(loc: EntityPointer, dir: EntityVec) -> Self {
        let (dir, dist) = {
            let dir_len_recip = dir.length_recip();

            if dir_len_recip.is_finite() && dir_len_recip > 0.0 {
                (dir * dir_len_recip, 0.)
            } else {
                (EntityVec::ZERO, f64::INFINITY)
            }
        };

        Self { loc, dir, dist }
    }

    pub fn loc(&mut self) -> &mut EntityPointer {
        &mut self.loc
    }

    pub fn pos(&self) -> EntityVec {
        self.loc.pos
    }

    pub fn dir(&self) -> EntityVec {
        self.dir
    }

    pub fn dist(&self) -> f64 {
        self.dist
    }

    pub fn step_line(&self) -> Line3 {
        Line3::new_origin_delta(self.pos(), self.dir)
    }

    pub fn step(&mut self) -> SmallVec<[VoxelRayCastIntersection; 3]> {
        // Construct a buffer to hold our `RayCastIntersection`s. There should be at most three of
        // these and, therefore, we should never allocate anything on the heap.
        let mut intersections = SmallVec::<[VoxelRayCastIntersection; 3]>::new();

        // We begin by storing a pointer to our starting block position. This will be useful later
        // when we try to determine the position of the blocks being intersected.
        let mut block_loc = self.loc.block_pointer();

        // Collect intersections
        {
            // Construct a line from our active `loc` to its end after we performed a step.
            let step_line = self.step_line();

            // Bump our location by this delta.
            self.loc.move_by(self.dir);

            // Determine the delta in blocks that we committed by taking this step. This is not
            // necessarily from one neighbor to the other in the case where we cast along the
            // diagonal of a block. However, we do know that we will step along each axis at most
            // once. This is why it's important that our step delta remains less than or equal to `1`:
            // in the worst case scenario where our step delta is axis-aligned, a delta greater than
            // `1` could skip entirely past a block!
            let start_block = step_line.start.block_pos();
            let end_block = step_line.end.block_pos();
            let block_delta = end_block - start_block;

            // For every axis...
            for axis in Axis3::variants() {
                // Determine the face through which we traveled.
                let delta = block_delta.comp(axis);
                debug_assert!((-1..=1).contains(&delta));

                let Some(sign) = Sign::of(delta) else {
                    continue;
                };

                let exit_face = BlockFace::compose(axis, sign);

                // Determine the plane of intersection.
                // N.B., because this step can only go through each axis at most once, taking the
                // plane w.r.t the `start_block` is identical to taking the plane w.r.t the actual
                // block from which the ray is traveling.
                let isect_plane = start_block.face_plane(exit_face);

                // Now, we just have to determine the point of intersection and commit it to the
                // buffer.
                let (isect_lerp, isect_pos) = isect_plane.intersection(step_line);

                intersections.push(VoxelRayCastIntersection {
                    block: block_loc, // This will be updated in a bit.
                    face: exit_face.invert(),
                    // N.B. This lerp value is the actual length along the ray because the ray is a
                    // unit vector.
                    distance: self.dist + isect_lerp,
                    pos: isect_pos,
                });
            }
        }

        // We then sort our intersection list by distance traveled to both satisfy the contract of
        // this method and to ensure that we can accurately determine block positions.
        intersections.sort_by(|a, b| a.distance.total_cmp(&b.distance));

        // Update block positions by accumulating face traversals onto `block_loc`—which is currently
        // just the position of our ray before the step began.
        for isect in &mut intersections {
            isect.block = block_loc.neighbor(isect.face.invert());
            block_loc = isect.block;
        }

        // Update the distance accumulator.
        // N.B. the direction is either normalized, in which case the step was of length 1, or we're
        // traveling with direction zero, in which case the distance traveled is already infinite.
        self.dist += 1.0;

        // Don't forget to bump the position!
        self.loc.move_by(self.dir);

        intersections
    }

    pub fn step_intersect(
        &mut self,
        world: Obj<WorldVoxelData>,
        collider_mats: &mut BlockMaterialCache<BlockColliderDescriptor>,
        isect_buffer: &mut impl VecLike<Elem = (VoxelRayCastIntersection, ColliderMaterial)>,
    ) {
        // Step the ray forward, keeping track of its old state.
        let start_dist = self.dist();
        let line = self.step_line();
        let block_isects = self.step();

        // Clear scratch buffer
        isect_buffer.clear();

        // Collect intersections
        for block_isect in block_isects {
            cbit::cbit! {
                for face_isect in intersecting_faces_in_block(
                    world,
                    collider_mats,
                    &mut block_isect.block.clone(),
                    line,
                ) {
                    // N.B. we don't limit the maximum distance because a ray can only be within a
                    // given block once.
                    if face_isect.dist_along_ray <= 0.0 {
                        continue;
                    }

                    isect_buffer.push((
                        VoxelRayCastIntersection {
                            block: block_isect.block,
                            face: face_isect.face,
                            pos: face_isect.pos,
                            distance: start_dist + face_isect.dist_along_ray,
                        },
                        face_isect.meta,
                    ));
                }
            }
        }

        // Sort intersections
        isect_buffer.sort_by(|a, b| a.0.distance.total_cmp(&b.0.distance));
    }

    pub fn step_forever(&mut self) -> impl Iterator<Item = VoxelRayCastIntersection> + '_ {
        iter::from_fn(|| Some(self.step())).flatten()
    }

    pub fn step_for(
        &mut self,
        max_dist: f64,
    ) -> impl Iterator<Item = VoxelRayCastIntersection> + '_ {
        self.step_forever()
            .take_while(move |isect| isect.distance <= max_dist)
    }

    pub fn step_intersect_forever<'a>(
        &'a mut self,
        world: Obj<WorldVoxelData>,
        collider_mats: &'a mut BlockMaterialCache<BlockColliderDescriptor>,
    ) -> impl Iterator<Item = (VoxelRayCastIntersection, ColliderMaterial)> + 'a {
        let mut backlog = smallvec::SmallVec::<[_; 4]>::new();

        iter::from_fn(move || {
            while backlog.is_empty() {
                self.step_intersect(world, collider_mats, &mut backlog);
            }

            backlog.reverse();

            Some(backlog.pop().unwrap())
        })
    }

    pub fn step_intersect_for<'a>(
        &'a mut self,
        world: Obj<WorldVoxelData>,
        collider_mats: &'a mut BlockMaterialCache<BlockColliderDescriptor>,
        max_dist: f64,
    ) -> impl Iterator<Item = (VoxelRayCastIntersection, ColliderMaterial)> + 'a {
        self.step_intersect_forever(world, collider_mats)
            .take_while(move |(isect, _)| isect.distance <= max_dist)
    }
}

// === Rigid Body === //

pub fn move_rigid_body_voxels(
    world: Obj<WorldVoxelData>,
    collider_mats: &mut BlockMaterialCache<BlockColliderDescriptor>,
    mut aabb: EntityAabb,
    delta: EntityVec,
    mut filter: impl FnMut(ColliderMaterial) -> bool,
) -> EntityVec {
    for axis in Axis3::variants() {
        // Decompose the movement part
        let signed_delta = delta.comp(axis);
        let unsigned_delta = signed_delta.abs();
        let sign = Sign::of(signed_delta).unwrap_or(Sign::Positive);
        let face = BlockFace::compose(axis, sign);

        // Determine how far we can move
        let actual_delta = cast_voxel_volume(
            world,
            collider_mats,
            aabb.quad(face),
            unsigned_delta,
            COLLISION_TOLERANCE,
            &mut filter,
        );

        // Commit the movement
        aabb.origin += face.unit_typed::<EntityVec>() * actual_delta;
    }

    aabb.origin
}
