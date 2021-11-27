use crate::bundler::{loader::utils::{ends_with, collect_paths}, game_data::{file_types::{darkest_parser, DarkestEntry}, BTreeMappable, BTreePatchable, Loadable, BTreeMapExt, BTreeSetable}, diff::DataMap};
use std::{
    collections::{HashMap},
    convert::TryInto,
};
use combine::EasyParser;

#[derive(Clone, Debug)]
pub struct HeroInfo {
    id: String,
    resistances: Resistances,
    weapons: Weapons,
    armours: Armours,
    skills: Skills,
    riposte_skill: Skill,
    move_skill: MoveSkill,
    tags: Vec<String>,
    extra_stack_limit: Vec<String>,
    deaths_door: DeathsDoor,
    modes: Modes,
    other: HashMap<(String, String), Vec<String>>,
}

impl BTreeMappable for HeroInfo {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        let mut inner = DataMap::new();

        inner.extend_prefixed("resistances", self.resistances.to_map());
        inner.extend_prefixed("weapons", self.weapons.to_map());
        inner.extend_prefixed("armours", self.armours.to_map());
        inner.extend_prefixed("skills", self.skills.to_map());
        inner.extend_prefixed("riposte_skill", self.riposte_skill.to_map());
        inner.extend_prefixed("move_skill", self.move_skill.to_map());
        inner.extend_prefixed("tags", self.tags.to_set());
        inner.extend_prefixed("extra_stack_limit", self.extra_stack_limit.to_set());
        inner.extend_prefixed("deaths_door", self.deaths_door.to_map());
        inner.extend_prefixed("modes", self.modes.to_map());
        for (key, value) in &self.other {
            let mut intermid = DataMap::new();
            intermid.extend_prefixed(&key.1, value.to_set());
            let mut intermid_outer = DataMap::new();
            intermid_outer.extend_prefixed(&key.0, intermid);
            inner.extend_prefixed("other", intermid_outer);
        }

        out.extend_prefixed(&self.id, inner);
        out
    }
}

impl BTreePatchable for HeroInfo {
    fn merge_patches(
        &self,
        patches: impl IntoIterator<Item = crate::bundler::ModFileChange>,
    ) -> (
        crate::bundler::diff::Patch,
        Vec<crate::bundler::ModFileChange>,
    ) {
        todo!()
    }
    fn apply_patch(&mut self, patch: crate::bundler::diff::Patch) -> Result<(), ()> {
        todo!()
    }
}

impl Loadable for HeroInfo {
    fn prepare_list(root_path: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
        collect_paths(
            &root_path.join("heroes"),
            |path| Ok(ends_with(path, ".info.darkest")),
        )
    }
    fn load_raw(path: &std::path::Path) -> std::io::Result<Self> {
        let id = path.file_name().unwrap().to_string_lossy().split('.').next().unwrap().to_string();

        let darkest_file = std::fs::read_to_string(path)?;
        let (darkest_file, rest) = darkest_parser().easy_parse(darkest_file.as_str()).unwrap();
        debug_assert_eq!(rest, "");

        // OK, now let's get these parts out...
        let mut resistances = None;
        let mut weapons = vec![];
        let mut armours = vec![];
        let mut skills = vec![];
        let mut riposte_skill = vec![];
        let mut move_skill = None;
        let mut tags = vec![];
        let mut extra_stack_limit = vec![];
        let mut deaths_door = None;
        let mut modes = vec![];
        let mut other = HashMap::new();

        for (key, entry) in darkest_file {
            match key.as_str() {
                "resistances" => { 
                    let existing = resistances.replace(entry);
                    debug_assert!(existing.is_none());
                }
                "weapon" => weapons.push(entry),
                "armour" => armours.push(entry),
                "combat_skill" => skills.push(entry),
                "riposte_skill" => riposte_skill.push(entry),
                "combat_move_skill" => { 
                    let existing = move_skill.replace(entry);
                    debug_assert!(existing.is_none());
                }
                "tag" => tags.extend(entry.get("id").cloned().unwrap()),
                "extra_stack_limit" => extra_stack_limit.extend(entry.get("id").cloned().unwrap()),
                "deaths_door" => { 
                    let existing = deaths_door.replace(entry);
                    debug_assert!(existing.is_none());
                }
                "mode" => modes.push(entry),
                _ => {
                    for (subkey, values) in entry {
                        let existing = other.insert((key.clone(), subkey), values);
                        debug_assert!(existing.is_none());
                    }
                }
            }
        }
        Ok(Self {
            id,
            resistances: Resistances::from_entry(resistances.unwrap()),
            weapons: Weapons::from_entries(weapons),
            armours: Armours::from_entries(armours),
            skills: Skills::from_entries(skills),
            riposte_skill: Skill::from_entries(riposte_skill),
            move_skill: MoveSkill::from_entry(move_skill.unwrap()),
            tags,
            extra_stack_limit,
            deaths_door: DeathsDoor::from_entry(deaths_door.unwrap()),
            modes: Modes::from_entries(modes),
            other,
        })
    }
}

#[derive(Clone, Debug)]
struct Resistances {
    stun: i32,
    poison: i32,
    bleed: i32,
    disease: i32,
    moving: i32,
    debuff: i32,
    death_blow: i32,
    trap: i32,
}

impl Resistances {
    fn from_entry(input: DarkestEntry) -> Self {
        macro_rules! extract {
            ($($key:literal -> $ident:ident),+) => {
                $(
                    let $ident = input.get($key).unwrap_or_else(|| panic!("Malformed hero information file, no {} resistance found", $key));
                    assert_eq!($ident.len(), 1, "Malformed hero information file: {} resistance have multiple values", $key);
                    let $ident = $ident[0].trim_end_matches('%').parse().unwrap_or_else(|_| panic!("Malformed hero information file, {} resistance is not an integer", $key));
                )+
            };
        }
        extract!(
            "stun" -> stun,
            "poison" -> poison,
            "bleed" -> bleed,
            "disease" -> disease,
            "move" -> moving,
            "debuff" -> debuff,
            "death_blow" -> death_blow,
            "trap" -> trap
        );
        Self {
            stun,
            poison,
            bleed,
            disease,
            moving,
            debuff,
            death_blow,
            trap,
        }
    }
}

impl BTreeMappable for Resistances {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.insert(vec!["stun".into()], self.stun.into());
        out.insert(vec!["poison".into()], self.poison.into());
        out.insert(vec!["bleed".into()], self.bleed.into());
        out.insert(vec!["disease".into()], self.disease.into());
        out.insert(vec!["move".into()], self.moving.into());
        out.insert(vec!["debuff".into()], self.debuff.into());
        out.insert(vec!["death_blow".into()], self.death_blow.into());
        out.insert(vec!["trap".into()], self.trap.into());
        out
    }
}

#[derive(Clone, Debug)]
struct Weapons([Weapon; 5]);
#[derive(Clone, Debug, Default)]
struct Weapon {
    atk: f32,
    dmg: (i32, i32),
    crit: f32,
    spd: i32,
}

impl Weapons {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        let out: Vec<_> = input.into_iter().map(Weapon::from_entry).collect();
        let out: &[_; 5] = out.as_slice().try_into().expect("Should be exactly 5 weapons");
        Self(out.to_owned())
    }
}
impl Weapon {
    fn from_entry(input: DarkestEntry) -> Self {
        let mut out = Self::default();
        out.atk = input.get("atk").expect("Weapon ATK not found").get(0).expect("Weapon ATK field is empty")
            .trim_end_matches('%')
            .parse()
            .expect("Weapon ATK is not a number");
        let mut dmg = input
            .get("dmg")
            .expect("Weapon DMG field not found")
            .into_iter()
            .map(|s| s.parse().expect("Weapon DMG field is not a number"));
        out.dmg = (dmg.next().expect("Weapon DMG field is empty"), dmg.next().expect("Weapon DMG field has only one entry"));
        out.crit = input.get("crit").expect("Weapon CRIT field not found")[0]
            .trim_end_matches('%')
            .parse()
            .expect("Weapon CRIT field is not a number");
        out.spd = input.get("spd").expect("Weapon SPD field not found")[0].parse().expect("Weapon SPD field is not a number");
        out
    }
}

impl BTreeMappable for Weapons {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        for (index, item) in self.0.iter().enumerate() {
            out.extend_prefixed(&index.to_string(), item.to_map());
        }
        out
    }
}

impl BTreeMappable for Weapon {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.insert(vec!["atk".into()], self.atk.into());
        out.insert(vec!["dmg min".into()], self.dmg.0.into());
        out.insert(vec!["dmg max".into()], self.dmg.1.into());
        out.insert(vec!["crit".into()], self.crit.into());
        out.insert(vec!["spd".into()], self.spd.into());
        out
    }
}

#[derive(Clone, Debug)]
struct Armours([Armour; 5]);
#[derive(Clone, Debug, Default)]
struct Armour {
    def: f32,
    prot: i32,
    hp: i32,
    spd: i32,
}

impl Armours {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        let out: Vec<_> = input.into_iter().map(Armour::from_entry).collect();
        let out: &[_; 5] = out.as_slice().try_into().expect("Should be exactly 5 armours");
        Self(out.to_owned())
    }
}
impl Armour {
    fn from_entry(input: DarkestEntry) -> Self {
        let mut out = Self::default();
        out.def = input.get("def").expect("Armour DEF field not found")[0]
            .trim_end_matches('%')
            .parse()
            .expect("Armour DEF field is not a number");
        out.prot = input.get("prot").expect("Armour PROT field not found")[0].parse().expect("Armour PROT field is not a number");
        out.hp = input.get("hp").expect("Armour HP field not found")[0].parse().expect("Armour HP field is not a number");
        out.spd = input.get("spd").expect("Armour SPD field not found")[0].parse().expect("Armour SPD field is not a number");
        out
    }
}

impl BTreeMappable for Armours {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        for (index, item) in self.0.iter().enumerate() {
            out.extend_prefixed(&index.to_string(), item.to_map());
        }
        out
    }
}

impl BTreeMappable for Armour {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.insert(vec!["def".into()], self.def.into());
        out.insert(vec!["prot".into()], self.prot.into());
        out.insert(vec!["hp".into()], self.hp.into());
        out.insert(vec!["spd".into()], self.spd.into());
        out
    }
}

#[derive(Clone, Debug)]
struct Skills(HashMap<(String, i32), Skill>);

impl Skills {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        let mut tmp: HashMap<(String, i32), Vec<DarkestEntry>> = HashMap::new();
        for entry in input {
            let id = entry.get("id").expect("Skill ID field not found")[0].clone();
            let level = entry.get("level").expect("Skill LEVEL field not found")[0].parse().expect("Skill LEVEL field is not a number");
            tmp.entry((id, level)).or_default().push(entry);
        }
        Self(
            tmp.into_iter()
                .map(|(key, value)| (key, Skill::from_entries(value)))
                .collect(),
        )
    }
}

impl BTreeMappable for Skills {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        for ((name, level), skill) in &self.0 {
            out.extend_prefixed(&format!("{}_{}", name, level), skill.to_map());
        }
        out
    }
}

#[derive(Clone, Debug)]
struct Skill {
    effects: Vec<String>,
    other: HashMap<String, String>,
}

impl Skill {
    fn from_entries(mut input: Vec<DarkestEntry>) -> Self {
        let effects = input
            .iter_mut()
            .flat_map(|entry| entry.remove("effect").unwrap_or_default())
            .collect();
        let other: HashMap<_, _> = input
            .into_iter()
            .flat_map(|entry| entry.into_iter())
            .map(|(key, v)| (key, v.join(" ")))
            .collect();
        Self { effects, other }
    }
}

impl BTreeMappable for Skill {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.extend_prefixed("effects", self.effects.to_map());
        out.extend(self.other.clone().into_iter().map(|(key, value)| (vec![key], value.into())));
        out
    }
}

#[derive(Clone, Debug)]
struct MoveSkill {
    forward: i32,
    backward: i32,
}
impl MoveSkill {
    fn from_entry(input: DarkestEntry) -> Self {
        let mut dmg = input
            .get("move")
            .expect("Move skill MOVE field not found")
            .into_iter()
            .map(|s| s.parse().expect("Move skill MOVE field is not a number"));
        Self {
            backward: dmg.next().expect("Move skill MOVE field is empty"),
            forward: dmg.next().expect("Move skill MOVE field has only one entry"),
        }
    }
}

impl BTreeMappable for MoveSkill {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.insert(vec!["forward".into()], self.forward.into());
        out.insert(vec!["backward".into()], self.backward.into());
        out
    }
}

#[derive(Clone, Debug)]
struct DeathsDoor {
    buffs: Vec<String>,
    recovery_buffs: Vec<String>,
    recovery_heart_attack_buffs: Vec<String>,
}

impl DeathsDoor {
    fn from_entry(mut input: DarkestEntry) -> Self {
        Self {
            buffs: input.remove("buffs").unwrap_or_default(),
            recovery_buffs: input.remove("recovery_buffs").unwrap_or_default(),
            recovery_heart_attack_buffs: input
                .remove("recovery_heart_attack_buffs")
                .unwrap_or_default(),
        }
    }
}

impl BTreeMappable for DeathsDoor {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.extend_prefixed("buffs", self.buffs.to_set());
        out
    }
}

#[derive(Clone, Debug)]
struct Modes(HashMap<String, Mode>);
impl Modes {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        Self(input.into_iter().map(Mode::from_entry).collect())
    }
}

impl BTreeMappable for Modes {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        for (key, value) in &self.0 {
            out.extend_prefixed(key, value.to_map());
        }
        out
    }
}

#[derive(Clone, Debug)]
struct Mode(HashMap<String, Vec<String>>);
impl Mode {
    fn from_entry(mut input: DarkestEntry) -> (String, Self) {
        (input.remove("id").unwrap().remove(0), Self(input.into_iter().collect()))
    }
}

impl BTreeMappable for Mode {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        for (key, value) in &self.0 {
            out.extend_prefixed(key, value.to_set());
        }
        out
    }
}