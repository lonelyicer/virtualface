//! v2 parameter derivation, ported from VRCFT `UnifiedExpressionsParameters.cs`.

use vf_abi::{UnifiedExpression as U, UnifiedSimpleExpression as S, VfUnifiedFrame};

fn sh(f: &VfUnifiedFrame, e: U) -> f32 {
    f.shapes[e.index()]
}

fn simple(f: &VfUnifiedFrame, s: S) -> f32 {
    match s {
        S::BrowUpRight => sh(f, U::BrowOuterUpRight) * 0.60 + sh(f, U::BrowInnerUpRight) * 0.40,
        S::BrowUpLeft => sh(f, U::BrowOuterUpLeft) * 0.60 + sh(f, U::BrowInnerUpLeft) * 0.40,
        S::BrowDownRight => sh(f, U::BrowLowererRight) * 0.75 + sh(f, U::BrowPinchRight) * 0.25,
        S::BrowDownLeft => sh(f, U::BrowLowererLeft) * 0.75 + sh(f, U::BrowPinchLeft) * 0.25,
        S::MouthSmileRight => {
            sh(f, U::MouthCornerPullRight) * 0.8 + sh(f, U::MouthCornerSlantRight) * 0.2
        }
        S::MouthSmileLeft => {
            sh(f, U::MouthCornerPullLeft) * 0.8 + sh(f, U::MouthCornerSlantLeft) * 0.2
        }
        S::MouthSadRight => sh(f, U::MouthFrownRight).max(sh(f, U::MouthStretchRight)),
        S::MouthSadLeft => sh(f, U::MouthFrownLeft).max(sh(f, U::MouthStretchLeft)),
    }
}

fn max2(a: f32, b: f32) -> f32 {
    a.max(b)
}

/// All OSC float parameters produced for a frame (name without `/avatar/parameters/` prefix).
pub fn derive_v2(f: &VfUnifiedFrame) -> Vec<(String, f32)> {
    let mut out = Vec::with_capacity(280);
    let push = |out: &mut Vec<(String, f32)>, name: &str, v: f32| {
        out.push((name.to_string(), v));
    };

    // Gaze (EParam Vector2 → X/Y)
    push(&mut out, "v2/EyeLeftX", f.eye.left.gaze[0]);
    push(&mut out, "v2/EyeLeftY", f.eye.left.gaze[1]);
    push(&mut out, "v2/EyeRightX", f.eye.right.gaze[0]);
    push(&mut out, "v2/EyeRightY", f.eye.right.gaze[1]);
    let cg = f.eye.combined_gaze();
    push(&mut out, "v2/EyeX", cg[0]);
    push(&mut out, "v2/EyeY", cg[1]);

    let avg_pupil = (f.eye.left.pupil_mm + f.eye.right.pupil_mm) * 0.5;
    push(&mut out, "v2/PupilDilation", avg_pupil); // Combined() normalizes in VRCFT; we send mm-ish
    push(&mut out, "v2/PupilDiameterLeft", f.eye.left.pupil_mm * 0.1);
    push(
        &mut out,
        "v2/PupilDiameterRight",
        f.eye.right.pupil_mm * 0.1,
    );
    push(
        &mut out,
        "v2/PupilDiameter",
        (f.eye.left.pupil_mm + f.eye.right.pupil_mm) * 0.05,
    );

    push(&mut out, "v2/EyeOpenLeft", f.eye.left.openness);
    push(&mut out, "v2/EyeOpenRight", f.eye.right.openness);
    push(&mut out, "v2/EyeOpen", f.eye.combined_openness());
    push(&mut out, "v2/EyeClosedLeft", 1.0 - f.eye.left.openness);
    push(&mut out, "v2/EyeClosedRight", 1.0 - f.eye.right.openness);
    push(&mut out, "v2/EyeClosed", 1.0 - f.eye.combined_openness());

    push(
        &mut out,
        "v2/EyeWide",
        max2(sh(f, U::EyeWideLeft), sh(f, U::EyeWideRight)),
    );
    push(
        &mut out,
        "v2/EyeLidLeft",
        f.eye.left.openness * 0.75 + sh(f, U::EyeWideLeft) * 0.25,
    );
    push(
        &mut out,
        "v2/EyeLidRight",
        f.eye.right.openness * 0.75 + sh(f, U::EyeWideRight) * 0.25,
    );
    push(
        &mut out,
        "v2/EyeLid",
        f.eye.combined_openness() * 0.75
            + (sh(f, U::EyeWideLeft) + sh(f, U::EyeWideRight)) * 0.5 * 0.25,
    );
    push(
        &mut out,
        "v2/EyeSquint",
        max2(sh(f, U::EyeSquintLeft), sh(f, U::EyeSquintRight)),
    );
    push(
        &mut out,
        "v2/EyesSquint",
        max2(sh(f, U::EyeSquintLeft), sh(f, U::EyeSquintRight)),
    );

    push(
        &mut out,
        "v2/BrowUp",
        (simple(f, S::BrowUpRight) + simple(f, S::BrowUpLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/BrowDown",
        (simple(f, S::BrowDownRight) + simple(f, S::BrowDownLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/BrowInnerUp",
        (sh(f, U::BrowInnerUpLeft) + sh(f, U::BrowInnerUpRight)) * 0.5,
    );
    push(
        &mut out,
        "v2/BrowOuterUp",
        (sh(f, U::BrowOuterUpLeft) + sh(f, U::BrowOuterUpRight)) * 0.5,
    );
    push(
        &mut out,
        "v2/BrowExpressionRight",
        (sh(f, U::BrowInnerUpRight) * 0.5 + sh(f, U::BrowOuterUpRight) * 0.5).min(1.0)
            - simple(f, S::BrowDownRight),
    );
    push(
        &mut out,
        "v2/BrowExpressionLeft",
        (sh(f, U::BrowInnerUpLeft) * 0.5 + sh(f, U::BrowOuterUpLeft) * 0.5).min(1.0)
            - simple(f, S::BrowDownLeft),
    );
    push(
        &mut out,
        "v2/BrowExpression",
        ((sh(f, U::BrowInnerUpRight) * 0.5 + sh(f, U::BrowOuterUpRight) * 0.5).min(1.0)
            - simple(f, S::BrowDownRight)
            + (sh(f, U::BrowInnerUpLeft) * 0.5 + sh(f, U::BrowOuterUpLeft) * 0.5).min(1.0)
            - simple(f, S::BrowDownLeft))
            * 0.5,
    );

    push(&mut out, "v2/JawX", sh(f, U::JawRight) - sh(f, U::JawLeft));
    push(
        &mut out,
        "v2/JawZ",
        sh(f, U::JawForward) - sh(f, U::JawBackward),
    );

    push(
        &mut out,
        "v2/CheekSquint",
        (sh(f, U::CheekSquintLeft) + sh(f, U::CheekSquintRight)) * 0.5,
    );
    push(
        &mut out,
        "v2/CheekPuffSuckLeft",
        sh(f, U::CheekPuffLeft) - sh(f, U::CheekSuckLeft),
    );
    push(
        &mut out,
        "v2/CheekPuffSuckRight",
        sh(f, U::CheekPuffRight) - sh(f, U::CheekSuckRight),
    );
    push(
        &mut out,
        "v2/CheekPuffSuck",
        (sh(f, U::CheekPuffRight) + sh(f, U::CheekPuffLeft)) * 0.5
            - (sh(f, U::CheekSuckRight) + sh(f, U::CheekSuckLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/CheekSuck",
        (sh(f, U::CheekSuckLeft) + sh(f, U::CheekSuckRight)) * 0.5,
    );

    push(
        &mut out,
        "v2/MouthUpperX",
        sh(f, U::MouthUpperRight) - sh(f, U::MouthUpperLeft),
    );
    push(
        &mut out,
        "v2/MouthLowerX",
        sh(f, U::MouthLowerRight) - sh(f, U::MouthLowerLeft),
    );
    push(
        &mut out,
        "v2/MouthX",
        (sh(f, U::MouthUpperRight) + sh(f, U::MouthLowerRight)) * 0.5
            - (sh(f, U::MouthUpperLeft) + sh(f, U::MouthLowerLeft)) * 0.5,
    );

    push(
        &mut out,
        "v2/LipSuckUpper",
        (sh(f, U::LipSuckUpperRight) + sh(f, U::LipSuckUpperLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipSuckLower",
        (sh(f, U::LipSuckLowerRight) + sh(f, U::LipSuckLowerLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipSuck",
        (sh(f, U::LipSuckUpperRight)
            + sh(f, U::LipSuckUpperLeft)
            + sh(f, U::LipSuckLowerRight)
            + sh(f, U::LipSuckLowerLeft))
            * 0.25,
    );
    push(
        &mut out,
        "v2/LipFunnelUpper",
        (sh(f, U::LipFunnelUpperRight) + sh(f, U::LipFunnelUpperLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipFunnelLower",
        (sh(f, U::LipFunnelLowerRight) + sh(f, U::LipFunnelLowerLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipFunnel",
        (sh(f, U::LipFunnelUpperRight)
            + sh(f, U::LipFunnelUpperLeft)
            + sh(f, U::LipFunnelLowerRight)
            + sh(f, U::LipFunnelLowerLeft))
            * 0.25,
    );
    push(
        &mut out,
        "v2/LipPuckerUpper",
        (sh(f, U::LipPuckerUpperRight) + sh(f, U::LipPuckerUpperLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipPuckerLower",
        (sh(f, U::LipPuckerLowerRight) + sh(f, U::LipPuckerLowerLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipPuckerRight",
        (sh(f, U::LipPuckerUpperRight) + sh(f, U::LipPuckerLowerRight)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipPuckerLeft",
        (sh(f, U::LipPuckerUpperLeft) + sh(f, U::LipPuckerLowerLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipPucker",
        (sh(f, U::LipPuckerUpperRight)
            + sh(f, U::LipPuckerUpperLeft)
            + sh(f, U::LipPuckerLowerRight)
            + sh(f, U::LipPuckerLowerLeft))
            * 0.25,
    );
    push(
        &mut out,
        "v2/LipSuckFunnelUpper",
        (sh(f, U::LipSuckUpperRight) + sh(f, U::LipSuckUpperLeft)) * 0.5
            - (sh(f, U::LipFunnelUpperRight) + sh(f, U::LipFunnelUpperLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipSuckFunnelLower",
        (sh(f, U::LipSuckLowerRight) + sh(f, U::LipSuckLowerLeft)) * 0.5
            - (sh(f, U::LipFunnelLowerRight) + sh(f, U::LipFunnelLowerLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/LipSuckFunnelLowerLeft",
        sh(f, U::LipSuckLowerLeft) - sh(f, U::LipFunnelLowerLeft),
    );
    push(
        &mut out,
        "v2/LipSuckFunnelLowerRight",
        sh(f, U::LipSuckLowerRight) - sh(f, U::LipFunnelLowerRight),
    );
    push(
        &mut out,
        "v2/LipSuckFunnelUpperLeft",
        sh(f, U::LipSuckUpperLeft) - sh(f, U::LipFunnelUpperLeft),
    );
    push(
        &mut out,
        "v2/LipSuckFunnelUpperRight",
        sh(f, U::LipSuckUpperRight) - sh(f, U::LipFunnelUpperRight),
    );

    push(
        &mut out,
        "v2/MouthUpperUp",
        sh(f, U::MouthUpperUpRight) * 0.5 + sh(f, U::MouthUpperUpLeft) * 0.5,
    );
    push(
        &mut out,
        "v2/MouthLowerDown",
        sh(f, U::MouthLowerDownRight) * 0.5 + sh(f, U::MouthLowerDownLeft) * 0.5,
    );
    push(
        &mut out,
        "v2/MouthOpen",
        sh(f, U::MouthUpperUpRight) * 0.25
            + sh(f, U::MouthUpperUpLeft) * 0.25
            + sh(f, U::MouthLowerDownRight) * 0.25
            + sh(f, U::MouthLowerDownLeft) * 0.25,
    );
    push(
        &mut out,
        "v2/MouthStretch",
        (sh(f, U::MouthStretchRight) + sh(f, U::MouthStretchLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/MouthTightener",
        (sh(f, U::MouthTightenerRight) + sh(f, U::MouthTightenerLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/MouthPress",
        (sh(f, U::MouthPressRight) + sh(f, U::MouthPressLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/MouthDimple",
        (sh(f, U::MouthDimpleRight) + sh(f, U::MouthDimpleLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/NoseSneer",
        (sh(f, U::NoseSneerRight) + sh(f, U::NoseSneerLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/MouthTightenerStretch",
        (sh(f, U::MouthTightenerRight) + sh(f, U::MouthTightenerLeft)) * 0.5
            - (sh(f, U::MouthStretchRight) + sh(f, U::MouthStretchLeft)) * 0.5,
    );
    push(
        &mut out,
        "v2/MouthTightenerStretchLeft",
        sh(f, U::MouthTightenerLeft) - sh(f, U::MouthStretchLeft),
    );
    push(
        &mut out,
        "v2/MouthTightenerStretchRight",
        sh(f, U::MouthTightenerRight) - sh(f, U::MouthStretchRight),
    );
    push(
        &mut out,
        "v2/MouthCornerYLeft",
        sh(f, U::MouthCornerSlantLeft) - sh(f, U::MouthFrownLeft),
    );
    push(
        &mut out,
        "v2/MouthCornerYRight",
        sh(f, U::MouthCornerSlantRight) - sh(f, U::MouthFrownRight),
    );
    push(
        &mut out,
        "v2/MouthCornerY",
        (sh(f, U::MouthCornerSlantLeft) - sh(f, U::MouthFrownLeft)
            + sh(f, U::MouthCornerSlantRight)
            - sh(f, U::MouthFrownRight))
            * 0.5,
    );
    push(
        &mut out,
        "v2/SmileFrownRight",
        simple(f, S::MouthSmileRight) - sh(f, U::MouthFrownRight),
    );
    push(
        &mut out,
        "v2/SmileFrownLeft",
        simple(f, S::MouthSmileLeft) - sh(f, U::MouthFrownLeft),
    );
    push(
        &mut out,
        "v2/SmileSadRight",
        simple(f, S::MouthSmileRight) - simple(f, S::MouthSadRight),
    );
    push(
        &mut out,
        "v2/SmileSadLeft",
        simple(f, S::MouthSmileLeft) - simple(f, S::MouthSadLeft),
    );
    push(
        &mut out,
        "v2/SmileFrown",
        simple(f, S::MouthSmileRight) * 0.5 + simple(f, S::MouthSmileLeft) * 0.5
            - sh(f, U::MouthFrownRight) * 0.5
            - sh(f, U::MouthFrownLeft) * 0.5,
    );
    push(
        &mut out,
        "v2/SmileSad",
        (simple(f, S::MouthSmileLeft) + simple(f, S::MouthSmileRight)) * 0.5
            - (simple(f, S::MouthSadLeft) + simple(f, S::MouthSadRight)) * 0.5,
    );

    push(
        &mut out,
        "v2/TongueX",
        sh(f, U::TongueRight) - sh(f, U::TongueLeft),
    );
    push(
        &mut out,
        "v2/TongueY",
        sh(f, U::TongueUp) - sh(f, U::TongueDown),
    );
    push(
        &mut out,
        "v2/TongueArchY",
        sh(f, U::TongueCurlUp) - sh(f, U::TongueBendDown),
    );
    push(
        &mut out,
        "v2/TongueShape",
        sh(f, U::TongueFlat) - sh(f, U::TongueSquish),
    );

    for e in U::ALL {
        push(&mut out, &format!("v2/{}", e.name()), sh(f, e));
    }
    for s in S::ALL {
        push(&mut out, &format!("v2/{}", s.name()), simple(f, s));
    }

    push(&mut out, "v2/Head/Yaw", f.head.yaw);
    push(&mut out, "v2/Head/Pitch", f.head.pitch);
    push(&mut out, "v2/Head/Roll", f.head.roll);
    push(&mut out, "v2/Head/PosX", f.head.pos[0]);
    push(&mut out, "v2/Head/PosY", f.head.pos[1]);
    push(&mut out, "v2/Head/PosZ", f.head.pos[2]);

    push(&mut out, "EyesX", cg[0]);
    push(&mut out, "EyesY", cg[1]);
    push(&mut out, "LeftEyeX", f.eye.left.gaze[0]);
    push(&mut out, "LeftEyeY", f.eye.left.gaze[1]);
    push(&mut out, "RightEyeX", f.eye.right.gaze[0]);
    push(&mut out, "RightEyeY", f.eye.right.gaze[1]);
    push(&mut out, "LeftEyeWiden", sh(f, U::EyeWideLeft));
    push(&mut out, "RightEyeWiden", sh(f, U::EyeWideRight));
    push(
        &mut out,
        "EyeWiden",
        (sh(f, U::EyeWideLeft) + sh(f, U::EyeWideRight)) * 0.5,
    );
    push(&mut out, "LeftEyeSqueeze", sh(f, U::EyeSquintLeft));
    push(&mut out, "RightEyeSqueeze", sh(f, U::EyeSquintRight));
    push(
        &mut out,
        "EyesSqueeze",
        (sh(f, U::EyeSquintLeft) + sh(f, U::EyeSquintRight)) * 0.5,
    );
    push(&mut out, "EyesDilation", avg_pupil);
    push(
        &mut out,
        "EyesPupilDiameter",
        (f.eye.left.pupil_mm + f.eye.right.pupil_mm) * 0.5,
    );
    push(&mut out, "LeftEyeLid", f.eye.left.openness);
    push(&mut out, "RightEyeLid", f.eye.right.openness);
    push(&mut out, "CombinedEyeLid", f.eye.combined_openness());
    push(
        &mut out,
        "LeftEyeLidExpanded",
        sh(f, U::EyeWideLeft) * 0.2 + f.eye.left.openness * 0.8,
    );
    push(
        &mut out,
        "RightEyeLidExpanded",
        sh(f, U::EyeWideRight) * 0.2 + f.eye.right.openness * 0.8,
    );
    push(
        &mut out,
        "EyeLidExpanded",
        (sh(f, U::EyeWideLeft) + sh(f, U::EyeWideRight)) * 0.1
            + (f.eye.left.openness + f.eye.right.openness) * 0.4,
    );

    out
}

pub fn encode_binary(addr: &str, value: f32, bits: u32) -> Vec<(String, bool)> {
    let mut msgs = Vec::new();
    msgs.push((format!("{addr}Negative"), value < 0.0));
    let v = value.abs();
    let max_int = 2u32.pow(bits.max(1));
    let big = if v > 0.99999 {
        max_int as i32
    } else {
        (v * max_int as f32) as i32
    };
    for i in 0..bits {
        let idx = 1u32 << i;
        msgs.push((format!("{addr}{idx}"), ((big >> i) & 1) == 1));
    }
    msgs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jaw_x_and_eyelid() {
        let mut f = VfUnifiedFrame::default();
        f.shapes[U::JawRight.index()] = 0.4;
        f.shapes[U::JawLeft.index()] = 0.1;
        f.eye.left.openness = 1.0;
        f.shapes[U::EyeWideLeft.index()] = 0.0;
        let p = derive_v2(&f);
        let jaw_x = p.iter().find(|(n, _)| n == "v2/JawX").unwrap().1;
        assert!((jaw_x - 0.3).abs() < 1e-5);
        let lid = p.iter().find(|(n, _)| n == "v2/EyeLidLeft").unwrap().1;
        assert!((lid - 0.75).abs() < 1e-5);
        assert!(p.iter().any(|(n, v)| n == "v2/JawOpen" && *v == 0.0));
        assert!(p.iter().any(|(n, _)| n == "v2/Head/Yaw"));
        assert!(p.iter().any(|(n, _)| n == "EyesX"));
        assert!(p.iter().any(|(n, _)| n == "v2/MouthStretch"));
        assert!(p.iter().any(|(n, _)| n == "v2/LipFunnelUpper"));
    }
}
