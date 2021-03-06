mod parse;

pub use parse::{
    id_for_effect, parse_message, BlockFlags, BlockGridBlock, Effect, FractalMessage, FractalModel,
    Parameter, TunerStatus, XYState,
};

pub type MidiMessage = Vec<u8>;

pub fn guess_model(model_name: &str) -> Option<FractalModel> {
    match model_name {
        "Axe-Fx II" => Some(FractalModel::II),
        "Axe-Fx III" => Some(FractalModel::III),
        _ => None,
    }
}

fn model_code(model: FractalModel) -> u8 {
    match model {
        FractalModel::Standard => 0x00,
        FractalModel::Ultra => 0x01,
        FractalModel::MFC101 => 0x02,
        FractalModel::II => 0x03,
        FractalModel::MFC101MK3 => 0x04,
        FractalModel::FX8 => 0x05,
        FractalModel::IIXL => 0x06,
        FractalModel::IIXLPlus => 0x07,
        FractalModel::AX8 => 0x08,
        FractalModel::FX8MK2 => 0x0A,
        FractalModel::III => 0x10,
    }
}

pub fn checksum(msg: MidiMessage) -> u8 {
    let xord = msg
        .iter()
        .take(msg.len() - 1)
        .fold(None, |acc: Option<u8>, x| match acc {
            Some(y) => Some(y ^ x),
            None => Some(*x),
        })
        .unwrap();
    0x7F & xord
}

pub fn with_checksum(msg: MidiMessage) -> MidiMessage {
    let term = msg.iter().last().unwrap();
    let msg_checksum = checksum(msg.clone());
    let msg_without_term: MidiMessage = msg
        .clone()
        .into_iter()
        .take(msg.len() - 1)
        .collect::<Vec<u8>>();
    [msg_without_term, vec![msg_checksum, *term]].concat()
}

fn wrap_msg(msg: MidiMessage) -> MidiMessage {
    let header = vec![0xF0, 0x00, 0x01, 0x74];
    with_checksum([header, msg, vec![0xF7]].concat())
}

#[no_mangle]
pub extern "C" fn get_preset_number(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x14])
}

pub fn get_current_preset_name(model: FractalModel) -> MidiMessage {
    if model == FractalModel::III {
        wrap_msg(vec![model_code(model), 0x0D, 0x7F, 0x7F])
    } else {
        wrap_msg(vec![model_code(model), 0x0F])
    }
}

pub fn get_current_scene_name(model: FractalModel) -> MidiMessage {
    get_scene_name(model, 0x7F)
}

pub fn get_scene_name(model: FractalModel, scene: u8) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x0E, scene])
}

fn encode_preset_number(n: u32) -> (u8, u8) {
    ((n >> 7) as u8, (n & 0x7F) as u8)
}

pub fn set_preset_number(model: FractalModel, n: u32) -> MidiMessage {
    let (a, b) = encode_preset_number(n);
    wrap_msg(vec![model_code(model), 0x3C, a, b])
}

pub fn set_current_preset_name(model: FractalModel, name: &str) -> MidiMessage {
    match model {
        FractalModel::III => set_current_preset_name_v2(model, name),
        _ => set_current_preset_name_v1(model, name),
    }
}

pub fn set_current_preset_name_v1(model: FractalModel, name: &str) -> MidiMessage {
    let namesci: Vec<u8> = name
        .chars()
        .filter(|c| c.is_ascii())
        .map(|c| c as u8)
        .collect();
    let pad: Vec<u8> = (0..(32 - namesci.len())).map(|_| 32).collect();
    wrap_msg([vec![model_code(model), 0x09], namesci, pad].concat())
}

pub fn set_current_preset_name_v2(model: FractalModel, name: &str) -> MidiMessage {
    set_preset_name(model, 0, name)
}

fn encode_char_iii(i: u32, last: u8, x: u8) -> (u8, u8) {
    let i = i % 7;
    println!("i: {}, i % 8: {}", i, i % 8);
    (last | (x >> (i + 1)), 0x7F & (x << (8 - (i + 1) - 1)))
}

pub fn encode_preset_name_iii(name: &str) -> MidiMessage {
    println!("----------\nencode_preset_name_iii {}\n----------", name);
    let mut i = 0;
    name.chars()
        .filter(|c| c.is_ascii())
        .map(|c| c as u8)
        .collect::<Vec<u8>>()
        .chunks(7)
        .map(|chunk| {
            chunk.iter().fold(vec![0b00000000], |mut acc, x| {
                let (last, next) = encode_char_iii(i, *acc.last().unwrap(), *x);
                println!(
                    "last: {:b} ({}), x: {:b} {}, next: {:b} ({})",
                    last, last, x, x, next, next,
                );
                let len = acc.len();
                i = i + 1;
                acc[len - 1] = last;
                acc.push(next);
                acc
            })
        })
        .flatten()
        .collect::<MidiMessage>()
}

pub fn get_firmware_version(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x08])
}

pub fn disconnect_from_controller(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x42])
}

pub fn get_midi_channel(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x17])
}

pub fn toggle_tuner(midi_channel: u8, tuner_status: TunerStatus) -> MidiMessage {
    vec![
        176 + (midi_channel - 1),
        15,
        match tuner_status {
            TunerStatus::On => 127,
            TunerStatus::Off => 0,
        },
    ]
}

pub fn toggle_tuner_sysex(model: FractalModel, tuner_status: TunerStatus) -> MidiMessage {
    wrap_msg(vec![
        model_code(model),
        0x11,
        match tuner_status {
            TunerStatus::On => 1,
            TunerStatus::Off => 0,
        },
    ])
}

pub enum MetronomeStatus {
    On,
    Off,
}

pub fn toggle_metronome(midi_channel: u8, status: MetronomeStatus) -> MidiMessage {
    vec![
        176 + (midi_channel - 1),
        122,
        match status {
            MetronomeStatus::On => 127,
            MetronomeStatus::Off => 0,
        },
    ]
}

pub fn get_preset_blocks_flags(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x0E])
}

pub fn set_scene_number(model: FractalModel, scene_number: u8) -> MidiMessage {
    let command = if model == FractalModel::III {
        0x0C
    } else {
        0x29
    };
    wrap_msg(vec![model_code(model), command, scene_number])
}

pub fn get_grid_layout_and_routing(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x20])
}

fn encode_effect_id(id: u8) -> (u8, u8) {
    (id & 0x7F, (id >> 7) & 0x7F)
}

pub fn get_block_parameters(model: FractalModel, effect: Effect) -> MidiMessage {
    let (a, b) = encode_effect_id(id_for_effect(effect));
    wrap_msg(vec![model_code(model), 0x01, a, b])
}

pub fn store_in_preset(model: FractalModel, preset_number: u32) -> MidiMessage {
    let (a, b) = encode_preset_number(preset_number);
    if model == FractalModel::III {
        // 0xF0, 0x00, 0x01, 0x74, 0x10, 0x01, 0x26, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x03,
        // 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3E, 0xF7
        wrap_msg(vec![
            model_code(model),
            0x01,
            0x26,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            b,
            a,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
        ])
    } else {
        wrap_msg(vec![model_code(model), 0x1D, a, b])
    }
}

pub fn set_tempo(model: FractalModel, tempo: u32) -> MidiMessage {
    let (a, b) = encode_preset_number(tempo);
    wrap_msg(vec![model_code(model), 0x14, b, a])
}

pub fn set_preset_name(model: FractalModel, preset_number: u32, name: &str) -> MidiMessage {
    // a
    // 00  F0 00 01 74 10 01 28 00  00 00 00 00 05 03 00 00  |   t  (         |
    // 10  00 00 00 20 00 30 48 04  02 01 00 40 20 10 08 04  |     0H    @    |
    // 20  02 01 00 40 20 10 08 04  02 01 00 40 20 10 08 04  |   @       @    |
    // 30  02 01 00 40 20 10 08 04  02 00 64 F7              |   @      d |
    let (a, b) = encode_preset_number(preset_number);
    let name = encode_preset_name_iii(name);
    let pad: Vec<u8> = (0..(37 - name.len())).map(|_| 0).collect();
    wrap_msg(
        [
            vec![
                model_code(model),
                0x01,
                0x28,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                b,
                a,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x20,
                0x00,
            ],
            name,
            pad,
        ]
        .concat(),
    )
}

pub fn get_looper_state(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x0F, 0x7F])
}

pub enum LooperState {
    Record = 0,
    Play = 1,
    Undo = 2,
    Once = 3,
    Reverse = 4,
    HalfSpeed = 5,
}

pub fn set_looper_state(model: FractalModel, state: LooperState) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x0F, state as u8])
}

pub fn status_dump(model: FractalModel) -> MidiMessage {
    wrap_msg(vec![model_code(model), 0x13])
}

#[cfg(test)]
mod tests {
    use crate::parse::*;
    use crate::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_checksum() {
        assert_eq!(
            0x09,
            checksum(vec![0xF0, 0x00, 0x01, 0x74, 0x03, 0x0F, 0xF7])
        );
    }

    #[test]
    fn test_with_checksum() {
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x03, 0x0F, 0x09, 0xF7],
            with_checksum(vec![0xF0, 0x00, 0x01, 0x74, 0x03, 0x0F, 0xF7])
        );
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x03, 0x14, 18, 0xF7],
            with_checksum(vec![0xF0, 0x00, 0x01, 0x74, 0x03, 0x14, 0xF7])
        );
    }

    #[test]
    fn test_set_preset_name() {
        let cases = vec![
            (
                FractalModel::III,
                389,
                "a",
                vec![
                    0xF0, 0x00, 0x01, 0x74, 0x10, 0x01, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
                    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x30, 0x40, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 106, 0xF7,
                ],
            ),
            (
                FractalModel::III,
                390,
                "a",
                vec![
                    0xF0, 0x00, 0x01, 0x74, 0x10, 0x01, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06,
                    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x30, 0x40, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 105, 0xF7,
                ],
            ),
            (
                FractalModel::III,
                389,
                "Changed!",
                vec![
                    0xF0, 0x00, 0x01, 0x74, 0x10, 0x01, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
                    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x21, 0x5A, 0x0C, 0x16, 0x73,
                    0x1D, 0x4A, 0x64, 0x10, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 107, 0xF7,
                ],
            ),
        ];
        for (model, preset_number, name, expected) in cases {
            let msg = set_preset_name(model, preset_number, name);
            assert_eq!(msg.len(), expected.len());
            assert_eq!(msg, expected, "name: {}", name);
        }
    }

    #[test]
    fn test_encode_char_iii() {
        assert_eq!(
            (0b00110000, 0b01000000),
            encode_char_iii(0, 0b00000000, 0b01100001)
        );
        assert_eq!(
            (0b01011000, 0b00100000),
            encode_char_iii(1, 0b01000000, 0b01100001)
        );
        assert_eq!(
            (0b00101100, 0b00010000),
            encode_char_iii(2, 0b00100000, 0b01100001)
        );
        // assert_eq!((None, None, None), encode_char_iii(6, 0b01001010, None));
    }

    #[test]
    fn test_encode_preset_name_iii() {
        let cases = vec![
            (vec![0x10, 0x00], " "),
            (vec![0x10, 0x40], "!"),
            (vec![0x18, 0x00], "0"),
            (vec![0x18, 0x40], "1"),
            (vec![0x19, 0x00], "2"),
            (vec![0x1A, 0x00], "4"),
            (vec![0x3E, 0x00], "|"),
            (vec![0x30, 0x40], "a"),
            (vec![0x30, 0x58, 0x20], "aa"),
            (vec![0x30, 0x58, 0x2C, 0x10], "aaa"),
            (vec![0x30, 0x58, 0x2C, 0x16, 0x08], "aaaa"),
            (vec![0x30, 0x58, 0x2C, 0x16, 0x0B, 0x04], "aaaaa"),
            (vec![0x30, 0x58, 0x2C, 0x16, 0x0B, 0x05, 0x42], "aaaaaa"),
            (
                vec![0x30, 0x58, 0x2C, 0x16, 0x0B, 0x05, 0x42, 0x61],
                "aaaaaaa",
            ),
            (
                vec![0x21, 0x5A, 0x0C, 0x16, 0x73, 0x1D, 0x4A, 0x64, 0x10, 0x40],
                "Changed!",
            ),
        ];
        for (msg, name) in cases {
            assert_eq!(
                msg,
                encode_preset_name_iii(name),
                "encode name '{}', expected {}",
                name,
                msg.iter()
                    .map(|x| format!("{:b}", x))
                    .collect::<Vec<String>>()
                    .join(" ")
            );
        }
    }

    #[test]
    fn test_get_preset_number() {
        assert_eq!(
            vec![
                0xF0,
                0x00,
                0x01,
                0x74,
                model_code(FractalModel::II),
                0x14,
                18,
                0xF7
            ],
            get_preset_number(FractalModel::II)
        );
    }

    #[test]
    fn test_set_preset_number() {
        assert_eq!(
            vec![
                0xF0,
                0x00,
                0x01,
                0x74,
                model_code(FractalModel::II),
                0x3C,
                0,
                127,
                69,
                0xF7
            ],
            set_preset_number(FractalModel::II, 127)
        );
        assert_eq!(
            vec![
                0xF0,
                0x00,
                0x01,
                0x74,
                model_code(FractalModel::II),
                0x3C,
                1,
                0,
                59,
                0xF7
            ],
            set_preset_number(FractalModel::II, 128)
        );
    }

    #[test]
    fn test_parse_message_preset_number() {
        assert_eq!(
            FractalMessage::CurrentPresetNumber(235),
            parse_message(vec![240, 0, 1, 116, 3, 20, 1, 107, 120, 247])
        );
        assert_eq!(
            FractalMessage::CurrentPresetNumber(236),
            parse_message(vec![240, 0, 1, 116, 3, 20, 1, 108, 121, 247])
        );
    }

    #[test]
    fn test_get_current_preset_name() {
        assert_eq!(
            vec![
                0xF0,
                0x00,
                0x01,
                0x74,
                model_code(FractalModel::II),
                0x0F,
                9,
                0xF7
            ],
            get_current_preset_name(FractalModel::II)
        );
    }

    #[test]
    fn test_get_current_preset_name_axe_3() {
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x10, 0x0D, 0x7F, 0x7F, 24, 0xF7],
            get_current_preset_name(FractalModel::III)
        );
    }

    #[test]
    fn test_set_current_preset_name() {
        assert_eq!(
            vec![
                0xF0,
                0x00,
                0x01,
                0x74,
                model_code(FractalModel::II),
                0x09,
                0x43,
                0x68,
                0x61,
                0x6E,
                0x67,
                0x65,
                0x64,
                0x21,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x20,
                0x6C,
                0xF7
            ],
            set_current_preset_name(FractalModel::II, "Changed!")
        );
        assert_eq!(
            set_current_preset_name(FractalModel::II, "O Praise The Name (Anstasis)"),
            set_current_preset_name(FractalModel::II, "O Praise The Name (Anástasis)")
        );
    }

    #[test]
    fn test_parse_current_preset_name() {
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 15, 66, 83, 32, 65, 67, 50, 48, 32, 66, 97, 115, 101, 32, 32,
                32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 0, 13, 247
            ]),
            FractalMessage::CurrentPresetName("BS AC20 Base".to_string())
        );
    }

    #[test]
    fn test_parse_preset_name() {
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 16, 13, 15, 3, 66, 83, 32, 65, 67, 50, 48, 32, 66, 97, 115, 101,
                32, 83, 67, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 0, 0,
                247
            ]),
            FractalMessage::PresetName(399, "BS AC20 Base SC".to_string())
        );
    }

    #[test]
    fn test_parse_scene_name() {
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 16, 14, 0, 67, 108, 101, 97, 110, 32, 32, 32, 32, 32, 32, 32, 32,
                32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 0, 94, 247
            ]),
            FractalMessage::SceneName(0, "Clean".to_string())
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 16, 14, 1, 79, 68, 49, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
                32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 0, 32, 247
            ]),
            FractalMessage::SceneName(1, "OD1".to_string())
        );
    }

    #[test]
    fn test_get_firmware_version() {
        assert_eq!(
            vec![
                0xF0,
                0x00,
                0x01,
                0x74,
                model_code(FractalModel::II),
                0x08,
                14,
                0xF7
            ],
            get_firmware_version(FractalModel::II)
        );
    }

    #[test]
    fn test_parse_firmware_version() {
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 8, 0x08, 0x02, 0, 0, 0, 0, 0, 247]),
            FractalMessage::FirmwareVersion { major: 8, minor: 2 }
        );
    }

    #[test]
    fn test_disconnect_from_controller() {
        assert_eq!(
            vec![
                0xF0,
                0x00,
                0x01,
                0x74,
                model_code(FractalModel::II),
                0x42,
                68,
                0xF7
            ],
            disconnect_from_controller(FractalModel::II)
        );
    }

    #[test]
    fn test_parse_front_panel_change_detected() {
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 0x21, 0, 0xF7]),
            FractalMessage::FrontPanelChangeDetected
        );
    }

    #[test]
    fn test_parse_midi_tempo_beat() {
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 0x10, 0xF7]),
            FractalMessage::MIDITempoBeat
        );
    }

    #[test]
    fn test_get_midi_channel() {
        assert_eq!(
            vec![240, 0, 1, 116, 3, 0x17, 17, 0xF7],
            get_midi_channel(FractalModel::II)
        );
    }

    #[test]
    fn test_parse_midi_channel() {
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 0x17, 9, 0, 0xF7]),
            FractalMessage::MIDIChannel(10)
        );
    }

    #[test]
    fn test_parse_tuner_info() {
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 0x0D, 1, 2, 63, 0xF7]),
            FractalMessage::TunerInfo {
                note: 1,
                string_number: 2,
                tuner_data: 63
            }
        );
    }

    #[test]
    fn test_toggle_tuner() {
        assert_eq!(vec![176, 15, 0], toggle_tuner(1, TunerStatus::Off));
        assert_eq!(vec![177, 15, 0], toggle_tuner(2, TunerStatus::Off));
        assert_eq!(vec![176, 15, 127], toggle_tuner(1, TunerStatus::On));
    }

    #[test]
    fn test_toggle_metronome() {
        assert_eq!(vec![176, 122, 0], toggle_metronome(1, MetronomeStatus::Off));
        assert_eq!(vec![177, 122, 0], toggle_metronome(2, MetronomeStatus::Off));
        assert_eq!(
            vec![176, 122, 127],
            toggle_metronome(1, MetronomeStatus::On)
        );
    }

    #[test]
    fn test_get_preset_blocks_flags() {
        assert_eq!(
            vec![240, 0, 1, 116, 3, 0x0E, 8, 0xF7],
            get_preset_blocks_flags(FractalModel::II)
        );
    }

    #[test]
    fn test_parse_preset_blocks_flags() {
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 14, 3, 74, 16, 83, 6, 3, 78, 24, 99, 6, 2, 86, 124, 39, 6, 3,
                94, 40, 3, 7, 2, 98, 48, 43, 120, 2, 100, 52, 51, 120, 3, 102, 124, 63, 120, 2, 10,
                125, 23, 7, 3, 38, 81, 115, 6, 2, 52, 125, 7, 120, 3, 58, 125, 127, 7, 247
            ]),
            FractalMessage::PresetBlocksFlags(vec![
                BlockFlags {
                    is_bypassed: false,
                    xy_state: XYState::X,
                    cc: 37,
                    effect_id: 106,
                    effect: Effect::Amp1
                },
                BlockFlags {
                    is_bypassed: false,
                    xy_state: XYState::X,
                    cc: 39,
                    effect_id: 108,
                    effect: Effect::Cab1
                },
                BlockFlags {
                    is_bypassed: true,
                    xy_state: XYState::X,
                    cc: 43,
                    effect_id: 100,
                    effect: Effect::Compressor1
                },
                BlockFlags {
                    is_bypassed: false,
                    xy_state: XYState::X,
                    cc: 47,
                    effect_id: 112,
                    effect: Effect::Delay1
                },
                BlockFlags {
                    is_bypassed: true,
                    xy_state: XYState::X,
                    cc: 49,
                    effect_id: 133,
                    effect: Effect::Drive1
                },
                BlockFlags {
                    is_bypassed: true,
                    xy_state: XYState::X,
                    cc: 50,
                    effect_id: 134,
                    effect: Effect::Drive2
                },
                BlockFlags {
                    is_bypassed: false,
                    xy_state: XYState::X,
                    cc: 51,
                    effect_id: 135,
                    effect: Effect::Enhancer1
                },
                BlockFlags {
                    is_bypassed: true,
                    xy_state: XYState::X,
                    cc: 69,
                    effect_id: 114,
                    effect: Effect::MultiDelay1
                },
                BlockFlags {
                    is_bypassed: false,
                    xy_state: XYState::X,
                    cc: 83,
                    effect_id: 110,
                    effect: Effect::Reverb1
                },
                BlockFlags {
                    is_bypassed: true,
                    xy_state: XYState::X,
                    cc: 90,
                    effect_id: 128,
                    effect: Effect::TremoloPanner1
                },
                BlockFlags {
                    is_bypassed: false,
                    xy_state: XYState::X,
                    cc: 93,
                    effect_id: 127,
                    effect: Effect::VolumePan1
                },
            ])
        );
    }

    #[test]
    fn test_set_scene_number() {
        assert_eq!(
            vec![240, 0, 1, 116, 3, 41, 0, 47, 247],
            set_scene_number(FractalModel::II, 0)
        );
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x03, 0x29, 0x00, 0x2F, 0xF7],
            set_scene_number(FractalModel::II, 0)
        );
    }

    #[test]
    fn test_set_scene_number_axe_3() {
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x10, 0x0C, 0x01, 0x18, 0xF7],
            set_scene_number(FractalModel::III, 1)
        );
    }

    #[test]
    fn test_get_grid_layout_and_routing() {
        assert_eq!(
            vec![240, 0, 1, 116, 3, 0x20, 38, 0xF7],
            get_grid_layout_and_routing(FractalModel::II)
        );
    }

    #[test]
    fn test_parse_scene_number() {
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 41, 0, 47, 247]),
            FractalMessage::CurrentSceneNumber(1)
        );
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 41, 1, 47, 247]),
            FractalMessage::CurrentSceneNumber(2)
        );
        assert_eq!(
            parse_message(vec![240, 0, 1, 116, 3, 41, 7, 47, 247]),
            FractalMessage::CurrentSceneNumber(8)
        );
        assert_eq!(
            parse_message(vec![0xF0, 0x00, 0x01, 0x74, 0x10, 0x0C, 0x01, 0x18, 0xF7,]),
            FractalMessage::CurrentSceneNumber(1)
        )
    }

    #[test]
    fn test_parse_grid_layout_and_routing() {
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 32, 0, 0, 0, 0, 127, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 100, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 5, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 1, 2, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 106, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                108, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 79, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 112, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 114, 0, 2, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 110, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                7, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 29, 247
            ]),
            FractalMessage::BlockGrid([
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 127,
                        effect: Effect::VolumePan1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 100,
                        effect: Effect::Compressor1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 128,
                        effect: Effect::TremoloPanner1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 133,
                        effect: Effect::Drive1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 134,
                        effect: Effect::Drive2,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 106,
                        effect: Effect::Amp1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 108,
                        effect: Effect::Cab1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 207,
                        effect: Effect::Shunt,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 112,
                        effect: Effect::Delay1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 114,
                        effect: Effect::MultiDelay1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 110,
                        effect: Effect::Reverb1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::EffectBlock {
                        effect_id: 135,
                        effect: Effect::Enhancer1,
                        connect_row_1: false,
                        connect_row_2: true,
                        connect_row_3: false,
                        connect_row_4: false,
                    },
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
                [
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                    BlockGridBlock::Empty,
                ],
            ])
        );
    }

    #[test]
    fn test_get_block_parameters() {
        assert_eq!(
            vec![240, 0, 1, 116, 3, 0x01, 0, 1, 6, 0xF7],
            get_block_parameters(FractalModel::II, Effect::TremoloPanner1)
        );
        assert_eq!(
            vec![240, 0, 1, 116, 3, 0x01, 127, 0, 120, 0xF7],
            get_block_parameters(FractalModel::II, Effect::VolumePan1)
        );
        assert_eq!(
            vec![240, 0, 1, 116, 3, 0x01, 1, 1, 7, 0xF7],
            get_block_parameters(FractalModel::II, Effect::TremoloPanner2)
        );
    }

    #[test]
    fn test_parse_block_parameters() {
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 0, 0, 109, 1, 0, 116, 0, 0, 2, 2, 65, 67, 45, 50, 48,
                32, 49, 50, 65, 88, 55, 32, 66, 0, 55, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 0,
                parameter: Parameter::EffectType,
                value_raw: 237
            }
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 1, 0, 75, 104, 0, 25, 0, 0, 0, 0, 50, 46, 48, 52, 0,
                78, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 1,
                parameter: Parameter::InputDrive,
                value_raw: 13387
            }
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 2, 0, 110, 70, 1, 49, 0, 0, 0, 0, 51, 46, 56, 56, 0,
                106, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 2,
                parameter: Parameter::Bass,
                value_raw: 25454
            }
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 3, 0, 25, 29, 3, 102, 0, 0, 0, 0, 56, 46, 48, 55, 0,
                30, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 3,
                parameter: Parameter::Middle,
                value_raw: 52889
            }
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 4, 0, 24, 51, 2, 76, 0, 0, 0, 0, 54, 46, 48, 48, 0,
                20, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 4,
                parameter: Parameter::Treble,
                value_raw: 39320
            }
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 5, 0, 126, 127, 3, 127, 0, 0, 0, 0, 49, 48, 46, 48,
                48, 0, 58, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 5,
                parameter: Parameter::MasterVolume,
                value_raw: 65534
            }
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 49, 48, 46, 48, 32, 72,
                122, 0, 102, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 6,
                parameter: Parameter::PreampLowCut,
                value_raw: 0
            }
        );
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 3, 1, 106, 0, 7, 0, 118, 50, 3, 107, 0, 0, 0, 0, 50, 48, 48, 48,
                48, 32, 72, 122, 0, 102, 247
            ]),
            FractalMessage::BlockParameters {
                effect_id: 106,
                effect: Effect::Amp1,
                parameter_id: 7,
                parameter: Parameter::HighCutFrequency,
                value_raw: 55670
            }
        );
    }

    #[test]
    fn test_store_in_preset() {
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x03, 0x1D, 0x01, 0x59, 0x43, 0xF7],
            store_in_preset(FractalModel::II, 217)
        );
        assert_eq!(
            vec![
                0xF0, 0x00, 0x01, 0x74, 0x10, 0x01, 0x26, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x03,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3E, 0xF7
            ],
            store_in_preset(FractalModel::III, 399)
        );
    }

    #[test]
    fn test_set_tempo() {
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x10, 0x14, 0x46, 0x00, 0x47, 0xF7],
            set_tempo(FractalModel::III, 70)
        );
        assert_eq!(
            vec![0xF0, 0x00, 0x01, 0x74, 0x10, 0x14, 0x0C, 0x01, 0x0C, 0xF7],
            set_tempo(FractalModel::III, 140)
        );
    }

    #[test]
    fn test_parse_status_dump() {
        assert_eq!(
            parse_message(vec![
                240, 0, 1, 116, 16, 19, 58, 0, 64, 62, 0, 64, 46, 0, 64, 70, 0, 64, 122, 0, 64, 58,
                1, 16, 54, 1, 16, 118, 0, 66, 119, 0, 65, 37, 0, 64, 126, 0, 64, 42, 0, 64, 110, 0,
                65, 50, 1, 64, 66, 0, 64, 67, 0, 64, 106, 0, 65, 102, 0, 64, 38, 1, 16, 73, 1, 16,
                72, 1, 16, 9, 247,
            ]),
            FractalMessage::StatusDump(vec![
                EffectStatus {
                    effect_id: 58,
                    effect_id_iii: Some(EffectID::ID_DISTORT1),
                    effect: Effect::Amp1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 62,
                    effect_id_iii: Some(EffectID::ID_CAB1),
                    effect: Effect::Cab1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 46,
                    effect_id_iii: Some(EffectID::ID_COMP1),
                    effect: Effect::Compressor1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 70,
                    effect_id_iii: Some(EffectID::ID_DELAY1),
                    effect: Effect::Delay1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 122,
                    effect_id_iii: Some(EffectID::ID_ENHANCER1),
                    effect: Effect::Enhancer1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 186,
                    effect_id_iii: Some(EffectID::ID_FBRETURN1),
                    effect: Effect::FeedbackReturn1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 1
                },
                EffectStatus {
                    effect_id: 182,
                    effect_id_iii: Some(EffectID::ID_FBSEND1),
                    effect: Effect::FeedbackSend1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 1
                },
                EffectStatus {
                    effect_id: 118,
                    effect_id_iii: Some(EffectID::ID_FUZZ1),
                    effect: Effect::Drive1,
                    bypassed: false,
                    channel: Channel::B,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 119,
                    effect_id_iii: Some(EffectID::ID_FUZZ2),
                    effect: Effect::Drive2,
                    bypassed: true,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 37,
                    effect_id_iii: Some(EffectID::ID_INPUT1),
                    effect: Effect::Input1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 126,
                    effect_id_iii: Some(EffectID::ID_MIXER1),
                    effect: Effect::Mixer1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 42,
                    effect_id_iii: Some(EffectID::ID_OUTPUT1),
                    effect: Effect::Output1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 110,
                    effect_id_iii: Some(EffectID::ID_PITCH1),
                    effect: Effect::Pitch1,
                    bypassed: true,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 178,
                    effect_id_iii: Some(EffectID::ID_PLEX1),
                    effect: Effect::PlexDelay1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 66,
                    effect_id_iii: Some(EffectID::ID_REVERB1),
                    effect: Effect::Reverb1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 67,
                    effect_id_iii: Some(EffectID::ID_REVERB2),
                    effect: Effect::Reverb2,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 106,
                    effect_id_iii: Some(EffectID::ID_TREMOLO1),
                    effect: Effect::TremoloPanner1,
                    bypassed: true,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 102,
                    effect_id_iii: Some(EffectID::ID_VOLUME1),
                    effect: Effect::VolumePan1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 4
                },
                EffectStatus {
                    effect_id: 166,
                    effect_id_iii: Some(EffectID::ID_LOOPER1),
                    effect: Effect::Looper1,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 1
                },
                EffectStatus {
                    effect_id: 201,
                    effect_id_iii: None,
                    effect: Effect::Unknown,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 1
                },
                EffectStatus {
                    effect_id: 200,
                    effect_id_iii: Some(EffectID::ID_PRESET_FC),
                    effect: Effect::PresetFC,
                    bypassed: false,
                    channel: Channel::A,
                    max_channels: 1
                }
            ]),
        );
    }
}

#[cfg(target_os = "macos")]
#[cfg(test)]
mod test_integration {
    use crate::*;
    use coremidi;

    struct TestOutput {
        #[allow(dead_code)]
        client: coremidi::Client,
        output_port: coremidi::OutputPort,
        destination: coremidi::Destination,
    }

    impl TestOutput {
        fn new(destination: coremidi::Destination) -> Self {
            let client = coremidi::Client::new("example-client").unwrap();
            let output_port = client.output_port("example-port").unwrap();
            TestOutput {
                client,
                destination,
                output_port,
            }
        }

        fn send(&self, msg: &MidiMessage) {
            let packet_buffer = coremidi::PacketBuffer::new(0, &msg[..] as &[u8]);
            self.output_port
                .send(&self.destination, &packet_buffer)
                .unwrap();
        }

        fn send_and_wait(&self, msg: &MidiMessage) {
            use std::thread;
            use std::time::Duration;
            self.send(msg);
            thread::sleep(Duration::from_millis(300));
        }
    }

    #[test]
    fn test_integration() {
        use std::sync::{Arc, Mutex};
        let client = coremidi::Client::new("example-client").unwrap();

        for destination in coremidi::Destinations {
            match &destination.display_name() {
                Some(name) => match guess_model(name) {
                    Some(model) => {
                        println!(
                            "Testing midi output '{}', inferred axe model, '{:?}'",
                            name, model
                        );

                        // setup midi input listener
                        let maybe_source = coremidi::Source::from_index(0);
                        if maybe_source.is_none() {
                            println!("No midi inputs! skipping integration test");
                            return;
                        }
                        let source = maybe_source.unwrap();
                        let mut currently_receiving_msg = vec![];
                        let received_messages: Arc<Mutex<Vec<MidiMessage>>> =
                            Arc::new(Mutex::new(vec![]));
                        let received_messages_writer = Arc::clone(&received_messages);
                        let callback = move |packet_list: &coremidi::PacketList| {
                            // println!("Received Packet: {}", packet_list);
                            for packet in packet_list.iter() {
                                for byte in packet.data() {
                                    currently_receiving_msg.push(byte.clone());
                                    if *byte == 0xF7 as u8 {
                                        // sysex message end, flush and parse
                                        received_messages_writer
                                            .lock()
                                            .unwrap()
                                            .push(currently_receiving_msg.clone());
                                        currently_receiving_msg = vec![];
                                    }
                                }
                            }
                        };
                        let input_port = client.input_port("example-port", callback).unwrap();
                        input_port.connect_source(&source).unwrap();
                        // end midi input setup

                        let output = TestOutput::new(destination);

                        println!("Getting current preset name...");
                        output.send_and_wait(&get_current_preset_name(model));
                        println!("Sending tuner on...");
                        output.send_and_wait(&toggle_tuner_sysex(model, TunerStatus::On));
                        println!("Sending tuner off...");
                        output.send_and_wait(&toggle_tuner_sysex(model, TunerStatus::Off));
                        for x in [7, 6, 5, 4, 3, 2, 1, 0].iter() {
                            println!("Setting scene to {}...", x + 1);
                            output.send_and_wait(&set_scene_number(model, *x));
                        }
                        use rand::Rng;
                        let tempo = rand::thread_rng().gen_range(60, 160);
                        println!("Setting tempo to {}.", tempo);
                        output.send(&set_tempo(model, tempo));
                        println!("Setting new preset name.");
                        output.send_and_wait(&set_current_preset_name(
                            model,
                            &format!("Changed from Rust! {}", tempo),
                        ));
                        // output.send_and_wait(&set_preset_name(model, 389, "Changed from Rust!"));
                        println!("Trying to store in preset 389");
                        output.send_and_wait(&store_in_preset(model, 389));

                        input_port.disconnect_source(&source).unwrap();

                        println!(
                            "Received {} messages",
                            received_messages.lock().unwrap().len()
                        );
                        println!("{:?}", received_messages.lock().unwrap());
                        let parsed_messages = received_messages
                            .lock()
                            .unwrap()
                            .iter()
                            .map(|msg| parse_message(msg.clone()))
                            .collect::<Vec<FractalMessage>>();
                        println!("{:?}", parsed_messages);
                        for x in [7, 6, 5, 4, 3, 2, 1, 0].iter() {
                            assert!(
                                parsed_messages.contains(&FractalMessage::CurrentSceneNumber(*x)),
                            );
                        }
                    }
                    None => {
                        println!("Skipping output '{}', could not guess axe model", name);
                    }
                },
                None => {
                    println!("Skipping midi output, no port name");
                }
            }
        }
    }
}
