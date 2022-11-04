use crate::scanner::yas_scanner::YasScannerConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ConfigNotifyData {
    pub config: YasScannerConfig,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ScanReqData {
    pub argv: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ScanRspData {
    pub success: bool,
    pub message: String,
    pub good_json: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct LockReqData {
    pub argv: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct LockRspData {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "cmd", content = "data")]
pub enum Packet {
    ConfigNotify(ConfigNotifyData),
    ScanReq(ScanReqData),
    ScanRsp(ScanRspData),
    LockReq(LockReqData),
    LockRsp(LockRspData),
}
