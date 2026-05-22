use crate::ipc::IpcClient;
use crate::paths::socket_path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipItem {
    pub id: String,
    pub content_type: String,
    pub wall_time: i64,
    pub snippet: String,
    pub is_sensitive: bool,
}

#[derive(Debug, Serialize)]
pub struct ListResult {
    pub items: Vec<ClipItem>,
    pub total: u64,
}

#[derive(Debug, Serialize)]
pub struct AppError {
    pub message: String,
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError { message: e.to_string() }
    }
}

#[tauri::command]
pub fn list_items(limit: u64) -> Result<ListResult, AppError> {
    let mut client = IpcClient::connect(&socket_path()).map_err(AppError::from)?;
    let req = serde_json::json!({
        "id": "1",
        "method": "list",
        "params": {"limit": limit, "offset": 0}
    });
    let resp = client.call(&req).map_err(AppError::from)?;
    if !resp.ok {
        return Err(AppError { message: resp.error.unwrap_or_else(|| "list failed".into()) });
    }
    let data = resp.data.unwrap_or(serde_json::Value::Null);
    let total = data["total"].as_u64().unwrap_or(0);
    let items: Vec<ClipItem> = data["items"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    Ok(ListResult { items, total })
}

#[tauri::command]
pub fn search_items(query: String, limit: u64) -> Result<ListResult, AppError> {
    let mut client = IpcClient::connect(&socket_path()).map_err(AppError::from)?;
    let req = serde_json::json!({
        "id": "2",
        "method": "search",
        "params": {"query": query, "limit": limit}
    });
    let resp = client.call(&req).map_err(AppError::from)?;
    if !resp.ok {
        return Err(AppError { message: resp.error.unwrap_or_else(|| "search failed".into()) });
    }
    let data = resp.data.unwrap_or(serde_json::Value::Null);
    let total = data["total"].as_u64().unwrap_or(0);
    let items: Vec<ClipItem> = data["items"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    Ok(ListResult { items, total })
}

#[tauri::command]
pub fn delete_item(id: String) -> Result<(), AppError> {
    let mut client = IpcClient::connect(&socket_path()).map_err(AppError::from)?;
    let req = serde_json::json!({
        "id": "3",
        "method": "delete",
        "params": {"id": id}
    });
    let resp = client.call(&req).map_err(AppError::from)?;
    if !resp.ok {
        return Err(AppError { message: resp.error.unwrap_or_else(|| "delete failed".into()) });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_item_deserializes_fully() {
        let json = serde_json::json!({
            "id": "abc-123",
            "content_type": "text/plain",
            "wall_time": 1716000000000i64,
            "snippet": "hello world",
            "is_sensitive": false
        });
        let item: ClipItem = serde_json::from_value(json).unwrap();
        assert_eq!(item.id, "abc-123");
        assert_eq!(item.content_type, "text/plain");
        assert_eq!(item.snippet, "hello world");
        assert!(!item.is_sensitive);
    }

    #[test]
    fn clip_item_sensitive_flag() {
        let json = serde_json::json!({
            "id": "xyz",
            "content_type": "text/plain",
            "wall_time": 0i64,
            "snippet": "••••",
            "is_sensitive": true
        });
        let item: ClipItem = serde_json::from_value(json).unwrap();
        assert!(item.is_sensitive);
    }

    #[test]
    fn app_error_from_anyhow() {
        let err = anyhow::anyhow!("something went wrong");
        let app_err = AppError::from(err);
        assert_eq!(app_err.message, "something went wrong");
    }
}
