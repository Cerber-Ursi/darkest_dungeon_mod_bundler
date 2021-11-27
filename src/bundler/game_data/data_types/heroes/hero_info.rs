use crate::bundler::{
    diff::{Conflicts, DataMap, ItemChange, Patch},
    game_data::{
        file_types::{darkest_parser, DarkestEntry},
        BTreeMapExt, BTreeMappable, BTreePatchable, BTreeSetable, GameDataValue, Loadable,
    },
    loader::utils::{collect_paths, ends_with},
    ModFileChange,
};
use combine::EasyParser;
use log::debug;
use std::{collections::HashMap, convert::TryInto, num::ParseFloatError};

fn parse_percent(value: &str) -> Result<f32, ParseFloatError> {
    if value.ends_with('%') {
        Ok(value.trim_end_matches('%').parse::<f32>()? / 100.0)
    } else {
        value.parse()
    }
}

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
    incompatible_party_member: Incompatibilities,
    death_reaction: DeathReaction,
    other: HashMap<(String, String), Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct HeroOverride {
    id: String,
    resistances: ResistancesOverride,
    weapons: Option<Weapons>,
    armours: Option<Armours>,
    skills: Option<Skills>,
    riposte_skill: Option<Skill>,
    move_skill: Option<MoveSkill>,
    tags: Vec<String>,
    extra_stack_limit: Vec<String>,
    deaths_door: Option<DeathsDoor>,
    modes: Option<Modes>,
    incompatible_party_member: Option<Incompatibilities>,
    death_reaction: Option<DeathReaction>,
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
        inner.extend_prefixed(
            "incompatible_party_member",
            self.incompatible_party_member.to_map(),
        );
        inner.extend_prefixed("death_reaction", self.death_reaction.to_map());
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

impl BTreeMappable for HeroOverride {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        let mut inner = DataMap::new();

        inner.extend_prefixed("resistances", self.resistances.to_map());
        if let Some(weapons) = &self.weapons {
            inner.extend_prefixed("weapons", weapons.to_map());
        }
        if let Some(armours) = &self.armours {
            inner.extend_prefixed("armours", armours.to_map());
        }
        if let Some(skills) = &self.skills {
            inner.extend_prefixed("skills", skills.to_map());
        }
        if let Some(riposte_skill) = &self.riposte_skill {
            inner.extend_prefixed("riposte_skill", riposte_skill.to_map());
        }
        if let Some(move_skill) = &self.move_skill {
            inner.extend_prefixed("move_skill", move_skill.to_map());
        }
        inner.extend_prefixed("tags", self.tags.to_set());
        inner.extend_prefixed("extra_stack_limit", self.extra_stack_limit.to_set());
        if let Some(deaths_door) = &self.deaths_door {
            inner.extend_prefixed("deaths_door", deaths_door.to_map());
        }
        if let Some(modes) = &self.modes {
            inner.extend_prefixed("modes", modes.to_map());
        }
        if let Some(incompatible_party_member) = &self.incompatible_party_member {
            inner.extend_prefixed(
                "incompatible_party_member",
                incompatible_party_member.to_map(),
            );
        }
        if let Some(death_reaction) = &self.death_reaction {
            inner.extend_prefixed("death_reaction", death_reaction.to_map());
        }
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

fn next_effect(
    prev: &str,
    orig_effects: &[String],
    patch: &Patch,
    prefix: &[String],
) -> Option<String> {
    let patched = patch.get(
        &prefix
            .iter()
            .cloned()
            .chain(std::iter::once(prev.into()))
            .collect::<Vec<_>>(),
    );
    if let Some(ItemChange::Removed) = &patched {
        // This is set by the patch to be the end of chain.
        return None;
    }
    patched
        .map(|item| match item {
            ItemChange::Set(GameDataValue::String(effect)) => effect,
            _ => panic!("Skill effects can only be strings"),
        })
        .or_else(|| {
            let index = orig_effects.iter().position(|eff| eff == prev);
            index.and_then(|index| orig_effects.get(index + 1))
        })
        .cloned()
}

fn patch_skill_effects(skill: &mut Skill, patch: &Patch, prefix: &[String]) {
    let prefix: Vec<_> = prefix
        .iter()
        .cloned()
        .chain(std::iter::once("effect".into()))
        .collect();
    let mut effects = vec![];
    let start_patched = patch.get(&prefix);
    if let Some(ItemChange::Removed) = &start_patched {
        // Effects are dropped entirely by patch
        skill.effects = vec![];
        return;
    }
    // Now, there might be some effects; let's find the start of them
    let start = start_patched
        .map(|item| match item {
            ItemChange::Set(GameDataValue::String(effect)) => effect,
            _ => panic!("Skill effects can only be strings"),
        })
        .or_else(|| skill.effects.get(0));
    if let Some(cur) = start {
        let mut cur = cur.to_string();
        // There really are some effects - either patch set them start, or the start remained unchanged
        effects.push(cur.clone());
        while let Some(next) = next_effect(&cur, &skill.effects, patch, &prefix) {
            effects.push(next.clone());
            cur = next;
        }
        skill.effects = effects;
    }
    // Otherwise, there were no effects and there are no effects.
}

fn patch_list(list: &mut Vec<String>, mut path: Vec<String>, change: ItemChange) {
    // TODO - some debug assert to ensure that the path is correct
    let key = path.pop().unwrap();
    match change.into_option().map(GameDataValue::unwrap_unit) {
        Some(()) => list.push(key),
        None => {
            // copied from Vec::remove_item
            let pos = list.iter().position(|x| x == &key).unwrap_or_else(|| {
                panic!(
                    "Unexpected key in hero info path: {:?}, attempt to remove non-existing entry",
                    path
                )
            });
            list.remove(pos);
        }
    };
}

impl BTreePatchable for HeroInfo {
    fn apply_patch(&mut self, patch: Patch) -> Result<(), ()> {
        // First, we should collect all effects used in patch.
        for ((skill, level), ref mut skill_data) in self.skills.0.iter_mut() {
            patch_skill_effects(
                skill_data,
                &patch,
                &["skills".into(), format!("{}_{}", skill, level)],
            );
        }
        patch_skill_effects(
            &mut self.riposte_skill,
            &patch,
            &["riposte_skill".into()],
        );

        // Now, all other parts are simpler... we'll just patch it key-by-key.
        for (mut path, change) in patch {
            match path.get(0).unwrap().as_str() {
                "resistances" => self.resistances.apply(path, change),
                "weapons" => self.weapons.apply(path, change),
                "armours" => self.armours.apply(path, change),
                "skills" => self.skills.apply(path, change),
                "riposte_skill" => self.riposte_skill.apply(path, change),
                "move_skill" => self.move_skill.apply(path, change),
                "tags" => patch_list(&mut self.tags, path, change),
                "extra_stack_limit" => patch_list(&mut self.extra_stack_limit, path, change),
                "deaths_door" => self.deaths_door.apply(path, change),
                "modes" => self.modes.apply(path, change),
                "incompatible_party_member" => self.incompatible_party_member.apply(path, change),
                "death_reaction" => self.death_reaction.apply(path, change),
                "other" => {
                    let first = path.remove(1);
                    let second = path.remove(2);
                    match change.into_option().map(GameDataValue::unwrap_string) {
                        Some(s) => {
                            self.other.entry((first, second)).or_default().push(s);
                        }
                        None => {
                            self.other.remove(&(first, second));
                        }
                    }
                }
                _ => panic!("Unexpected key in hero data patch: {:?}", path),
            }
        }
        Ok(())
    }
    fn try_merge_patches(
        &self,
        patches: impl IntoIterator<Item = ModFileChange>,
    ) -> (Patch, Conflicts) {
        let mut merged = Patch::new();
        let mut unmerged = Conflicts::new();

        // TODO - this is almost the same as `regroup` in `resolve` module
        let mut changes = HashMap::new();
        for (mod_name, mod_changes) in patches {
            for (path, item) in mod_changes {
                // False positive from clippy - https://github.com/rust-lang/rust-clippy/issues/5693
                #[allow(clippy::or_fun_call)]
                changes
                    .entry(path)
                    .or_insert(vec![])
                    .push((mod_name.clone(), item));
            }
        }

        let mut skill_effects = HashMap::new();
        let mut riposte_effects = vec![];
        for (path, changes) in changes {
            debug_assert!(!changes.is_empty());
            if path.get(0).map(String::as_str) == Some("skills")
                && path.get(2).map(String::as_str) == Some("effects")
            {
                // False positive from clippy - https://github.com/rust-lang/rust-clippy/issues/5693
                #[allow(clippy::or_fun_call)]
                skill_effects
                    .entry(path.get(1).unwrap().clone())
                    .or_insert(vec![])
                    .extend(changes);
            } else if path.get(0).map(String::as_str) == Some("riposte_skill")
                && path.get(1).map(String::as_str) == Some("effects")
            {
                riposte_effects.extend(changes);
            } else if changes.len() == 1 {
                merged.insert(path, changes.into_iter().next().unwrap().1);
            } else {
                for change in changes {
                    unmerged.entry(path.clone()).or_default().push(change)
                }
            }
        }
        for (skill, mut effects) in skill_effects {
            let path = vec!["skills".into(), skill, "effects".into()];
            if effects.len() == 1 {
                merged.insert(path, effects.remove(0).1);
            } else {
                unmerged.insert(path, effects);
            }
        }
        let path = vec!["riposte_skill".into(), "effects".into()];
        if riposte_effects.len() == 1 {
            merged.insert(path, riposte_effects.remove(0).1);
        } else {
            unmerged.insert(path, riposte_effects);
        }

        (merged, unmerged)
    }

    fn ask_for_resolve(&self, sink: &mut cursive::CbSink, conflicts: Conflicts) -> Patch {
        todo!()
    }
}

impl BTreePatchable for HeroOverride {
    fn apply_patch(&mut self, patch: Patch) -> Result<(), ()> {
        debug!("{:?}", patch);
        todo!()
    }
    fn try_merge_patches(
        &self,
        patches: impl IntoIterator<Item = ModFileChange>,
    ) -> (Patch, Conflicts) {
        todo!()
    }
    fn ask_for_resolve(&self, sink: &mut cursive::CbSink, patches: Conflicts) -> Patch {
        todo!()
    }
}

impl Loadable for HeroInfo {
    fn prepare_list(root_path: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
        let path = root_path.join("heroes");
        if path.exists() {
            collect_paths(&path, |path| Ok(ends_with(path, ".info.darkest")))
        } else {
            Ok(vec![])
        }
    }
    fn load_raw(path: &std::path::Path) -> std::io::Result<Self> {
        let id = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .split('.')
            .next()
            .unwrap()
            .to_string();

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
        let mut incompatible_party_member = vec![];
        let mut death_reaction = vec![];
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
                "incompatible_party_member" => incompatible_party_member.push(entry),
                "death_reaction" => death_reaction.push(entry),
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
            incompatible_party_member: Incompatibilities::from_entries(incompatible_party_member),
            death_reaction: DeathReaction::from_entries(death_reaction),
            other,
        })
    }
}

impl Loadable for HeroOverride {
    fn prepare_list(root_path: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
        let path = root_path.join("heroes");
        if path.exists() {
            collect_paths(&path, |path| Ok(ends_with(path, ".override.darkest")))
        } else {
            Ok(vec![])
        }
    }
    fn load_raw(path: &std::path::Path) -> std::io::Result<Self> {
        let id = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .split('.')
            .next()
            .unwrap()
            .to_string();

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
        let mut incompatible_party_member = vec![];
        let mut death_reaction = vec![];
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
                "incompatible_party_member" => incompatible_party_member.push(entry),
                "death_reaction" => death_reaction.push(entry),
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
            resistances: resistances
                .map(ResistancesOverride::from_entry)
                .unwrap_or_default(),
            weapons: opt_vec(weapons).map(Weapons::from_entries),
            armours: opt_vec(armours).map(Armours::from_entries),
            skills: opt_vec(skills).map(Skills::from_entries),
            riposte_skill: opt_vec(riposte_skill).map(Skill::from_entries),
            move_skill: move_skill.map(MoveSkill::from_entry),
            tags,
            extra_stack_limit,
            deaths_door: deaths_door.map(DeathsDoor::from_entry),
            modes: opt_vec(modes).map(Modes::from_entries),
            incompatible_party_member: opt_vec(incompatible_party_member)
                .map(Incompatibilities::from_entries),
            death_reaction: opt_vec(death_reaction).map(DeathReaction::from_entries),
            other,
        })
    }
}

fn opt_vec<T>(v: Vec<T>) -> Option<Vec<T>> {
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[derive(Clone, Debug)]
struct Resistances {
    stun: f32,
    poison: f32,
    bleed: f32,
    disease: f32,
    moving: f32,
    debuff: f32,
    death_blow: f32,
    trap: f32,
}

impl Resistances {
    fn from_entry(mut input: DarkestEntry) -> Self {
        macro_rules! extract {
            ($($key:literal -> $ident:ident),+) => {
                $(
                    let $ident = input.remove($key).unwrap_or_else(|| panic!("Malformed hero information file, no {} resistance found", $key));
                    assert_eq!($ident.len(), 1, "Malformed hero information file: {} resistance have multiple values", $key);
                    let $ident = parse_percent(&$ident[0]).unwrap_or_else(|_| panic!("Malformed hero information file, {} resistance is not an percent-like value", $key));
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
        assert!(input.is_empty());
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
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "resistances");
        assert!(path.len() == 2, "Invalid path: {:?}", path);
        let num = change
            .into_option()
            .unwrap_or_else(|| {
                panic!(
                    "Unexpected patch for resistances: trying to remove key {:?}",
                    path
                )
            })
            .unwrap_f32();
        *(match path[1].as_str() {
            "stun" => &mut self.stun,
            "poison" => &mut self.poison,
            "bleed" => &mut self.bleed,
            "disease" => &mut self.disease,
            "move" => &mut self.moving,
            "debuff" => &mut self.debuff,
            "death_blow" => &mut self.death_blow,
            "trap" => &mut self.trap,
            _ => panic!("Unexpected patch for resistances: key = {:?}", path),
        }) = num;
    }
}

#[derive(Clone, Debug, Default)]
struct ResistancesOverride {
    stun: Option<f32>,
    poison: Option<f32>,
    bleed: Option<f32>,
    disease: Option<f32>,
    moving: Option<f32>,
    debuff: Option<f32>,
    death_blow: Option<f32>,
    trap: Option<f32>,
}

impl ResistancesOverride {
    fn from_entry(input: DarkestEntry) -> Self {
        macro_rules! extract {
            ($($key:literal -> $ident:ident),+) => {
                $(
                    let $ident = input.get($key).map(|data| {
                        assert_eq!(data.len(), 1, "Malformed hero information file: {} resistance have multiple values", $key);
                        parse_percent(&data[0]).unwrap_or_else(|_| panic!("Malformed hero information file, {} resistance is not an percent-like value", $key))
                    });
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
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "resistances");
        assert!(path.len() == 2, "Invalid path: {:?}", path);
        *(match path[1].as_str() {
            "stun" => &mut self.stun,
            "poison" => &mut self.poison,
            "bleed" => &mut self.bleed,
            "disease" => &mut self.disease,
            "move" => &mut self.moving,
            "debuff" => &mut self.debuff,
            "death_blow" => &mut self.death_blow,
            "trap" => &mut self.trap,
            _ => panic!("Unexpected patch for resistances: key = {:?}", path),
        }) = change.into_option().map(GameDataValue::unwrap_f32);
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

impl BTreeMappable for ResistancesOverride {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        if let Some(stun) = self.stun {
            out.insert(vec!["stun".into()], stun.into());
        }
        if let Some(poison) = self.poison {
            out.insert(vec!["poison".into()], poison.into());
        }
        if let Some(bleed) = self.bleed {
            out.insert(vec!["bleed".into()], bleed.into());
        }
        if let Some(disease) = self.disease {
            out.insert(vec!["disease".into()], disease.into());
        }
        if let Some(moving) = self.moving {
            out.insert(vec!["move".into()], moving.into());
        }
        if let Some(debuff) = self.debuff {
            out.insert(vec!["debuff".into()], debuff.into());
        }
        if let Some(death_blow) = self.death_blow {
            out.insert(vec!["death_blow".into()], death_blow.into());
        }
        if let Some(trap) = self.trap {
            out.insert(vec!["trap".into()], trap.into());
        }
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
        let out: &[_; 5] = out
            .as_slice()
            .try_into()
            .expect("Should be exactly 5 weapons");
        Self(out.to_owned())
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "weapons");
        let index: usize = path[1].parse().unwrap();
        match path[2].as_str() {
            "atk" => self.0[index].atk = change.unwrap_set().unwrap_f32(),
            "dmg min" => self.0[index].dmg.0 = change.unwrap_set().unwrap_i32(),
            "dmg max" => self.0[index].dmg.1 = change.unwrap_set().unwrap_i32(),
            "crit" => self.0[index].atk = change.unwrap_set().unwrap_f32(),
            "spd" => self.0[index].spd = change.unwrap_set().unwrap_i32(),
            _ => panic!("Unexpected key in hero into patch: {:?}", path),
        };
    }
}

impl Weapon {
    fn from_entry(input: DarkestEntry) -> Self {
        let mut out = Self::default();
        out.atk = parse_percent(
            input
                .get("atk")
                .expect("Weapon ATK not found")
                .get(0)
                .expect("Weapon ATK field is empty"),
        )
        .expect("Weapon ATK is not a number");
        let mut dmg = input
            .get("dmg")
            .expect("Weapon DMG field not found")
            .iter()
            .map(|s| s.parse().expect("Weapon DMG field is not a number"));
        out.dmg = (
            dmg.next().expect("Weapon DMG field is empty"),
            dmg.next().expect("Weapon DMG field has only one entry"),
        );
        out.crit = parse_percent(&input.get("crit").expect("Weapon CRIT field not found")[0])
            .expect("Weapon CRIT field is not a number");
        let spd = input
            .get("spd")
            .expect("Weapon SPD field not found")
            .get(0)
            .expect("Weapon SPD field is empty");
        out.spd = spd.parse().expect("Weapon SPD field is not a number");
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
    prot: f32,
    hp: i32,
    spd: i32,
}

impl Armours {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        let out: Vec<_> = input.into_iter().map(Armour::from_entry).collect();
        let out: &[_; 5] = out
            .as_slice()
            .try_into()
            .expect("Should be exactly 5 armours");
        Self(out.to_owned())
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "armours");
        let index: usize = path[1].parse().unwrap();
        match path[2].as_str() {
            "def" => self.0[index].def = change.unwrap_set().unwrap_f32(),
            "prot" => self.0[index].prot = change.unwrap_set().unwrap_f32(),
            "hp" => self.0[index].hp = change.unwrap_set().unwrap_i32(),
            "spd" => self.0[index].spd = change.unwrap_set().unwrap_i32(),
            _ => panic!("Unexpected key in hero into patch: {:?}", path),
        };
    }
}
impl Armour {
    fn from_entry(input: DarkestEntry) -> Self {
        let mut out = Self::default();
        out.def = parse_percent(&input.get("def").expect("Armour DEF field not found")[0])
            .expect("Armour DEF field is not a number");
        out.prot = parse_percent(&input.get("prot").expect("Armour PROT field not found")[0])
            .expect("Armour PROT field is not a number");
        out.hp = input.get("hp").expect("Armour HP field not found")[0]
            .parse()
            .expect("Armour HP field is not a number");
        out.spd = input.get("spd").expect("Armour SPD field not found")[0]
            .parse()
            .expect("Armour SPD field is not a number");
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
            let level = entry.get("level").expect("Skill LEVEL field not found")[0]
                .parse()
                .expect("Skill LEVEL field is not a number");
            tmp.entry((id, level)).or_default().push(entry);
        }
        Self(
            tmp.into_iter()
                .map(|(key, value)| (key, Skill::from_entries(value)))
                .collect(),
        )
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "skills");
        let (name, level) = {
            let mut iter = path[1].split('_');
            (
                iter.next().unwrap(),
                iter.next()
                    .unwrap_or_else(|| panic!("Unexpected path in hero data: {:?}, wrong skill ID, expected format <NAME>_<LEVEL>", path))
                    .parse()
                    .unwrap_or_else(|_| panic!("Unexpected path in hero data: {:?}, wrong skill ID, expected format <NAME>_<LEVEL>", path))
            )
        };
        self.0
            .get_mut(&(name.into(), level))
            .unwrap_or_else(|| panic!("Unexpected path in hero data: {:?}, skill not found", path))
            .apply(path, change);
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
    fn apply(&mut self, mut path: Vec<String>, change: ItemChange) {
        let key = match path[0].as_str() {
            "skills" => path.remove(2),
            "riposte_skill" => path.remove(1),
            _ => panic!("Unexpected path in hero info: {:?}", path),
        };
        match change.into_option().map(GameDataValue::unwrap_string) {
            Some(s) => self.other.insert(key, s),
            None => self.other.remove(&key),
        };
    }
}

impl BTreeMappable for Skill {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.extend_prefixed("effects", self.effects.to_map());
        out.extend(
            self.other
                .clone()
                .into_iter()
                .map(|(key, value)| (vec![key], value.into())),
        );
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
            .iter()
            .map(|s| s.parse().expect("Move skill MOVE field is not a number"));
        Self {
            backward: dmg.next().expect("Move skill MOVE field is empty"),
            forward: dmg
                .next()
                .expect("Move skill MOVE field has only one entry"),
        }
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "move_skill");
        match path[1].as_str() {
            "forward" => self.forward = change.unwrap_set().unwrap_i32(),
            "backward" => self.backward = change.unwrap_set().unwrap_i32(),
            _ => panic!("Unexpected key in hero info patch: {:?}", path),
        };
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
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert!(path.len() == 3);
        debug_assert_eq!(path[0], "move_skill");
        let place = match path[1].as_str() {
            "buffs" => &mut self.buffs,
            "recovery_buffs" => &mut self.recovery_buffs,
            "recovery_heart_attack_buffs" => &mut self.recovery_heart_attack_buffs,
            _ => panic!("Unexpected key in hero info patch: {:?}", path),
        };
        patch_list(place, path, change);
    }
}

impl BTreeMappable for DeathsDoor {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        out.extend_prefixed("buffs", self.buffs.to_set());
        out.extend_prefixed("recovery_buffs", self.recovery_buffs.to_set());
        out.extend_prefixed(
            "recovery_heart_attack_buffs",
            self.recovery_heart_attack_buffs.to_set(),
        );
        out
    }
}

#[derive(Clone, Debug)]
struct Modes(HashMap<String, Mode>);
impl Modes {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        Self(input.into_iter().map(Mode::from_entry).collect())
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "modes");
        self.0
            .get_mut(path.get(1).unwrap())
            .unwrap_or_else(|| panic!("Unexpected path in hero data: {:?}, mode not found", path))
            .apply(path, change);
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
        (
            input.remove("id").unwrap().remove(0),
            Self(input.into_iter().collect()),
        )
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "modes");
        assert!(path.len() == 4);
        let place = self.0.entry(path.get(2).unwrap().clone()).or_default();
        patch_list(place, path, change);
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

#[derive(Clone, Debug)]
struct Incompatibilities(HashMap<String, Vec<String>>);
impl Incompatibilities {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        let mut map = HashMap::new();
        for mut entry in input {
            let id = entry.remove("id").unwrap().remove(0);
            let tag = entry.remove("hero_tag").unwrap().remove(0);
            // False positive from clippy - https://github.com/rust-lang/rust-clippy/issues/5693
            #[allow(clippy::or_fun_call)]
            map.entry(id).or_insert(vec![]).push(tag);
        }
        Self(map)
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "incompatible_party_member");
        assert!(path.len() == 3);
        let place = self.0.entry(path.get(1).unwrap().clone()).or_default();
        patch_list(place, path, change);
    }
}

impl BTreeMappable for Incompatibilities {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        for (key, value) in &self.0 {
            out.extend_prefixed(key, value.to_set());
        }
        out
    }
}

#[derive(Clone, Debug)]
struct DeathReaction(Vec<String>);
impl DeathReaction {
    fn from_entries(input: Vec<DarkestEntry>) -> Self {
        Self(input.into_iter().map(|entry| entry.to_string()).collect())
    }
    fn apply(&mut self, path: Vec<String>, change: ItemChange) {
        debug_assert_eq!(path[0], "death_reaction");
        assert!(path.len() == 2);
        patch_list(&mut self.0, path, change);
    }
}

impl BTreeMappable for DeathReaction {
    fn to_map(&self) -> DataMap {
        self.0.to_set()
    }
}
