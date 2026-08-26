use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RegisterStart {
    pub username: String,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct BlobReply {
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct RegisterFinish {
    pub username: String,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginStart {
    pub username: String,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginReply {
    pub id: String,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginFinish {
    pub id: String,
    pub message: String,
    pub device: Option<String>,
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Sealed {
    pub data: String,
}

#[derive(Serialize, Deserialize)]
pub struct Grant {
    pub token: String,
    pub device: String,
}

#[derive(Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub created: String,
    pub revoked: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Me {
    pub username: String,
    pub device: Device,
    pub secrets: u32,
    pub files: u32,
    pub devices: u32,
}

#[derive(Serialize, Deserialize)]
pub struct SecretPut {
    pub data: String,
}

#[derive(Serialize, Deserialize)]
pub struct SecretItem {
    pub name: String,
    pub updated: String,
}

#[derive(Serialize, Deserialize)]
pub struct SecretValue {
    pub data: String,
}

#[derive(Serialize, Deserialize)]
pub struct FileItem {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub hash: String,
    pub created: String,
    pub updated: String,
}
