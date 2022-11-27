use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyerLoginData {
    pub hmac: Vec<u8>,
    pub passcode: String,
}

impl Default for BuyerLoginData {
    fn default() -> Self {
        Self {
            hmac: vec![],
            passcode: "".to_string(),
        }
    }
}

impl Into<JsValue> for BuyerLoginData {
    fn into(self) -> wasm_bindgen::JsValue {
        JsValue::from_str(&serde_json::to_string(&self).unwrap_or_default())
    }
}

impl From<JsValue> for BuyerLoginData {
    fn from(val: wasm_bindgen::JsValue) -> Self {
        // serde_wasm_bindgen::from_value(val).unwrap_or_default()
        serde_json::from_str(&val.as_string().unwrap_or_default()).unwrap_or_default()
    }
}