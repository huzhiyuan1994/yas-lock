use std::convert::From;
use std::fs::File;
use std::io::prelude::*;

use serde::ser::{Serialize, SerializeMap, Serializer};

use crate::artifact::internal_artifact::{
    ArtifactSetKey, ArtifactSlotKey, ArtifactStat, ArtifactStatKey, InternalArtifact,
};

type MonaArtifact = InternalArtifact;

impl ArtifactStatKey {
    pub fn to_mona(&self) -> String {
        let temp = match self {
            ArtifactStatKey::HealingBonus => "cureEffect",
            ArtifactStatKey::CriticalDamage => "criticalDamage",
            ArtifactStatKey::Critical => "critical",
            ArtifactStatKey::Atk => "attackStatic",
            ArtifactStatKey::AtkPercentage => "attackPercentage",
            ArtifactStatKey::ElementalMastery => "elementalMastery",
            ArtifactStatKey::Recharge => "recharge",
            ArtifactStatKey::HpPercentage => "lifePercentage",
            ArtifactStatKey::Hp => "lifeStatic",
            ArtifactStatKey::DefPercentage => "defendPercentage",
            ArtifactStatKey::Def => "defendStatic",
            ArtifactStatKey::ElectroBonus => "thunderBonus",
            ArtifactStatKey::PyroBonus => "fireBonus",
            ArtifactStatKey::HydroBonus => "waterBonus",
            ArtifactStatKey::CryoBonus => "iceBonus",
            ArtifactStatKey::AnemoBonus => "windBonus",
            ArtifactStatKey::GeoBonus => "rockBonus",
            ArtifactStatKey::PhysicalBonus => "physicalBonus",
        };
        String::from(temp)
    }
}

impl ArtifactSetKey {
    pub fn to_mona(&self) -> String {
        let same = self.to_string();
        let temp = match self {
            ArtifactSetKey::ArchaicPetra => "archaicPetra",
            ArtifactSetKey::HeartOfDepth => "heartOfDepth",
            ArtifactSetKey::BlizzardStrayer => "blizzardStrayer",
            ArtifactSetKey::RetracingBolide => "retracingBolide",
            ArtifactSetKey::NoblesseOblige => "noblesseOblige",
            ArtifactSetKey::GladiatorsFinale => "gladiatorFinale",
            ArtifactSetKey::MaidenBeloved => "maidenBeloved",
            ArtifactSetKey::ViridescentVenerer => "viridescentVenerer",
            ArtifactSetKey::Lavawalker => "lavaWalker",
            ArtifactSetKey::CrimsonWitchOfFlames => "crimsonWitch",
            ArtifactSetKey::Thundersoother => "thunderSmoother",
            ArtifactSetKey::ThunderingFury => "thunderingFury",
            ArtifactSetKey::BloodstainedChivalry => "bloodstainedChivalry",
            ArtifactSetKey::WanderersTroupe => "wandererTroupe",
            ArtifactSetKey::Scholar => "scholar",
            ArtifactSetKey::Gambler => "gambler",
            ArtifactSetKey::TinyMiracle => "tinyMiracle",
            ArtifactSetKey::MartialArtist => "martialArtist",
            ArtifactSetKey::BraveHeart => "braveHeart",
            ArtifactSetKey::ResolutionOfSojourner => "resolutionOfSojourner",
            ArtifactSetKey::DefenderWill => "defenderWill",
            ArtifactSetKey::Berserker => "berserker",
            ArtifactSetKey::Instructor => "instructor",
            ArtifactSetKey::Exile => "exile",
            ArtifactSetKey::Adventurer => "adventurer",
            ArtifactSetKey::LuckyDog => "luckyDog",
            ArtifactSetKey::TravelingDoctor => "travelingDoctor",
            ArtifactSetKey::PrayersForWisdom => "prayersForWisdom",
            ArtifactSetKey::PrayersToSpringtime => "prayersToSpringtime",
            ArtifactSetKey::PrayersForIllumination => "prayersForIllumination",
            ArtifactSetKey::PrayersForDestiny => "prayersForDestiny",
            ArtifactSetKey::PaleFlame => "paleFlame",
            ArtifactSetKey::TenacityOfTheMillelith => "tenacityOfTheMillelith",
            ArtifactSetKey::EmblemOfSeveredFate => "emblemOfSeveredFate",
            ArtifactSetKey::ShimenawasReminiscence => "shimenawaReminiscence",
            ArtifactSetKey::HuskOfOpulentDreams => "huskOfOpulentDreams",
            ArtifactSetKey::OceanHuedClam => "oceanHuedClam",
            _ => same.as_str(),
        };
        String::from(temp)
    }
}

impl ArtifactSlotKey {
    pub fn to_mona(&self) -> String {
        let temp = match self {
            ArtifactSlotKey::Flower => "flower",
            ArtifactSlotKey::Plume => "feather",
            ArtifactSlotKey::Sands => "sand",
            ArtifactSlotKey::Goblet => "cup",
            ArtifactSlotKey::Circlet => "head",
        };
        String::from(temp)
    }
}

impl Serialize for ArtifactStat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(Some(2))?;
        root.serialize_entry("name", &self.key.to_mona())?;
        let value = match self.key {
            ArtifactStatKey::Atk
            | ArtifactStatKey::ElementalMastery
            | ArtifactStatKey::Hp
            | ArtifactStatKey::Def => self.value,
            _ => self.value / 100.0,
        };
        root.serialize_entry("value", &value)?;
        root.end()
    }
}

impl Serialize for MonaArtifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(Some(7))?;

        root.serialize_entry("setName", &self.set_key.to_mona())?;
        root.serialize_entry("position", &self.slot_key.to_mona())?;
        root.serialize_entry("mainTag", &self.main_stat)?;

        let mut sub_stats: Vec<&ArtifactStat> = vec![];
        if let Some(ref s) = self.sub_stat_1 {
            sub_stats.push(s);
        }
        if let Some(ref s) = self.sub_stat_2 {
            sub_stats.push(s);
        }
        if let Some(ref s) = self.sub_stat_3 {
            sub_stats.push(s);
        }
        if let Some(ref s) = self.sub_stat_4 {
            sub_stats.push(s);
        }
        // let mut subs = serializer.serialize_seq(Some(sub_stats.len()))?;
        //
        // for i in sub_stats {
        //     subs.serialize_element(i);
        // }
        // subs.end();
        // subs.

        root.serialize_entry("normalTags", &sub_stats)?;
        root.serialize_entry("omit", &false)?;
        root.serialize_entry("level", &self.level)?;
        root.serialize_entry("star", &self.rarity)?;

        root.end()
    }
}

pub struct MonaFormat<'a> {
    version: String,
    flower: Vec<&'a MonaArtifact>,
    feather: Vec<&'a MonaArtifact>,
    cup: Vec<&'a MonaArtifact>,
    sand: Vec<&'a MonaArtifact>,
    head: Vec<&'a MonaArtifact>,
}

impl<'a> Serialize for MonaFormat<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(Some(6))?;
        root.serialize_entry("version", &self.version)?;
        root.serialize_entry("flower", &self.flower)?;
        root.serialize_entry("feather", &self.feather)?;
        root.serialize_entry("sand", &self.sand)?;
        root.serialize_entry("cup", &self.cup)?;
        root.serialize_entry("head", &self.head)?;
        root.end()
    }
}

impl<'a> MonaFormat<'a> {
    pub fn new(results: &Vec<InternalArtifact>) -> MonaFormat {
        let mut flower: Vec<&MonaArtifact> = Vec::new();
        let mut feather: Vec<&MonaArtifact> = Vec::new();
        let mut cup: Vec<&MonaArtifact> = Vec::new();
        let mut sand: Vec<&MonaArtifact> = Vec::new();
        let mut head: Vec<&MonaArtifact> = Vec::new();

        for art in results.iter() {
            match art.slot_key {
                ArtifactSlotKey::Flower => flower.push(art),
                ArtifactSlotKey::Plume => feather.push(art),
                ArtifactSlotKey::Sands => sand.push(art),
                ArtifactSlotKey::Goblet => cup.push(art),
                ArtifactSlotKey::Circlet => head.push(art),
            }
        }

        MonaFormat {
            flower,
            feather,
            cup,
            sand,
            head,

            version: String::from("1"),
        }
    }

    pub fn save(&self, path: String) {
        let mut file = match File::create(&path) {
            Err(why) => panic!("couldn't create {}: {}", path, why),
            Ok(file) => file,
        };
        let s = serde_json::to_string(&self).unwrap();

        match file.write_all(s.as_bytes()) {
            Err(why) => panic!("couldn't write to {}: {}", path, why),
            _ => {}
        }
    }
}
