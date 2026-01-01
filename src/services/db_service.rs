use crate::config::settings::MongoConfig;
use crate::{Error, Result};
use chrono::Utc;
use mongodb::{
    bson::{doc, Bson, Document},
    options::{ClientOptions, ServerApi, ServerApiVersion},
    Client, Database,
};
use once_cell::sync::OnceCell;
use std::sync::Arc;
use tokio::sync::Mutex;

static DB_INSTANCE: OnceCell<Arc<Mutex<Database>>> = OnceCell::new();

pub async fn initialize_db(config: &MongoConfig) -> Result<Client> {
    if DB_INSTANCE.get().is_some() {
        return Err(Error::Database("Database already initialized".to_string()));
    }

    let mut uri = format!("mongodb://{}:{}", config.host, config.port);

    if let (Some(user), Some(pass)) = (&config.user, &config.password) {
        uri = format!(
            "mongodb://{}:{}@{}:{}",
            user, pass, config.host, config.port
        );
    }

    // 创建客户端
    let mut client_options =
        ClientOptions::parse(uri).await.map_err(|e| Error::Database(e.to_string()))?;

    // 设置ServerAPI版本
    let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
    client_options.server_api = Some(server_api);
    
    // 优化连接池 (默认是100，对于个人项目太大了)
    client_options.min_pool_size = Some(0);
    client_options.max_pool_size = Some(10);
    
    // 禁用副本集检测，直接连接（适用于单机 MongoDB）
    client_options.direct_connection = Some(true);
    
    // 设置连接超时（避免长时间等待）
    client_options.connect_timeout = Some(std::time::Duration::from_secs(5));
    client_options.server_selection_timeout = Some(std::time::Duration::from_secs(5));

    let client = Client::with_options(client_options).map_err(|e| Error::Database(e.to_string()))?;

    // 获取数据库
    let database = client.database(&config.database);

    // 测试连接
    database
        .run_command(doc! { "ping": 1 })
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    println!("✅ 成功连接到MongoDB数据库");

    let db_arc = Arc::new(Mutex::new(database));
    DB_INSTANCE
        .set(db_arc)
        .expect("Failed to set database instance");

    Ok(client)
}

pub async fn get_db() -> Result<Arc<Mutex<Database>>> {
    DB_INSTANCE
        .get()
        .cloned()
        .ok_or_else(|| Error::Database("Database not initialized".to_string()))
}

pub async fn find_one(collection_name: &str, filter: Document) -> Result<Option<Document>> {
    let db = get_db().await?;
    let db_lock = db.lock().await;

    let collection = db_lock.collection::<Document>(collection_name);
    let opt = collection
        .find_one(filter)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 规范化返回中的日期字段为 ISO 字符串
    let normalized = opt.map(|d| normalize_document_dates(d));
    Ok(normalized)
}

pub async fn find_many(collection_name: &str, filter: Document) -> Result<Vec<Document>> {
    let db = get_db().await?;
    let db_lock = db.lock().await;

    let collection = db_lock.collection::<Document>(collection_name);

    let mut cursor = collection
        .find(filter)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut results = Vec::new();

    while cursor
        .advance()
        .await
        .map_err(|e| Error::Database(e.to_string()))?
    {
        let doc = cursor
            .deserialize_current()
            .map_err(|e| Error::Database(e.to_string()))?;
        results.push(normalize_document_dates(doc));
    }

    Ok(results)
}

pub async fn insert_one(collection_name: &str, document: Document) -> Result<String> {
    let db = get_db().await?;
    let db_lock = db.lock().await;

    let collection = db_lock.collection::<Document>(collection_name);

    let result = collection
        .insert_one(document)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| Error::Database("Failed to get inserted ID".to_string()))?
        .to_hex())
}

pub async fn update_one(collection_name: &str, filter: Document, update: Document) -> Result<u64> {
    let db = get_db().await?;
    let db_lock = db.lock().await;

    let collection = db_lock.collection::<Document>(collection_name);

    let result = collection
        .update_one(filter, update)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(result.modified_count)
}

pub async fn delete_one(collection_name: &str, filter: Document) -> Result<u64> {
    let db = get_db().await?;
    let db_lock = db.lock().await;

    let collection = db_lock.collection::<Document>(collection_name);

    let result = collection
        .delete_one(filter)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(result.deleted_count)
}

// 将 Document 中的 BSON 日期或扩展 JSON 日期转换为 ISO 字符串（递归）
fn normalize_document_dates(doc: Document) -> Document {
    fn normalize_bson(value: Bson) -> Bson {
        match value {
            Bson::Document(d) => {
                // 检查扩展 JSON 的 {"$date": {"$numberLong": "..."}} 形式
                if let Some(inner) = d.get("$date") {
                    // 处理多种可能的内层类型
                    match inner {
                        Bson::Document(inner_doc) => {
                            if let Some(Bson::String(num_str)) = inner_doc.get("$numberLong") {
                                if let Ok(ms) = num_str.parse::<i64>() {
                                    if let Some(dt) =
                                        chrono::DateTime::<Utc>::from_timestamp_millis(ms)
                                    {
                                        return Bson::String(dt.to_rfc3339());
                                    }
                                }
                            }
                            // 如果无法解析，递归处理 inner_doc
                            let mut new_inner = inner_doc.clone();
                            for (k, v) in inner_doc.iter() {
                                new_inner.insert(k.clone(), normalize_bson(v.clone()));
                            }
                            return Bson::Document(new_inner);
                        }
                        Bson::Int64(ms) => {
                            if let Some(dt) = chrono::DateTime::<Utc>::from_timestamp_millis(*ms) {
                                return Bson::String(dt.to_rfc3339());
                            }
                        }
                        Bson::String(s) => {
                            // 字符串可能是 ISO 或数字字符串
                            if let Ok(ms) = s.parse::<i64>() {
                                if let Some(dt) = chrono::DateTime::<Utc>::from_timestamp_millis(ms)
                                {
                                    return Bson::String(dt.to_rfc3339());
                                }
                            }
                            return Bson::String(s.clone());
                        }
                        Bson::DateTime(dt) => {
                            return Bson::String(
                                chrono::DateTime::<Utc>::from(dt.to_system_time()).to_rfc3339(),
                            );
                        }
                        _ => {}
                    }
                }

                // 否则递归处理子文档
                let mut new_doc = Document::new();
                for (k, v) in d.into_iter() {
                    new_doc.insert(k, normalize_bson(v));
                }
                Bson::Document(new_doc)
            }
            Bson::Array(arr) => Bson::Array(arr.into_iter().map(normalize_bson).collect()),
            Bson::DateTime(dt) => {
                Bson::String(chrono::DateTime::<Utc>::from(dt.to_system_time()).to_rfc3339())
            }
            other => other,
        }
    }

    let mut new = Document::new();
    for (k, v) in doc.into_iter() {
        new.insert(k, normalize_bson(v));
    }
    new
}
