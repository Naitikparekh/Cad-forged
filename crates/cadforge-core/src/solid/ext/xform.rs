//! 3D affine helpers on top of [`M3`]: composition, rotation about an axis, scaling, reflection and frame alignment.
use crate::geom::Mat;
use crate::solid::mesh::{m3, M3};
use crate::solid::V3;

/// `a ∘ b`: apply `b` first, then `a`.
pub fn mul(a: &M3, b: &M3) -> M3 {
    let mut r = [0.0; 12];
    for i in 0..3 {
        for j in 0..3 {
            r[i * 4 + j] = (0..3).map(|k| a[i * 4 + k] * b[k * 4 + j]).sum();
        }
        r[i * 4 + 3] = (0..3).map(|k| a[i * 4 + k] * b[k * 4 + 3]).sum::<f64>() + a[i * 4 + 3];
    }
    r
}

/// Linear part only, with `t` as translation.
fn linear(rows: [[f64; 3]; 3], t: V3) -> M3 {
    [rows[0][0], rows[0][1], rows[0][2], t.x, rows[1][0], rows[1][1], rows[1][2], t.y, rows[2][0], rows[2][1], rows[2][2], t.z]
}

/// Keeps the point `o` fixed under the linear map `rows`.
fn about(rows: [[f64; 3]; 3], o: V3) -> M3 {
    let m = linear(rows, V3::default());
    let fixed = m3::apply(&m, o);
    linear(rows, o.sub(fixed))
}

/// Rotation by `ang` (right-hand rule) about the line through `o` with direction `axis`.
pub fn rotate_axis(o: V3, axis: V3, ang: f64) -> M3 {
    let u = axis.unit();
    let (c, s) = (ang.cos(), ang.sin());
    let k = 1.0 - c;
    about(
        [
            [c + u.x * u.x * k, u.x * u.y * k - u.z * s, u.x * u.z * k + u.y * s],
            [u.y * u.x * k + u.z * s, c + u.y * u.y * k, u.y * u.z * k - u.x * s],
            [u.z * u.x * k - u.y * s, u.z * u.y * k + u.x * s, c + u.z * u.z * k],
        ],
        o,
    )
}

/// Scale by `s` along x, y and z about `o`.
pub fn scale_axes(o: V3, s: V3) -> M3 {
    about([[s.x, 0.0, 0.0], [0.0, s.y, 0.0], [0.0, 0.0, s.z]], o)
}

/// Reflection in the plane `n · p = d` (`n` need not be a unit vector).
pub fn reflect(n: V3, d: f64) -> M3 {
    let n = n.unit();
    let r = [[1.0 - 2.0 * n.x * n.x, -2.0 * n.x * n.y, -2.0 * n.x * n.z], [-2.0 * n.y * n.x, 1.0 - 2.0 * n.y * n.y, -2.0 * n.y * n.z], [-2.0 * n.z * n.x, -2.0 * n.z * n.y, 1.0 - 2.0 * n.z * n.z]];
    linear(r, n.scale(2.0 * d))
}

/// Orthonormal frame (x, y, z) with x along `x` and the plane spanned by `x` and `in_plane`; `None` if they are parallel.
fn frame(x: V3, in_plane: V3) -> Option<[V3; 3]> {
    let n = x.cross(in_plane);
    if n.len() < 1e-9 * x.len().max(1.0) * in_plane.len().max(1.0) {
        return None;
    }
    let (ex, ez) = (x.unit(), n.unit());
    Some([ex, ez.cross(ex), ez])
}

/// Rigid motion taking the points `src` onto `dst` (one, two or three pairs): a translation, then the smallest
/// rotation aligning the first edge, then the rotation aligning the plane of three points.
pub fn align_points(src: &[V3], dst: &[V3]) -> M3 {
    let n = src.len().min(dst.len());
    let shift = m3::translate(dst[0].x - src[0].x, dst[0].y - src[0].y, dst[0].z - src[0].z);
    if n < 2 {
        return shift;
    }
    let (e_s, e_d) = (src[1].sub(src[0]).unit(), dst[1].sub(dst[0]).unit());
    let axis = e_s.cross(e_d);
    let turn = if axis.len() < 1e-12 {
        if e_s.dot(e_d) < 0.0 {
            let side = if e_s.x.abs() < 0.9 { V3::new(1.0, 0.0, 0.0) } else { V3::new(0.0, 1.0, 0.0) };
            rotate_axis(dst[0], e_s.cross(side), std::f64::consts::PI)
        } else {
            m3::identity()
        }
    } else {
        rotate_axis(dst[0], axis, axis.len().atan2(e_s.dot(e_d)))
    };
    let step1 = mul(&turn, &shift);
    if n < 3 {
        return step1;
    }
    let moved: Vec<V3> = src.iter().take(3).map(|p| m3::apply(&step1, *p)).collect();
    let (Some(fs), Some(fd)) = (frame(moved[1].sub(moved[0]), moved[2].sub(moved[0])), frame(dst[1].sub(dst[0]), dst[2].sub(dst[0]))) else { return step1 };
    // R = Fd · Fsᵀ, with the frames as matrix columns
    let col = |a: &[V3; 3], i: usize, j: usize| [a[j].x, a[j].y, a[j].z][i];
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| col(&fd, i, k) * col(&fs, j, k)).sum();
        }
    }
    mul(&about(r, dst[0]), &step1)
}

/// The 2D drawing-plane map of `m` when it keeps the plane z = 0 in place (so drawing objects can follow it).
pub fn to_plan_mat(m: &M3) -> Option<Mat> {
    if m[8].abs() > 1e-9 || m[9].abs() > 1e-9 || m[11].abs() > 1e-9 {
        return None;
    }
    Some([m[0], m[4], m[1], m[5], m[3], m[7]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: V3, b: V3) -> bool {
        a.sub(b).len() < 1e-9
    }

    #[test]
    fn rotation_about_an_offset_axis() {
        let m = rotate_axis(V3::new(1.0, 0.0, 0.0), V3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2);
        assert!(close(m3::apply(&m, V3::new(2.0, 0.0, 0.0)), V3::new(1.0, 1.0, 0.0)));
        assert!(close(m3::apply(&m, V3::new(1.0, 0.0, 5.0)), V3::new(1.0, 0.0, 5.0)));
    }

    #[test]
    fn reflection_and_scaling_fix_their_reference() {
        let m = reflect(V3::new(0.0, 0.0, 1.0), 4.0);
        assert!(close(m3::apply(&m, V3::new(1.0, 2.0, 1.0)), V3::new(1.0, 2.0, 7.0)));
        assert!(m3::det(&m) < 0.0);
        let s = scale_axes(V3::new(1.0, 1.0, 1.0), V3::new(2.0, 1.0, 3.0));
        assert!(close(m3::apply(&s, V3::new(1.0, 1.0, 1.0)), V3::new(1.0, 1.0, 1.0)));
        assert!(close(m3::apply(&s, V3::new(2.0, 2.0, 2.0)), V3::new(3.0, 2.0, 4.0)));
    }

    #[test]
    fn point_alignment_maps_sources_onto_destinations() {
        let src = [V3::new(0.0, 0.0, 0.0), V3::new(1.0, 0.0, 0.0), V3::new(0.0, 1.0, 0.0)];
        let dst = [V3::new(5.0, 5.0, 0.0), V3::new(5.0, 6.0, 0.0), V3::new(4.0, 5.0, 0.0)];
        for n in 1..=3 {
            let m = align_points(&src[..n], &dst[..n]);
            assert!(close(m3::apply(&m, src[0]), dst[0]));
            if n >= 2 {
                assert!(close(m3::apply(&m, src[1]), dst[1]));
            }
            if n == 3 {
                assert!(close(m3::apply(&m, src[2]), dst[2]));
            }
        }
    }

    #[test]
    fn plan_maps_need_the_drawing_plane_kept() {
        assert!(to_plan_mat(&m3::translate(1.0, 2.0, 0.0)).is_some());
        assert!(to_plan_mat(&m3::translate(1.0, 2.0, 1.0)).is_none());
        assert!(to_plan_mat(&rotate_axis(V3::default(), V3::new(1.0, 0.0, 0.0), 0.3)).is_none());
    }
}
