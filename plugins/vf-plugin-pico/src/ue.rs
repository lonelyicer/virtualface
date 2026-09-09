//! PICO / ARKit-order weights → Unified Expressions.
//! Ported from VRCFTPicoModule `Updater.cs`.

use crate::remap::pico_to_arkit;
use vf_abi::{UnifiedExpression as U, VF_VALID_EXPR, VF_VALID_EYE, VfUnifiedFrame};

const A_EYE_BLINK_L: usize = 0;
const A_EYE_LOOK_DOWN_L: usize = 1;
const A_EYE_LOOK_IN_L: usize = 2;
const A_EYE_LOOK_OUT_L: usize = 3;
const A_EYE_LOOK_UP_L: usize = 4;
const A_EYE_SQUINT_L: usize = 5;
const A_EYE_WIDE_L: usize = 6;
const A_EYE_BLINK_R: usize = 7;
const A_EYE_LOOK_DOWN_R: usize = 8;
const A_EYE_LOOK_IN_R: usize = 9;
const A_EYE_LOOK_OUT_R: usize = 10;
const A_EYE_LOOK_UP_R: usize = 11;
const A_EYE_SQUINT_R: usize = 12;
const A_EYE_WIDE_R: usize = 13;
const A_JAW_FORWARD: usize = 14;
const A_JAW_LEFT: usize = 15;
const A_JAW_RIGHT: usize = 16;
const A_JAW_OPEN: usize = 17;
const A_MOUTH_CLOSE: usize = 18;
const A_MOUTH_FUNNEL: usize = 19;
const A_MOUTH_PUCKER: usize = 20;
const A_MOUTH_LEFT: usize = 21;
const A_MOUTH_RIGHT: usize = 22;
const A_MOUTH_SMILE_L: usize = 23;
const A_MOUTH_SMILE_R: usize = 24;
const A_MOUTH_FROWN_L: usize = 25;
const A_MOUTH_FROWN_R: usize = 26;
const A_MOUTH_DIMPLE_L: usize = 27;
const A_MOUTH_DIMPLE_R: usize = 28;
const A_MOUTH_STRETCH_L: usize = 29;
const A_MOUTH_STRETCH_R: usize = 30;
const A_MOUTH_ROLL_LOWER: usize = 31;
const A_MOUTH_ROLL_UPPER: usize = 32;
const A_MOUTH_SHRUG_LOWER: usize = 33;
const A_MOUTH_SHRUG_UPPER: usize = 34;
const A_MOUTH_PRESS_L: usize = 35;
const A_MOUTH_PRESS_R: usize = 36;
const A_MOUTH_LOWER_DOWN_L: usize = 37;
const A_MOUTH_LOWER_DOWN_R: usize = 38;
const A_MOUTH_UPPER_UP_L: usize = 39;
const A_MOUTH_UPPER_UP_R: usize = 40;
const A_BROW_DOWN_L: usize = 41;
const A_BROW_DOWN_R: usize = 42;
const A_BROW_INNER_UP: usize = 43;
const A_BROW_OUTER_UP_L: usize = 44;
const A_BROW_OUTER_UP_R: usize = 45;
const A_CHEEK_PUFF: usize = 46;
const A_CHEEK_SQUINT_L: usize = 47;
const A_CHEEK_SQUINT_R: usize = 48;
const A_NOSE_SNEER_L: usize = 49;
const A_NOSE_SNEER_R: usize = 50;
const A_TONGUE_OUT: usize = 51;

fn g(a: &[f32], i: usize) -> f32 {
    a.get(i).copied().unwrap_or(0.0)
}

fn set(f: &mut VfUnifiedFrame, e: U, v: f32) {
    f.shapes[e.index()] = v;
}

/// Stateful bits matching Pico Updater (mouth L/R smoothing).
#[derive(Default)]
pub struct UeMapper {
    last_mouth_left: f32,
    last_mouth_right: f32,
}

impl UeMapper {
    pub fn map_pico(&mut self, pico: &[f32], ts_us: u64) -> VfUnifiedFrame {
        let arkit = pico_to_arkit(pico);
        self.map_arkit(&arkit, ts_us)
    }

    pub fn map_arkit(&mut self, arkit: &[f32], ts_us: u64) -> VfUnifiedFrame {
        let mut f = VfUnifiedFrame::default();
        f.timestamp_us = ts_us;
        f.valid = VF_VALID_EYE | VF_VALID_EXPR;

        f.eye.left.openness = 1.0 - g(arkit, A_EYE_BLINK_L);
        f.eye.left.gaze[0] = g(arkit, A_EYE_LOOK_IN_L) - g(arkit, A_EYE_LOOK_OUT_L);
        f.eye.left.gaze[1] = g(arkit, A_EYE_LOOK_UP_L) - g(arkit, A_EYE_LOOK_DOWN_L);
        f.eye.right.openness = 1.0 - g(arkit, A_EYE_BLINK_R);
        f.eye.right.gaze[0] = g(arkit, A_EYE_LOOK_OUT_R) - g(arkit, A_EYE_LOOK_IN_R);
        f.eye.right.gaze[1] = g(arkit, A_EYE_LOOK_UP_R) - g(arkit, A_EYE_LOOK_DOWN_R);

        set(&mut f, U::BrowInnerUpLeft, g(arkit, A_BROW_INNER_UP));
        set(&mut f, U::BrowInnerUpRight, g(arkit, A_BROW_INNER_UP));
        set(&mut f, U::BrowOuterUpLeft, g(arkit, A_BROW_OUTER_UP_L));
        set(&mut f, U::BrowOuterUpRight, g(arkit, A_BROW_OUTER_UP_R));
        set(&mut f, U::BrowLowererLeft, g(arkit, A_BROW_DOWN_L));
        set(&mut f, U::BrowPinchLeft, g(arkit, A_BROW_DOWN_L));
        set(&mut f, U::BrowLowererRight, g(arkit, A_BROW_DOWN_R));
        set(&mut f, U::BrowPinchRight, g(arkit, A_BROW_DOWN_R));
        set(&mut f, U::EyeSquintLeft, g(arkit, A_EYE_SQUINT_L));
        set(&mut f, U::EyeSquintRight, g(arkit, A_EYE_SQUINT_R));
        set(&mut f, U::EyeWideLeft, g(arkit, A_EYE_WIDE_L));
        set(&mut f, U::EyeWideRight, g(arkit, A_EYE_WIDE_R));

        set(&mut f, U::JawOpen, g(arkit, A_JAW_OPEN));
        set(&mut f, U::JawLeft, g(arkit, A_JAW_LEFT));
        set(&mut f, U::JawRight, g(arkit, A_JAW_RIGHT));
        set(&mut f, U::JawForward, g(arkit, A_JAW_FORWARD));
        set(&mut f, U::MouthClosed, g(arkit, A_MOUTH_CLOSE));
        set(&mut f, U::CheekSquintLeft, g(arkit, A_CHEEK_SQUINT_L));
        set(&mut f, U::CheekSquintRight, g(arkit, A_CHEEK_SQUINT_R));

        const SMOOTH: f32 = 0.5;
        self.last_mouth_left += (g(arkit, A_MOUTH_LEFT) - self.last_mouth_left) * SMOOTH;
        self.last_mouth_right += (g(arkit, A_MOUTH_RIGHT) - self.last_mouth_right) * SMOOTH;
        let cheek = g(arkit, A_CHEEK_PUFF);
        const DIFF: f32 = 0.1;
        if cheek > 0.1 {
            if self.last_mouth_left > self.last_mouth_right + DIFF {
                set(&mut f, U::CheekPuffLeft, cheek);
                set(&mut f, U::CheekPuffRight, cheek + self.last_mouth_left);
            } else if self.last_mouth_right > self.last_mouth_left + DIFF {
                set(&mut f, U::CheekPuffLeft, cheek + self.last_mouth_right);
                set(&mut f, U::CheekPuffRight, cheek);
            } else {
                set(&mut f, U::CheekPuffLeft, cheek);
                set(&mut f, U::CheekPuffRight, cheek);
            }
        } else {
            set(&mut f, U::CheekPuffLeft, cheek);
            set(&mut f, U::CheekPuffRight, cheek);
        }

        set(&mut f, U::NoseSneerLeft, g(arkit, A_NOSE_SNEER_L));
        set(&mut f, U::NoseSneerRight, g(arkit, A_NOSE_SNEER_R));
        set(&mut f, U::MouthUpperUpLeft, g(arkit, A_MOUTH_UPPER_UP_L));
        set(&mut f, U::MouthUpperUpRight, g(arkit, A_MOUTH_UPPER_UP_R));
        set(
            &mut f,
            U::MouthLowerDownLeft,
            g(arkit, A_MOUTH_LOWER_DOWN_L),
        );
        set(
            &mut f,
            U::MouthLowerDownRight,
            g(arkit, A_MOUTH_LOWER_DOWN_R),
        );

        let jaw = g(arkit, A_JAW_OPEN);
        let roll_l = g(arkit, A_MOUTH_ROLL_LOWER);
        let frown_l = g(arkit, A_MOUTH_FROWN_L);
        let frown_r = g(arkit, A_MOUTH_FROWN_R);
        set(
            &mut f,
            U::MouthFrownLeft,
            if jaw > 0.1 {
                frown_l / 2.0
            } else if roll_l > 0.2 {
                frown_l * 2.5 + roll_l
            } else {
                frown_l
            },
        );
        set(
            &mut f,
            U::MouthFrownRight,
            if jaw > 0.1 {
                frown_r / 2.0
            } else if roll_l > 0.2 {
                frown_r * 2.5 + roll_l
            } else {
                frown_r
            },
        );

        set(&mut f, U::MouthDimpleLeft, g(arkit, A_MOUTH_DIMPLE_L));
        set(&mut f, U::MouthDimpleRight, g(arkit, A_MOUTH_DIMPLE_R));
        set(&mut f, U::MouthUpperLeft, g(arkit, A_MOUTH_LEFT));
        set(&mut f, U::MouthLowerLeft, g(arkit, A_MOUTH_LEFT));
        set(&mut f, U::MouthUpperRight, g(arkit, A_MOUTH_RIGHT));
        set(&mut f, U::MouthLowerRight, g(arkit, A_MOUTH_RIGHT));
        set(&mut f, U::MouthPressLeft, g(arkit, A_MOUTH_PRESS_L));
        set(&mut f, U::MouthPressRight, g(arkit, A_MOUTH_PRESS_R));
        set(&mut f, U::MouthRaiserLower, g(arkit, A_MOUTH_SHRUG_LOWER));
        set(&mut f, U::MouthRaiserUpper, g(arkit, A_MOUTH_SHRUG_UPPER));

        let smile_l = g(arkit, A_MOUTH_SMILE_L) - roll_l;
        let smile_r = g(arkit, A_MOUTH_SMILE_R) - roll_l;
        if roll_l < 0.2 {
            set(&mut f, U::MouthCornerPullLeft, smile_l);
            set(&mut f, U::MouthCornerSlantLeft, smile_l - roll_l);
            set(&mut f, U::MouthCornerPullRight, smile_r);
            set(&mut f, U::MouthCornerSlantRight, smile_r);
        } else {
            set(&mut f, U::MouthCornerPullLeft, 0.0);
            set(&mut f, U::MouthCornerSlantLeft, 0.0);
            set(&mut f, U::MouthCornerPullRight, 0.0);
            set(&mut f, U::MouthCornerSlantRight, 0.0);
        }
        set(&mut f, U::MouthStretchLeft, g(arkit, A_MOUTH_STRETCH_L));
        set(&mut f, U::MouthStretchRight, g(arkit, A_MOUTH_STRETCH_R));

        let pucker = g(arkit, A_MOUTH_PUCKER);
        let funnel = g(arkit, A_MOUTH_FUNNEL);
        let is_funnel_l = pucker > 0.3 && g(arkit, A_MOUTH_PRESS_L) < 0.2;
        let is_funnel_r = pucker > 0.3 && g(arkit, A_MOUTH_PRESS_R) < 0.2;
        let fl = if is_funnel_l { pucker } else { funnel };
        let fr = if is_funnel_r { pucker } else { funnel };
        set(&mut f, U::LipFunnelUpperLeft, fl);
        set(&mut f, U::LipFunnelLowerLeft, fl);
        set(&mut f, U::LipFunnelUpperRight, fr);
        set(&mut f, U::LipFunnelLowerRight, fr);
        set(&mut f, U::LipPuckerUpperLeft, pucker);
        set(&mut f, U::LipPuckerUpperRight, pucker);
        set(&mut f, U::LipPuckerLowerLeft, pucker);
        set(&mut f, U::LipPuckerLowerRight, pucker);
        set(&mut f, U::LipSuckUpperLeft, g(arkit, A_MOUTH_ROLL_UPPER));
        set(&mut f, U::LipSuckUpperRight, g(arkit, A_MOUTH_ROLL_UPPER));
        set(&mut f, U::LipSuckLowerLeft, roll_l);
        set(&mut f, U::LipSuckLowerRight, roll_l);
        set(&mut f, U::TongueOut, g(arkit, A_TONGUE_OUT));
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pico_jaw_open_to_ue() {
        let mut pico = [0f32; 72];
        pico[7] = 0.8;
        let f = UeMapper::default().map_pico(&pico, 0);
        assert!((f.shape(U::JawOpen) - 0.8).abs() < 1e-6);
        assert!(f.valid & VF_VALID_EYE != 0);
    }

    #[test]
    fn arkit_blink_to_openness() {
        let mut a = [0f32; 52];
        a[A_JAW_OPEN] = 0.7;
        a[A_EYE_BLINK_L] = 0.2;
        a[A_EYE_BLINK_R] = 0.3;
        let f = UeMapper::default().map_arkit(&a, 0);
        assert!((f.shape(U::JawOpen) - 0.7).abs() < 1e-5);
        assert!((f.eye.left.openness - 0.8).abs() < 1e-5);
        assert!((f.eye.right.openness - 0.7).abs() < 1e-5);
    }
}
