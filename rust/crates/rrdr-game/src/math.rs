//! Small `Vec3` helpers on top of [`rrdr_bridge::Vec3`].

use rrdr_bridge::Vec3;

/// Vector helpers used by the game layer.
pub trait Vec3Ops: Copy {
    /// Squared length.
    fn length_squared(self) -> f32;
    /// Euclidean length.
    fn length(self) -> f32;
    /// Component-wise addition.
    fn add(self, other: Vec3) -> Vec3;
    /// Component-wise subtraction.
    fn sub(self, other: Vec3) -> Vec3;
    /// Dot product.
    fn dot(self, other: Vec3) -> f32;
    /// Cross product.
    fn cross(self, other: Vec3) -> Vec3;
    /// Normalized vector, or zero for a zero-length vector.
    fn normalized(self) -> Vec3;
    /// Squared distance.
    fn distance_squared(self, other: Vec3) -> f32;
    /// Euclidean distance.
    fn distance(self, other: Vec3) -> f32;
    /// True when all components are finite.
    fn is_finite(self) -> bool;
}

impl Vec3Ops for Vec3 {
    fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    fn add(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    fn sub(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    fn dot(self, other: Vec3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    fn normalized(self) -> Vec3 {
        let length = self.length();
        if length > f32::EPSILON {
            Vec3::new(self.x / length, self.y / length, self.z / length)
        } else {
            Vec3::default()
        }
    }

    fn distance_squared(self, other: Vec3) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    fn distance(self, other: Vec3) -> f32 {
        self.distance_squared(other).sqrt()
    }

    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

// Vec3 comes from rrdr-bridge. The orphan rules forbid implementing
// `std::ops::Add`/`Sub` for a foreign type, so addition and subtraction are
// methods on `Vec3Ops` instead.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_math() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(3.0, 4.0, 0.0);

        assert_eq!(a.distance(b), 5.0);
        assert_eq!(a.dot(b), 0.0);
        assert_eq!(a.add(b), b);
        assert_eq!(b.sub(a), b);
        assert_eq!(b.normalized().length(), 1.0);
        assert!(b.is_finite());
        assert!(!Vec3::new(f32::NAN, 0.0, 0.0).is_finite());
    }
}
