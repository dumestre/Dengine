use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone)]
pub struct AvailableModule {
    pub asset: String,
    pub display_name: String,
    pub category: String,
    pub description: Option<String>,
    pub extra_info: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct ModuleCategory {
    pub name: String,
    pub modules: Vec<AvailableModule>,
}

#[derive(Clone)]
pub struct ModuleControl {
    #[allow(dead_code)]
    pub node_id: u32,
    pub name: String,
    pub value: f32,
    pub param_a: f32,
    pub param_b: f32,
}

#[derive(Clone)]
pub struct ModuleChainItem {
    pub id: u32,
    pub name: String,
    pub asset: String,
    pub enabled: bool,
    pub group_id: Option<u32>,
    pub description: Option<String>,
    pub controls: Vec<ModuleControl>,
    pub extra_info: Vec<(String, String)>,
}

pub fn friendly_module_name(asset: &str) -> String {
    let stem = Path::new(asset)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(asset);
    stem.split(|c: char| c == '_' || c == '-')
        .filter(|part| !part.trim().is_empty())
        .map(|part| {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                let mut result = first.to_uppercase().collect::<String>();
                result.push_str(chars.as_str());
                result
            } else {
                String::new()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn module_category(module_name: &str) -> String {
    let stem = Path::new(module_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(module_name)
        .to_string();
    if let Some(idx) = stem.find('_') {
        let prefix = &stem[..idx];
        if !prefix.trim().is_empty() {
            return prefix.to_string();
        }
    }
    if let Some(idx) = stem.find('-') {
        let prefix = &stem[..idx];
        if !prefix.trim().is_empty() {
            return prefix.to_string();
        }
    }
    "General".to_string()
}

pub fn parse_available_module(asset: String) -> AvailableModule {
    let mut display = friendly_module_name(&asset);
    let mut category = String::new();
    let mut description = None;
    let mut extra_info: Vec<(String, String)> = Vec::new();
    let path = Path::new("Assets")
        .join("Animations")
        .join("Modules")
        .join(&asset);
    if let Ok(contents) = fs::read_to_string(&path) {
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                match key.as_str() {
                    "name" => display = value.to_string(),
                    "category" => category = value.to_string(),
                    "description" => description = Some(value.to_string()),
                    _ => extra_info.push((key.to_string(), value.to_string())),
                }
            }
        }
    }
    if category.is_empty() {
        category = module_category(&asset);
    }
    AvailableModule {
        asset,
        display_name: display,
        category,
        description,
        extra_info,
    }
}

pub fn group_modules_by_category(modules: Vec<AvailableModule>) -> Vec<ModuleCategory> {
    let mut map: BTreeMap<String, Vec<AvailableModule>> = BTreeMap::new();
    for module in modules {
        map.entry(module.category.clone()).or_default().push(module);
    }
    let mut out = Vec::new();
    for (category, mut modules) in map {
        modules.sort_by_key(|m| m.display_name.clone());
        out.push(ModuleCategory {
            name: category,
            modules,
        });
    }
    out
}
