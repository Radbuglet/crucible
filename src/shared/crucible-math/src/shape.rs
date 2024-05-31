use num_traits::Signed;
use typed_glam::{
    glam,
    traits::{NumericVector3, SignedNumericVector2, SignedNumericVector3},
};

use crate::{
    lerp_percent_at, BlockFace, EntityVec, EntityVecExt, Sign, VecCompExt, VolumetricIter, WorldVec,
};

// === Line3 === //

#[derive(Debug, Copy, Clone)]
pub struct Line3 {
    pub start: EntityVec,
    pub end: EntityVec,
}

impl Line3 {
    pub fn new(start: EntityVec, end: EntityVec) -> Self {
        Self { start, end }
    }

    pub fn new_origin_delta(start: EntityVec, delta: EntityVec) -> Self {
        Self {
            start,
            end: start + delta,
        }
    }

    pub fn signed_delta(&self) -> EntityVec {
        self.end - self.start
    }

    pub fn length(&self) -> f64 {
        self.signed_delta().length()
    }

    pub fn length_squared(&self) -> f64 {
        self.signed_delta().length_squared()
    }
}

// === AaPlane === //

#[derive(Debug, Copy, Clone)]
pub struct AaPlane {
    pub origin: f64,
    pub normal: BlockFace,
}

impl AaPlane {
    pub fn intersection(self, line: Line3) -> (f64, EntityVec) {
        let lerp = lerp_percent_at(
            self.origin,
            line.start.comp(self.normal.axis()),
            line.end.comp(self.normal.axis()),
        );
        (lerp, line.start.lerp(line.end, lerp))
    }

    pub fn project_down<V, U>(self, pos: V) -> U
    where
        V: SignedNumericVector3<Comp = f64>,
        U: SignedNumericVector2<Comp = f64>,
    {
        let (h, v) = self.normal.axis().ortho_hv();
        U::new(pos.comp(h), pos.comp(v))
    }
}

// === AaQuad === //

#[derive(Debug, Copy, Clone)]
pub struct AaQuad<V: NumericVector3> {
    pub origin: V,
    pub face: BlockFace,
    pub size: (V::Comp, V::Comp),
}

impl<V: NumericVector3> AaQuad<V> {
    pub fn new_given_volume(origin: V, face: BlockFace, volume: V) -> Self {
        let (h, v) = face.axis().ortho_hv();
        let size = (volume.comp(h), volume.comp(v));

        Self { origin, face, size }
    }

    pub fn new_unit(origin: V, face: BlockFace) -> Self {
        let one = V::ONE.x();

        Self {
            origin,
            face,
            size: (one, one),
        }
    }

    pub fn translated(&self, by: V) -> Self {
        Self {
            origin: self.origin + by,
            face: self.face,
            size: self.size,
        }
    }

    pub fn size_deltas(&self) -> (V, V) {
        let (h, v) = self.face.axis().ortho_hv();
        let (sh, sv) = self.size;
        (
            h.unit_typed::<V>() * V::splat(sh),
            v.unit_typed::<V>() * V::splat(sv),
        )
    }

    pub fn as_rect<U: SignedNumericVector2<Comp = V::Comp>>(&self) -> Aabb2<U> {
        let (h, v) = self.face.axis().ortho_hv();
        let (sh, sv) = self.size;

        Aabb2 {
            origin: U::new(self.origin.comp(h), self.origin.comp(v)),
            size: U::new(sh, sv),
        }
    }

    pub fn extrude_hv(self, delta: V::Comp) -> Aabb3<V>
    where
        V: SignedNumericVector3,
        V::Comp: Signed,
    {
        Aabb3 {
            origin: if self.face.sign() == Sign::Negative {
                self.origin - self.face.axis().unit_typed::<V>() * V::splat(delta)
            } else {
                self.origin
            },
            size: self.face.axis().extrude_volume_hv(self.size, delta),
        }
    }
}

impl<V: NumericVector3<Comp = f64>> AaQuad<V> {
    pub fn as_plane(&self) -> AaPlane {
        AaPlane {
            origin: self.origin.comp(self.face.axis()),
            normal: self.face,
        }
    }

    pub fn intersection(&self, line: Line3) -> Option<(f64, EntityVec)> {
        let plane = self.as_plane();
        let (lerp, pos) = plane.intersection(line);

        if self
            .as_rect::<glam::DVec2>()
            .contains(plane.project_down(pos))
        {
            Some((lerp, pos))
        } else {
            None
        }
    }
}

// === Aabb2 === //

#[derive(Debug, Copy, Clone)]
pub struct Aabb2<V> {
    pub origin: V,
    pub size: V,
}

impl<V: SignedNumericVector2> Aabb2<V> {
    pub fn new(x: V::Comp, y: V::Comp, w: V::Comp, h: V::Comp) -> Self {
        Self {
            origin: V::new(x, y),
            size: V::new(w, h),
        }
    }

    pub fn from_origin_size(pos: V, size: V, percent: V) -> Self {
        Self {
            origin: pos - size * percent,
            size,
        }
    }

    pub fn at_percent(&self, percent: V) -> V {
        self.origin + self.size * percent
    }

    pub fn contains(&self, point: V) -> bool
    where
        V::Comp: PartialOrd,
    {
        (self.origin.x() <= point.x() && point.x() < self.origin.x() + self.size.x())
            && (self.origin.y() <= point.y() && point.y() < self.origin.y() + self.size.y())
    }

    pub fn intersects(&self, other: Self) -> bool
    where
        V::Comp: PartialOrd,
    {
        Aabb2 {
            origin: self.origin - other.size,
            size: self.size + other.size,
        }
        .contains(other.origin)
    }
}

// === Aabb3 === //

pub type EntityAabb = Aabb3<EntityVec>;
pub type WorldAabb = Aabb3<WorldVec>;

#[derive(Debug, Copy, Clone)]
pub struct Aabb3<V> {
    pub origin: V,
    pub size: V,
}

impl<V: SignedNumericVector3> Aabb3<V> {
    pub const ZERO: Self = Self {
        origin: V::ZERO,
        size: V::ZERO,
    };

    pub const ONE: Self = Self {
        origin: V::ZERO,
        size: V::ONE,
    };

    #[must_use]
    pub fn from_corners_max_excl(a: V, b: V) -> Self {
        let min = a.min(b);
        let max = a.max(b);

        Self {
            origin: min,
            size: max - min,
        }
    }

    #[must_use]
    pub fn from_origin_size(origin: V, size: V, percent: V) -> Self {
        Aabb3 { origin, size }.centered_at(percent)
    }

    #[must_use]
    pub fn with_origin(&self, new_origin: V) -> Self {
        Aabb3 {
            origin: new_origin,
            size: self.size,
        }
    }

    #[must_use]
    pub fn translated(&self, by: V) -> Self {
        Self {
            origin: self.origin + by,
            size: self.size,
        }
    }

    #[must_use]
    pub fn centered_at(&self, percent: V) -> Self {
        self.translated(-self.size * percent)
    }

    #[must_use]
    pub fn at_percent(&self, percent: V) -> V {
        self.origin + self.size * percent
    }

    #[must_use]
    pub fn max_corner(&self) -> V {
        self.origin + self.size
    }

    #[must_use]
    pub fn grow(self, by: V) -> Self {
        Self {
            origin: self.origin - by,
            size: self.size + by + by,
        }
    }

    #[must_use]
    pub fn quad(self, face: BlockFace) -> AaQuad<V> {
        let origin = self.origin;
        let origin = if face.sign() == Sign::Positive {
            origin + face.unit_typed::<V>() * V::splat(self.size.comp(face.axis()))
        } else {
            origin
        };

        AaQuad::new_given_volume(origin, face, self.size)
    }

    #[must_use]
    pub fn contains(&self, point: V) -> bool
    where
        V::Comp: PartialOrd,
    {
        (self.origin.x() <= point.x() && point.x() < self.origin.x() + self.size.x())
            && (self.origin.y() <= point.y() && point.y() < self.origin.y() + self.size.y())
            && (self.origin.z() <= point.z() && point.z() < self.origin.z() + self.size.z())
    }

    #[must_use]
    pub fn intersects(&self, other: Self) -> bool
    where
        V::Comp: PartialOrd,
    {
        Self {
            origin: self.origin - other.size,
            size: self.size + other.size,
        }
        .contains(other.origin)
    }

    #[must_use]
    pub fn offset_by(&self, delta: V) -> Self {
        Self {
            origin: self.origin + delta,
            size: self.size,
        }
    }
}

impl EntityAabb {
    pub fn as_blocks(&self) -> WorldAabb {
        Aabb3::from_blocks_corners(self.origin.block_pos(), self.max_corner().block_pos())
    }
}

impl WorldAabb {
    pub fn from_blocks_corners(a: WorldVec, b: WorldVec) -> Self {
        let origin = a.min(b);
        let size = (b - a).abs() + WorldVec::ONE;

        Self { origin, size }
    }

    pub fn iter_blocks(self) -> impl Iterator<Item = WorldVec> {
        VolumetricIter::new_exclusive_iter([
            self.size.x() as u32,
            self.size.y() as u32,
            self.size.z() as u32,
        ])
        .map(move |[x, y, z]| self.origin + WorldVec::new(x as i32, y as i32, z as i32))
    }
}
