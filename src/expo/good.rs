use anyhow::Result;
use std::convert::From;

use serde::ser::{Serialize, SerializeMap, Serializer};

use crate::artifact::internal_artifact::{
    ArtifactSetKey, ArtifactSlotKey, ArtifactStat, ArtifactStatKey, InternalArtifact,
};

impl ArtifactStatKey {
    pub fn to_good(&self) -> String {
        let temp = match self {
            ArtifactStatKey::HealingBonus => "heal_",
            ArtifactStatKey::CriticalDamage => "critDMG_",
            ArtifactStatKey::Critical => "critRate_",
            ArtifactStatKey::Atk => "atk",
            ArtifactStatKey::AtkPercentage => "atk_",
            ArtifactStatKey::ElementalMastery => "eleMas",
            ArtifactStatKey::Recharge => "enerRech_",
            ArtifactStatKey::HpPercentage => "hp_",
            ArtifactStatKey::Hp => "hp",
            ArtifactStatKey::DefPercentage => "def_",
            ArtifactStatKey::Def => "def",
            ArtifactStatKey::ElectroBonus => "electro_dmg_",
            ArtifactStatKey::PyroBonus => "pyro_dmg_",
            ArtifactStatKey::HydroBonus => "hydro_dmg_",
            ArtifactStatKey::CryoBonus => "cryo_dmg_",
            ArtifactStatKey::AnemoBonus => "anemo_dmg_",
            ArtifactStatKey::GeoBonus => "geo_dmg_",
            ArtifactStatKey::PhysicalBonus => "physical_dmg_",
            ArtifactStatKey::DendroBonus => "dendro_dmg_",
        };
        String::from(temp)
    }
}

impl ArtifactSetKey {
    pub fn to_good(&self) -> String {
        return self.to_string();
    }
}

impl ArtifactSlotKey {
    pub fn to_good(&self) -> String {
        let temp = match self {
            ArtifactSlotKey::Flower => "flower",
            ArtifactSlotKey::Plume => "plume",
            ArtifactSlotKey::Sands => "sands",
            ArtifactSlotKey::Goblet => "goblet",
            ArtifactSlotKey::Circlet => "circlet",
        };
        String::from(temp)
    }
}

fn to_char_key(name: &str) -> &str {
    match name {
        "神里绫华" => "KamisatoAyaka",
        "琴" => "Jean",
        "旅行者" => "Traveler",
        "丽莎" => "Lisa",
        "芭芭拉" => "Barbara",
        "凯亚" => "Kaeya",
        "迪卢克" => "Diluc",
        "雷泽" => "Razor",
        "安柏" => "Amber",
        "温迪" => "Venti",
        "香菱" => "Xiangling",
        "北斗" => "Beidou",
        "行秋" => "Xingqiu",
        "魈" => "Xiao",
        "凝光" => "Ningguang",
        "可莉" => "Klee",
        "钟离" => "Zhongli",
        "菲谢尔" => "Fischl",
        "班尼特" => "Bennett",
        "达达利亚" => "Tartaglia",
        "诺艾尔" => "Noelle",
        "七七" => "Qiqi",
        "重云" => "Chongyun",
        "甘雨" => "Ganyu",
        "阿贝多" => "Albedo",
        "迪奥娜" => "Diona",
        "莫娜" => "Mona",
        "刻晴" => "Keqing",
        "砂糖" => "Sucrose",
        "辛焱" => "Xinyan",
        "罗莎莉亚" => "Rosaria",
        "胡桃" => "HuTao",
        "枫原万叶" => "KaedeharaKazuha",
        "烟绯" => "Yanfei",
        "宵宫" => "Yoimiya",
        "托马" => "Thoma",
        "优菈" => "Eula",
        "雷电将军" => "RaidenShogun",
        "早柚" => "Sayu",
        "珊瑚宫心海" => "SangonomiyaKokomi",
        "五郎" => "Gorou",
        "九条裟罗" => "KujouSara",
        "荒泷一斗" => "AratakiItto",
        "八重神子" => "YaeMiko",
        "鹿野院平藏" => "ShikanoinHeizou",
        "夜兰" => "Yelan",
        "绮良良" => "Kirara",
        "埃洛伊" => "Aloy",
        "申鹤" => "Shenhe",
        "云堇" => "YunJin",
        "久岐忍" => "KukiShinobu",
        "神里绫人" => "KamisatoAyato",
        "柯莱" => "Collei",
        "多莉" => "Dori",
        "提纳里" => "Tighnari",
        "妮露" => "Nilou",
        "赛诺" => "Cyno",
        "坎蒂丝" => "Candace",
        "纳西妲" => "Nahida",
        "莱依拉" => "Layla",
        "流浪者" => "Wanderer",
        "珐露珊" => "Faruzan",
        "瑶瑶" => "Yaoyao",
        "艾尔海森" => "Alhaitham",
        "迪希雅" => "Dehya",
        "米卡" => "Mika",
        "卡维" => "Kaveh",
        "白术" => "Baizhu",
        "" => "",
        _ => "Wanderer", // 流浪者很忙
    }
}

struct GoodArtifactStat<'a> {
    stat: &'a ArtifactStat,
}

impl<'a> Serialize for GoodArtifactStat<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(Some(2))?;
        root.serialize_entry("key", &self.stat.key.to_good())?;
        root.serialize_entry("value", &self.stat.value)?;
        root.end()
    }
}

struct GoodArtifact<'a> {
    artifact: &'a InternalArtifact,
}

impl<'a> Serialize for GoodArtifact<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(Some(8))?;

        root.serialize_entry("setKey", &self.artifact.set_key.to_good())?;
        root.serialize_entry("slotKey", &self.artifact.slot_key.to_good())?;
        root.serialize_entry("level", &self.artifact.level)?;
        root.serialize_entry("rarity", &self.artifact.rarity)?;
        root.serialize_entry("lock", &self.artifact.lock)?;
        root.serialize_entry("location", &to_char_key(&self.artifact.location))?;
        root.serialize_entry("mainStatKey", &self.artifact.main_stat.key.to_good())?;
        let mut substats: Vec<GoodArtifactStat> = vec![];
        if let Some(ref s) = self.artifact.sub_stat_1 {
            substats.push(GoodArtifactStat { stat: s });
        }
        if let Some(ref s) = self.artifact.sub_stat_2 {
            substats.push(GoodArtifactStat { stat: s });
        }
        if let Some(ref s) = self.artifact.sub_stat_3 {
            substats.push(GoodArtifactStat { stat: s });
        }
        if let Some(ref s) = self.artifact.sub_stat_4 {
            substats.push(GoodArtifactStat { stat: s });
        }
        root.serialize_entry("substats", &substats)?;
        root.end()
    }
}

pub struct GoodFormat<'a> {
    format: String,
    version: u32,
    source: String,
    artifacts: Vec<GoodArtifact<'a>>,
}

impl<'a> Serialize for GoodFormat<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(Some(4))?;
        root.serialize_entry("format", &self.format)?;
        root.serialize_entry("version", &self.version)?;
        root.serialize_entry("source", &self.source)?;
        root.serialize_entry("artifacts", &self.artifacts)?;
        root.end()
    }
}

impl<'a> GoodFormat<'a> {
    pub fn new(results: &'a Vec<InternalArtifact>) -> GoodFormat {
        let artifacts: Vec<GoodArtifact<'a>> = results
            .into_iter()
            .map(|artifact| GoodArtifact { artifact })
            .collect();

        GoodFormat {
            format: String::from("GOOD"),
            version: 1,
            source: String::from("yas-lock"),
            artifacts,
        }
    }
}
