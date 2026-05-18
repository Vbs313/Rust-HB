//! 构建时卡牌数据生成
//! 从 card_data_keywords.json 生成静态卡牌查找表
//! 数据来源：CardDefs.xml -> extract_keywords.js -> card_data_keywords.json

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(serde::Deserialize)]
struct CardEntry {
    id: u32,
    name: String,
    #[serde(rename = "type")]
    card_type: String,
    class: String,
    cost: i32,
    attack: i32,
    health: i32,
    armor: i32,
    durability: i32,
    name_en: String,
    name_cn: String,
    text_en: String,
    text_cn: String,
    set_id: u32,
    // 关键字（可选，从 CardDefs.xml 提取）
    #[serde(default)]
    has_taunt: bool,
    #[serde(default)]
    has_divine_shield: bool,
    #[serde(default)]
    has_charge: bool,
    #[serde(default)]
    has_rush: bool,
    #[serde(default)]
    has_stealth: bool,
    #[serde(default)]
    has_windfury: bool,
    #[serde(default)]
    has_poisonous: bool,
    #[serde(default)]
    has_lifesteal: bool,
    #[serde(default)]
    has_reborn: bool,
    #[serde(default)]
    has_elusive: bool,
    #[serde(default)]
    has_immune: bool,
    #[serde(default)]
    has_mega_windfury: bool,
}

#[derive(serde::Deserialize)]
struct CardDbJson {
    version: String,
    total: usize,
    cards: Vec<CardEntry>,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    let json_path = Path::new(&manifest_dir).join("card_data_keywords.json");

    // Fallback to plain card_data.json if keywords not extracted yet
    let json_path = if json_path.exists() {
        json_path
    } else {
        println!(
            "cargo:warning=card_data_keywords.json not found, using card_data.json (no keywords)"
        );
        Path::new(&manifest_dir).join("card_data.json")
    };

    if !json_path.exists() {
        println!("cargo:warning=Card data not found at {:?}", json_path);
        println!("cargo:warning=Run: node crates/silverfish-card-db/extract_cards.js");
        generate_empty();
        return;
    }

    println!("cargo:rerun-if-changed={}", json_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    let json_content = fs::read_to_string(&json_path).expect("Failed to read");
    let db: CardDbJson = serde_json::from_str(&json_content).expect("Failed to parse");
    println!("cargo:warning=Loaded {} cards", db.total);

    let type_map: HashMap<&str, u8> = [
        ("随从", 1u8),
        ("法术", 2u8),
        ("武器", 3u8),
        ("英雄", 4u8),
        ("英雄技能", 5u8),
        ("地标", 6u8),
        ("附魔", 7u8),
    ]
    .iter()
    .cloned()
    .collect();
    let type_id = |t: &str| -> u8 { *type_map.get(t).unwrap_or(&0) };

    let mut sorted: Vec<&CardEntry> = db.cards.iter().collect();
    sorted.sort_by_key(|c| c.id);

    let mut code = String::new();
    code.push_str("// 自动生成的卡牌数据库\n");
    code.push_str("#[allow(non_upper_case_globals, dead_code)]\n");
    code.push_str("#[derive(Debug, Clone, Copy, PartialEq)]\n#[repr(C)]\npub struct CardData {\n");
    code.push_str("    pub id: u32,\n    pub type_id: u8,\n    pub cost: i32,\n    pub attack: i32,\n    pub health: i32,\n    pub armor: i32,\n    pub durability: i32,\n    pub set_id: u32,\n");
    code.push_str("    pub has_taunt: bool,\n    pub has_divine_shield: bool,\n    pub has_charge: bool,\n    pub has_rush: bool,\n    pub has_stealth: bool,\n    pub has_windfury: bool,\n    pub has_poisonous: bool,\n    pub has_lifesteal: bool,\n    pub has_reborn: bool,\n    pub has_elusive: bool,\n    pub has_immune: bool,\n    pub has_mega_windfury: bool,\n");
    code.push_str("    pub name_cn: &'static str,\n    pub name_en: &'static str,\n    pub text: &'static str,\n}\n\n");
    code.push_str(&format!("pub const CARD_COUNT: usize = {};\n\n#[rustfmt::skip]\npub static CARD_DATA: &[CardData] = &[\n", sorted.len()));

    for card in &sorted {
        let tid = type_id(&card.card_type);
        let cn = esc(&card.name_cn);
        let en = esc(&card.name_en);
        let txt = esc(&card.text_cn);
        code.push_str(&format!(
            "    CardData {{ id: {}, type_id: {}, cost: {}, attack: {}, health: {}, armor: {}, durability: {}, set_id: {}, has_taunt: {}, has_divine_shield: {}, has_charge: {}, has_rush: {}, has_stealth: {}, has_windfury: {}, has_poisonous: {}, has_lifesteal: {}, has_reborn: {}, has_elusive: {}, has_immune: {}, has_mega_windfury: {}, name_cn: \"{}\", name_en: \"{}\", text: \"{}\" }},\n",
            card.id, tid, card.cost, card.attack, card.health,
            card.armor, card.durability, card.set_id,
            bool_val(card.has_taunt), bool_val(card.has_divine_shield), bool_val(card.has_charge),
            bool_val(card.has_rush), bool_val(card.has_stealth), bool_val(card.has_windfury),
            bool_val(card.has_poisonous), bool_val(card.has_lifesteal), bool_val(card.has_reborn),
            bool_val(card.has_elusive), bool_val(card.has_immune), bool_val(card.has_mega_windfury),
            cn, en, txt,
        ));
    }

    code.push_str("];\n\n");
    code.push_str("pub fn card_by_id(id: CardId) -> Option<&'static CardData> { let v: u32 = id.into(); CARD_DATA.binary_search_by_key(&v, |c| c.id).ok().map(|i| &CARD_DATA[i]) }\n");
    code.push_str("pub fn card_by_raw_id(id: u32) -> Option<&'static CardData> { CARD_DATA.binary_search_by_key(&id, |c| c.id).ok().map(|i| &CARD_DATA[i]) }\n");
    code.push_str("pub fn all_cards() -> &'static [CardData] { CARD_DATA }\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set");
    let out_path = Path::new(&out_dir).join("card_data.rs");
    fs::write(&out_path, &code).expect("Failed to write");
    println!(
        "cargo:warning=Generated {} cards ({:.1} KB)",
        sorted.len(),
        code.len() as f64 / 1024.0
    );
}

fn bool_val(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn generate_empty() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set");
    let out_path = Path::new(&out_dir).join("card_data.rs");
    let content = "#[derive(Debug, Clone, Copy, PartialEq)]\n#[repr(C)]\npub struct CardData { pub id: u32, pub type_id: u8, pub cost: i32, pub attack: i32, pub health: i32, pub armor: i32, pub durability: i32, pub set_id: u32, pub has_taunt: bool, pub has_divine_shield: bool, pub has_charge: bool, pub has_rush: bool, pub has_stealth: bool, pub has_windfury: bool, pub has_poisonous: bool, pub has_lifesteal: bool, pub has_reborn: bool, pub has_elusive: bool, pub has_immune: bool, pub has_mega_windfury: bool, pub name_cn: &'static str, pub name_en: &'static str, pub text: &'static str, }\npub const CARD_COUNT: usize = 0;\npub static CARD_DATA: &[CardData] = &[];\npub fn card_by_id(_id: CardId) -> Option<&'static CardData> { None }\npub fn card_by_raw_id(_id: u32) -> Option<&'static CardData> { None }\npub fn all_cards() -> &'static [CardData] { &[] }\n";
    fs::write(&out_path, content).expect("Failed to write");
    println!("cargo:warning=Generated empty card database");
}
