use super::super::{BTreeMappable, BTreePatchable, Loadable};
use crate::bundler::{
    diff::{DataMap, Patch},
    game_data::BTreeMapExt,
    loader::utils::{collect_paths, has_ext},
    ModFileChange,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StringsTable(HashMap<String, LanguageTable>);

#[derive(Serialize, Deserialize, Clone, Debug)]
struct LanguageTable(HashMap<String, String>);

impl BTreeMappable for StringsTable {
    fn to_map(&self) -> DataMap {
        let mut out = DataMap::new();
        for (key, table) in &self.0 {
            out.extend_prefixed(key, table.to_map());
        }
        out
    }
}
impl BTreePatchable for StringsTable {
    fn merge_patches(
        &self,
        patches: impl IntoIterator<Item = ModFileChange>,
    ) -> (Patch, Vec<ModFileChange>) {
        todo!()
    }
    fn apply_patch(&mut self, patch: Patch) -> Result<(), ()> {
        todo!()
    }
}
impl Loadable for StringsTable {
    fn prepare_list(root_path: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
        let path = root_path.join("localization");
        if path.exists() {
            collect_paths(&path, |path| Ok(has_ext(path, "xml")))
        } else {
            Ok(vec![])
        }
    }
    fn load_raw(path: &std::path::Path) -> std::io::Result<Self> {
        let mut out = HashMap::new();

        let mut xml = std::fs::read_to_string(path)?;
        // <HACK> Workaround: some localization files contain too big (non-existing) XML version.
        let decl = xml.lines().next().unwrap();
        let version = regex::Regex::new(r#"<?xml version="(.*?)"(.*)>"#).unwrap().captures(decl);
        match version {
            Some(version) => {
                let version = &version[0];
                if version > "1" {
                    xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#) + xml.splitn(2, '\n').nth(1).unwrap();
                }
            }
            _ => {}
        }
        // <HACK> Workaround: some localization files contain invalid comments.
        xml = regex::Regex::new("<!---(.*?)--->").unwrap().replace_all(&xml, "").into();
        let document = roxmltree::Document::parse(&xml)
            .expect(&format!("Malformed localization XML {:?}", path));
        let root = document.root_element();
        debug_assert_eq!(root.tag_name().name(), "root");
        for child in root.children() {
            if !child.is_element() {
                continue;
            }
            debug_assert_eq!(child.tag_name().name(), "language");
            let language = child.attribute("id").expect("Language ID not found");
            let mut table = HashMap::new();
            for item in child.children() {
                if !item.is_element() {
                    continue;
                }
                debug_assert_eq!(item.tag_name().name(), "entry");
                let key = item.attribute("id").expect("Entry ID not found");
                let value = item.text().unwrap_or("");
                table.insert(key.into(), value.into());
            }
            out.insert(language.into(), LanguageTable(table));
        }

        Ok(Self(out))
    }
}

impl BTreeMappable for LanguageTable {
    fn to_map(&self) -> DataMap {
        self.0
            .clone()
            .into_iter()
            .map(|(key, value)| (vec![key], value.into()))
            .collect()
    }
}
