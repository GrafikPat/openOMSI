//! `.sli` spline definitions (unit `mc_splines`).

use crate::PathDef;
use omsi_cfg::CfgFile;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SplineProfilePoint {
    /// Lateral position (m, positive = right).
    pub x: f32,
    /// Height above the spline (m).
    pub z: f32,
    /// Texture u coordinate.
    pub u: f32,
    /// Texture scale along the spline (v per metre).
    pub v_scale: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SplineProfile {
    pub texture: usize,
    pub points: Vec<SplineProfilePoint>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeightProfile {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
}

/// `[patchwork_chain]` after a `[texture]`: the texture is cut lengthwise into as many parts
/// as there are weights, and the spline shows them one after another in a random order in
/// which the letters of `chain` join up (part i runs from letter i to letter i+1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PatchworkChain {
    /// Metres per part (roughly: the spline is cut into `trunc(length / this) + 1`).
    pub segment_length: f32,
    pub chain: String,
    /// One digit per part: how often it is drawn among those that fit.
    pub weights: String,
    /// One `0`/`1` per part: whether it may also be laid backwards.
    pub invertable: String,
}

impl PatchworkChain {
    /// Whether OMSI takes the chain (Omsi.exe sub_5ab908: as many weights and inverse flags
    /// as the chain has transitions, the flags only `0` and `1`; else it is left out).
    pub fn valid(&self) -> bool {
        let n = self.chain.len();
        n >= 2
            && self.weights.len() + 1 == n
            && self.invertable.len() + 1 == n
            && self.invertable.bytes().all(|b| b == b'0' || b == b'1')
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SplineTexture {
    pub file: String,
    pub patchwork: Option<PatchworkChain>,
    pub alpha: i32,
    /// `[scaleTexByLength]` after this texture: its v runs over the whole spline, however
    /// long (the sagging wires), instead of per metre.
    pub scale_by_length: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RailEnh {
    pub values: [f32; 8],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThirdRail {
    pub values: [f32; 6],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Spline {
    pub path: PathBuf,
    pub length: f32,
    pub textures: Vec<SplineTexture>,
    pub scale_tex_by_length: bool,
    pub height_profiles: Vec<HeightProfile>,
    pub profiles: Vec<SplineProfile>,
    pub paths: Vec<PathDef>,
    pub rail_enh: Vec<RailEnh>,
    pub third_rail: Vec<ThirdRail>,
    pub half_cant_width: Option<f32>,
    pub only_editor: bool,
    /// `[terrainholeprofile]`s with their `[terrainholeprofilepnt]`s (x across, height, and
    /// how far an end of the hole stands off the spline's end): the outline a spline laid
    /// with `[spline_terrain_align]` cuts out of the ground. None given, Omsi.exe makes them
    /// from the drawn profiles (see `omsi_geometry::terrain_hole_profiles`).
    pub terrain_hole_profiles: Vec<Vec<[f32; 3]>>,
    pub unknown_keywords: Vec<(String, usize)>,
}

impl Spline {
    pub fn load(path: &Path) -> Result<Spline, omsi_cfg::CfgError> {
        let f = CfgFile::read(path)?;
        Ok(Self::parse(&f))
    }

    pub fn parse(file: &CfgFile) -> Spline {
        let mut s = Spline { path: file.path.clone(), ..Default::default() };
        let mut r = file.reader();
        // the texture the material statements go to: the last `[texture]`, or the one a
        // `[matl]` named
        let mut matl: Option<usize> = None;
        while let Some(k) = r.next_keyword() {
            match k.as_str() {
                "length" => s.length = r.f32(),
                "texture" => {
                    s.textures.push(SplineTexture { file: r.str().to_string(), ..Default::default() });
                    matl = None;
                }
                "matl" => {
                    // `[matl] <texture> <index>` as in a model.cfg: selects the texture the
                    // following material statements change. The Simple Spline Editor writes
                    // one after every `[texture]` (this installation: 5956 of 5969 name the texture
                    // just begun, 13 another one; index 0 in 5981 of 6013), so without it the file counted as failed
                    // although `[matl_alpha]` was already taken.
                    let texture = r.str().to_string();
                    let _index = r.i32();
                    matl = s.textures.iter().rposition(|t| t.file.eq_ignore_ascii_case(&texture));
                }
                "scaletexbylength" => {
                    // (a flag of the last texture, Omsi.exe 0x5ac01f)
                    s.scale_tex_by_length = true;
                    if let Some(t) = s.textures.last_mut() {
                        t.scale_by_length = true;
                    }
                }
                "patchwork_chain" => {
                    let pc = PatchworkChain { segment_length: r.f32(), chain: r.word().to_string(), weights: r.word().to_string(), invertable: r.word().to_string() };
                    if let Some(t) = s.textures.last_mut() {
                        t.patchwork = Some(pc);
                    }
                }
                "matl_alpha" => {
                    let a = r.i32();
                    let i = matl.or(s.textures.len().checked_sub(1));
                    if let Some(t) = i.and_then(|i| s.textures.get_mut(i)) {
                        t.alpha = a;
                    }
                }
                "heightprofile" => {
                    let v = r.f32s::<4>();
                    s.height_profiles.push(HeightProfile { x0: v[0], x1: v[1], z0: v[2], z1: v[3] });
                }
                "profile" => {
                    s.profiles.push(SplineProfile { texture: r.usize(), points: Vec::new() });
                }
                "profilepnt" => {
                    let v = r.f32s::<4>();
                    if let Some(p) = s.profiles.last_mut() {
                        p.points.push(SplineProfilePoint { x: v[0], z: v[1], u: v[2], v_scale: v[3] });
                    }
                }
                "path" => {
                    let kind = r.i32();
                    let x = r.f32();
                    let z = r.f32();
                    let width = r.f32();
                    let direction = r.i32();
                    s.paths.push(PathDef { kind, start: [x, 0.0, z], width, direction, ..Default::default() });
                }
                "path_2" => {
                    let kind = r.i32();
                    let x = r.f32();
                    let z = r.f32();
                    let width = r.f32();
                    let direction = r.i32();
                    let extra = r.f32();
                    s.paths.push(PathDef { kind, start: [x, 0.0, z], width, direction, params: vec![extra], ..Default::default() });
                }
                "rail_enh" => s.rail_enh.push(RailEnh { values: r.f32s::<8>() }),
                "third_rail" => s.third_rail.push(ThirdRail { values: r.f32s::<6>() }),
                "halfcantwidth" => s.half_cant_width = Some(r.f32()),
                "onlyeditor" => s.only_editor = true,
                // (a point goes to the last profile begun, Omsi.exe 0x5ad923: none begun, it
                // is dropped)
                "terrainholeprofile" => s.terrain_hole_profiles.push(Vec::new()),
                "terrainholeprofilepnt" => {
                    let v = r.f32s::<3>();
                    if let Some(p) = s.terrain_hole_profiles.last_mut() {
                        p.push(v);
                    }
                }
                _ => s.unknown_keywords.push((k, r.block_line())),
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `[matl] <texture> 0` after a `[texture]`, as the Simple Spline Editor writes it (2981 of
    // the installation's 17 969 .sli): the material statements go to the named texture, and
    // the keyword is known, so the file no longer counts as failed
    #[test]
    fn matl_selects_the_texture_for_the_material_statements() {
        let text = "[texture]\r\nErde.jpg\r\n[matl]\r\nErde.jpg\r\n0\r\n[matl_alpha]\r\n2\r\n[texture]\r\nZaun.tga\r\n[matl]\r\nerde.JPG\r\n0\r\n[matl_alpha]\r\n1\r\n[profile]\r\n0\r\n[profilepnt]\r\n-2\r\n0.1\r\n0\r\n1\r\n";
        let s = Spline::parse(&omsi_cfg::CfgFile::from_str("Feldweg_4m_1.sli", text));
        assert!(s.unknown_keywords.is_empty(), "{:?}", s.unknown_keywords);
        assert_eq!(s.textures.len(), 2);
        // the second block names the first texture again (case-insensitively): it changes
        // Erde.jpg, not the texture begun last
        assert_eq!(s.textures[0].alpha, 1);
        assert_eq!(s.textures[1].alpha, 0);
    }

    // without a `[matl]`, `[matl_alpha]` goes to the last texture as before
    #[test]
    fn matl_alpha_without_matl_goes_to_the_last_texture() {
        let text = "[texture]\r\nA.jpg\r\n[texture]\r\nB.jpg\r\n[matl_alpha]\r\n1\r\n";
        let s = Spline::parse(&omsi_cfg::CfgFile::from_str("x.sli", text));
        assert_eq!((s.textures[0].alpha, s.textures[1].alpha), (0, 1));
    }
}
