//! PICO native blendshape order → ARKit 52.

const ARKIT_COUNT: usize = 52;

/// Maps `BlendShape.Index` (PICO) to ARKit 52 index.
/// Source: VRCFTPicoModule `BlendShapeIndex.cs` vs Apple ARKit order.
pub const PICO_TO_ARKIT: [Option<usize>; 52] = [
    Some(1),  // 0 EyeLookDown_L -> eyeLookDownLeft
    Some(49), // 1 NoseSneer_L
    Some(2),  // 2 EyeLookIn_L
    Some(43), // 3 BrowInnerUp
    Some(42), // 4 BrowDown_R
    Some(18), // 5 MouthClose
    Some(38), // 6 MouthLowerDown_R
    Some(17), // 7 JawOpen
    Some(40), // 8 MouthUpperUp_R
    Some(34), // 9 MouthShrugUpper
    Some(19), // 10 MouthFunnel
    Some(9),  // 11 EyeLookIn_R
    Some(8),  // 12 EyeLookDown_R
    Some(50), // 13 NoseSneer_R
    Some(32), // 14 MouthRollUpper
    Some(16), // 15 JawRight
    Some(41), // 16 BrowDown_L
    Some(33), // 17 MouthShrugLower
    Some(31), // 18 MouthRollLower
    Some(23), // 19 MouthSmile_L
    Some(35), // 20 MouthPress_L
    Some(24), // 21 MouthSmile_R
    Some(36), // 22 MouthPress_R
    Some(28), // 23 MouthDimple_R
    Some(21), // 24 MouthLeft
    Some(14), // 25 JawForward
    Some(5),  // 26 EyeSquint_L
    Some(25), // 27 MouthFrown_L
    Some(0),  // 28 EyeBlink_L
    Some(47), // 29 CheekSquint_L
    Some(44), // 30 BrowOuterUp_L
    Some(4),  // 31 EyeLookUp_L
    Some(15), // 32 JawLeft
    Some(29), // 33 MouthStretch_L
    Some(20), // 34 MouthPucker
    Some(11), // 35 EyeLookUp_R
    Some(45), // 36 BrowOuterUp_R
    Some(48), // 37 CheekSquint_R
    Some(7),  // 38 EyeBlink_R
    Some(39), // 39 MouthUpperUp_L
    Some(26), // 40 MouthFrown_R
    Some(12), // 41 EyeSquint_R
    Some(30), // 42 MouthStretch_R
    Some(46), // 43 CheekPuff
    Some(3),  // 44 EyeLookOut_L
    Some(10), // 45 EyeLookOut_R
    Some(13), // 46 EyeWide_R
    Some(6),  // 47 EyeWide_L
    Some(22), // 48 MouthRight
    Some(27), // 49 MouthDimple_L
    Some(37), // 50 MouthLowerDown_L
    Some(51), // 51 TongueOut
];

pub(crate) fn pico_to_arkit(pico: &[f32]) -> [f32; ARKIT_COUNT] {
    let mut out = [0f32; ARKIT_COUNT];
    for (pico_i, dst) in PICO_TO_ARKIT.iter().enumerate() {
        if let Some(ai) = *dst
            && let Some(v) = pico.get(pico_i)
        {
            out[ai] = *v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARKIT_NAMES: [&str; 52] = [
        "eyeBlinkLeft",
        "eyeLookDownLeft",
        "eyeLookInLeft",
        "eyeLookOutLeft",
        "eyeLookUpLeft",
        "eyeSquintLeft",
        "eyeWideLeft",
        "eyeBlinkRight",
        "eyeLookDownRight",
        "eyeLookInRight",
        "eyeLookOutRight",
        "eyeLookUpRight",
        "eyeSquintRight",
        "eyeWideRight",
        "jawForward",
        "jawLeft",
        "jawRight",
        "jawOpen",
        "mouthClose",
        "mouthFunnel",
        "mouthPucker",
        "mouthLeft",
        "mouthRight",
        "mouthSmileLeft",
        "mouthSmileRight",
        "mouthFrownLeft",
        "mouthFrownRight",
        "mouthDimpleLeft",
        "mouthDimpleRight",
        "mouthStretchLeft",
        "mouthStretchRight",
        "mouthRollLower",
        "mouthRollUpper",
        "mouthShrugLower",
        "mouthShrugUpper",
        "mouthPressLeft",
        "mouthPressRight",
        "mouthLowerDownLeft",
        "mouthLowerDownRight",
        "mouthUpperUpLeft",
        "mouthUpperUpRight",
        "browDownLeft",
        "browDownRight",
        "browInnerUp",
        "browOuterUpLeft",
        "browOuterUpRight",
        "cheekPuff",
        "cheekSquintLeft",
        "cheekSquintRight",
        "noseSneerLeft",
        "noseSneerRight",
        "tongueOut",
    ];

    #[test]
    fn jaw_open_lands_on_arkit_17() {
        let mut pico = [0f32; 72];
        pico[7] = 0.8;
        let arkit = pico_to_arkit(&pico);
        assert!((arkit[17] - 0.8).abs() < 1e-6);
        assert_eq!(ARKIT_NAMES[17], "jawOpen");
    }
}
