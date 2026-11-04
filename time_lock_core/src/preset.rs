use crate::{clock::Mode, date::DateTime};
use core::{fmt, str};

pub const MAX_PRESET_BYTES: usize = 512;
pub const TITLES: [(u64, &str, u16); 8] = [
    (0x0004000000055D00, "X", 5),
    (0x0004000000055E00, "Y", 5),
    (0x000400000011C400, "Omega Ruby", 7),
    (0x000400000011C500, "Alpha Sapphire", 7),
    (0x0004000000164800, "Sun", 2),
    (0x0004000000175E00, "Moon", 2),
    (0x00040000001B5000, "Ultra Sun", 2),
    (0x00040000001B5100, "Ultra Moon", 2),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetError {
    Size,
    Utf8,
    Syntax,
    UnknownField,
    DuplicateField,
    MissingField,
    Schema,
    Title,
    Date,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub title_id: u64,
    pub target: DateTime,
    pub initial_mode: Mode,
    pub reuse: bool,
    pub enabled: bool,
}

impl Preset {
    pub fn new(title_id: u64) -> Self {
        Self {
            title_id,
            target: DateTime::EPOCH,
            initial_mode: Mode::Frozen,
            reuse: true,
            enabled: false,
        }
    }

    pub fn validate(&self) -> Result<(), PresetError> {
        if !TITLES.iter().any(|(id, _, _)| *id == self.title_id) {
            return Err(PresetError::Title);
        }
        self.target.validate().map_err(|_| PresetError::Date)?;
        if self.initial_mode == Mode::Disabled {
            return Err(PresetError::Value);
        }
        Ok(())
    }

    pub fn parse(bytes: &[u8], expected_title: u64) -> Result<Self, PresetError> {
        if bytes.len() > MAX_PRESET_BYTES {
            return Err(PresetError::Size);
        }
        let text = str::from_utf8(bytes).map_err(|_| PresetError::Utf8)?;
        let keys = [
            "schema_version",
            "title_id",
            "target",
            "initial_mode",
            "reuse",
            "enabled",
            "penalty_policy",
        ];
        let mut seen = 0u8;
        let mut preset = Self::new(expected_title);
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or(PresetError::Syntax)?;
            let index = keys
                .iter()
                .position(|k| *k == key.trim())
                .ok_or(PresetError::UnknownField)?;
            let bit = 1 << index;
            if seen & bit != 0 {
                return Err(PresetError::DuplicateField);
            }
            seen |= bit;
            let value = value.trim();
            match index {
                0 if value != "1" => return Err(PresetError::Schema),
                0 => {}
                1 => {
                    if value.len() != 16 {
                        return Err(PresetError::Title);
                    }
                    preset.title_id =
                        u64::from_str_radix(value, 16).map_err(|_| PresetError::Title)?;
                    if preset.title_id != expected_title {
                        return Err(PresetError::Title);
                    }
                }
                2 => preset.target = DateTime::parse(value).map_err(|_| PresetError::Date)?,
                3 => {
                    preset.initial_mode = match value {
                        "frozen" => Mode::Frozen,
                        "running" => Mode::Running,
                        _ => return Err(PresetError::Value),
                    }
                }
                4 => preset.reuse = parse_bool(value)?,
                5 => preset.enabled = parse_bool(value)?,
                6 if value != "observe" => return Err(PresetError::Value),
                _ => {}
            }
        }
        if seen != 0x7f {
            return Err(PresetError::MissingField);
        }
        preset.validate()?;
        Ok(preset)
    }
}

fn parse_bool(value: &str) -> Result<bool, PresetError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(PresetError::Value),
    }
}

impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "schema_version=1\ntitle_id={:016X}\ntarget={}\ninitial_mode={}\nreuse={}\nenabled={}\npenalty_policy=observe\n",
            self.title_id, self.target, if self.initial_mode == Mode::Running { "running" } else { "frozen" },
            self.reuse, self.enabled)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::ToString;

    #[test]
    fn round_trip_all_titles() {
        for (id, _, _) in TITLES {
            let mut p = Preset::new(id);
            p.target = DateTime::parse("2024-02-29T23:59:59.999").unwrap();
            p.enabled = true;
            assert_eq!(Preset::parse(p.to_string().as_bytes(), id), Ok(p));
            assert_eq!(
                Preset::parse(p.to_string().replace('\n', "\r\n").as_bytes(), id),
                Ok(p)
            );
        }
    }

    #[test]
    fn malformed_or_foreign_presets_cannot_enable_a_clock() {
        let id = TITLES[0].0;
        let text = Preset::new(id).to_string();
        for invalid in [
            text.replace("schema_version=1", "schema_version=2"),
            text.replace("enabled=false", "enabled=yes"),
            text.replace("penalty_policy=observe", "penalty_policy=clear"),
            text.replace("initial_mode=frozen", "initial_mode=disabled"),
            text.replace("target=", "other="),
            text.replace("2000-01-01", "2000-02-30"),
            text.replace("reuse=true\n", ""),
            text.clone() + "enabled=true\n",
            text.clone() + "typo=true\n",
        ] {
            assert!(Preset::parse(invalid.as_bytes(), id).is_err(), "{invalid}");
        }
        assert_eq!(
            Preset::parse(text.as_bytes(), TITLES[1].0),
            Err(PresetError::Title)
        );
        assert_eq!(Preset::parse(&[0xff], id), Err(PresetError::Utf8));
        assert_eq!(Preset::parse(&[b' '; 513], id), Err(PresetError::Size));
    }
}
