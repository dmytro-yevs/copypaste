use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: impl Into<String>, data: serde_json::Value) -> Self {
        Self { id: id.into(), ok: true, data: Some(data), error: None }
    }

    pub fn err(id: impl Into<String>, msg: impl Into<String>) -> Self {
        Self { id: id.into(), ok: false, data: None, error: Some(msg.into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_deserializes() {
        let json = r#"{"id":"1","method":"list","params":{"limit":10,"offset":0}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "list");
        assert_eq!(req.id, "1");
    }

    #[test]
    fn response_ok_serializes() {
        let resp = Response::ok("1", serde_json::json!({"total": 0, "items": []}));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(!s.contains("\"error\""));
    }

    #[test]
    fn response_err_serializes() {
        let resp = Response::err("2", "not found");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("\"error\":\"not found\""));
        assert!(!s.contains("\"data\""));
    }
}
