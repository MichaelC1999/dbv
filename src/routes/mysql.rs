use axum::{
    Json,
    extract::{Path, Query, State},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::{MySqlPool, Row, mysql::MySqlRow};

use crate::{auth::rbac::ReadAccess, errors::AppError, state::AppState};

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

#[derive(Serialize)]
pub struct DatabasesResponse {
    databases: Vec<String>,
}

#[derive(Serialize)]
pub struct TablesResponse {
    tables: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnInfo {
    name: String,
    data_type: String,
    column_type: String,
    nullable: bool,
    key: Option<String>,
    default: Option<String>,
    extra: String,
}

#[derive(Serialize)]
pub struct SchemaResponse {
    columns: Vec<ColumnInfo>,
}

#[derive(Deserialize)]
pub struct RowsQuery {
    page: Option<u32>,
    limit: Option<u32>,
    filter: Option<String>,
}

#[derive(Serialize)]
pub struct RowsResponse {
    rows: Vec<Map<String, Value>>,
    total: u64,
    page: u32,
    limit: u32,
}

fn pool(state: &AppState) -> Result<&MySqlPool, AppError> {
    state.mysql.as_ref().ok_or_else(|| {
        AppError::Unavailable("MySQL is not configured; set MYSQL_URL to enable it".into())
    })
}

pub async fn list_databases(
    State(state): State<AppState>,
    _access: ReadAccess,
) -> Result<Json<DatabasesResponse>, AppError> {
    let databases = sqlx::query_scalar::<_, String>(
        "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA \
         WHERE SCHEMA_NAME NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys') \
         ORDER BY SCHEMA_NAME",
    )
    .fetch_all(pool(&state)?)
    .await?;
    Ok(Json(DatabasesResponse { databases }))
}

pub async fn list_tables(
    State(state): State<AppState>,
    _access: ReadAccess,
    Path(db): Path<String>,
) -> Result<Json<TablesResponse>, AppError> {
    validate_identifier(&db)?;
    ensure_database(pool(&state)?, &db).await?;
    let tables = sqlx::query_scalar::<_, String>(
        "SELECT TABLE_NAME FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = ? AND TABLE_TYPE IN ('BASE TABLE', 'VIEW') ORDER BY TABLE_NAME",
    )
    .bind(&db)
    .fetch_all(pool(&state)?)
    .await?;
    Ok(Json(TablesResponse { tables }))
}

pub async fn table_schema(
    State(state): State<AppState>,
    _access: ReadAccess,
    Path((db, table)): Path<(String, String)>,
) -> Result<Json<SchemaResponse>, AppError> {
    let columns = load_columns(pool(&state)?, &db, &table).await?;
    Ok(Json(SchemaResponse { columns }))
}

pub async fn table_rows(
    State(state): State<AppState>,
    _access: ReadAccess,
    Path((db, table)): Path<(String, String)>,
    Query(params): Query<RowsQuery>,
) -> Result<Json<RowsResponse>, AppError> {
    let pool = pool(&state)?;
    let columns = load_columns(pool, &db, &table).await?;
    let (page, limit, offset) = pagination(params.page, params.limit);
    let textual: Vec<&ColumnInfo> = columns
        .iter()
        .filter(|c| is_textual(&c.data_type))
        .collect();
    let filter = params
        .filter
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let (where_sql, pattern) = build_filter(&textual, filter);

    let count_sql = format!(
        "SELECT COUNT(*) FROM {}.{}{}",
        quote_identifier(&db)?,
        quote_identifier(&table)?,
        where_sql
    );
    let mut count_query = sqlx::query_scalar::<_, u64>(&count_sql);
    if let Some(pattern) = &pattern {
        for _ in &textual {
            count_query = count_query.bind(pattern);
        }
    }
    let total = count_query.fetch_one(pool).await?;

    let select_list = columns
        .iter()
        .map(select_expression)
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let rows_sql = format!(
        "SELECT {select_list} FROM {}.{}{} LIMIT ? OFFSET ?",
        quote_identifier(&db)?,
        quote_identifier(&table)?,
        where_sql
    );
    let mut rows_query = sqlx::query(&rows_sql);
    if let Some(pattern) = &pattern {
        for _ in &textual {
            rows_query = rows_query.bind(pattern);
        }
    }
    let raw_rows = rows_query
        .bind(u64::from(limit))
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let rows = raw_rows
        .iter()
        .map(|row| serialize_row(row, &columns))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(RowsResponse {
        rows,
        total,
        page,
        limit,
    }))
}

async fn ensure_database(pool: &MySqlPool, db: &str) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?)",
    )
    .bind(db)
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("MySQL database '{db}'")))
    }
}

async fn load_columns(
    pool: &MySqlPool,
    db: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, AppError> {
    validate_identifier(db)?;
    validate_identifier(table)?;
    ensure_database(pool, db).await?;
    let rows = sqlx::query(
        "SELECT COLUMN_NAME, DATA_TYPE, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, \
         COLUMN_DEFAULT, EXTRA FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
    )
    .bind(db)
    .bind(table)
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Err(AppError::NotFound(format!("MySQL table '{db}.{table}'")));
    }
    rows.into_iter()
        .map(|row| {
            let name: String = row.get("COLUMN_NAME");
            validate_identifier(&name)?;
            Ok(ColumnInfo {
                name,
                data_type: row.get("DATA_TYPE"),
                column_type: row.get("COLUMN_TYPE"),
                nullable: row.get::<String, _>("IS_NULLABLE") == "YES",
                key: nonempty(row.get("COLUMN_KEY")),
                default: row.get("COLUMN_DEFAULT"),
                extra: row.get("EXTRA"),
            })
        })
        .collect()
}

fn nonempty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn pagination(page: Option<u32>, limit: Option<u32>) -> (u32, u32, u64) {
    let page = page.unwrap_or(1).max(1);
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    (page, limit, u64::from(page - 1) * u64::from(limit))
}

fn validate_identifier(identifier: &str) -> Result<(), AppError> {
    if identifier.is_empty()
        || identifier.chars().count() > 64
        || identifier.chars().any(char::is_control)
    {
        return Err(AppError::BadRequest("invalid MySQL identifier".into()));
    }
    Ok(())
}

fn quote_identifier(identifier: &str) -> Result<String, AppError> {
    validate_identifier(identifier)?;
    Ok(format!("`{}`", identifier.replace('`', "``")))
}

fn is_textual(data_type: &str) -> bool {
    matches!(
        data_type,
        "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum" | "set"
    )
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn quote_metadata_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
}

fn build_filter(columns: &[&ColumnInfo], filter: Option<&str>) -> (String, Option<String>) {
    let Some(filter) = filter.filter(|_| !columns.is_empty()) else {
        return (String::new(), None);
    };
    let predicates = columns
        .iter()
        .map(|column| {
            format!(
                "LOWER({}) LIKE LOWER(?) ESCAPE '\\\\'",
                quote_metadata_identifier(&column.name)
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    (
        format!(" WHERE ({predicates})"),
        Some(format!("%{}%", escape_like(filter))),
    )
}

fn select_expression(column: &ColumnInfo) -> Result<String, AppError> {
    let name = quote_identifier(&column.name)?;
    if matches!(
        column.data_type.as_str(),
        "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" | "bit"
    ) {
        Ok(format!("TO_BASE64({name}) AS {name}"))
    } else {
        Ok(format!("CAST({name} AS CHAR) AS {name}"))
    }
}

fn serialize_row(row: &MySqlRow, columns: &[ColumnInfo]) -> Result<Map<String, Value>, AppError> {
    let mut result = Map::new();
    for column in columns {
        let raw: Option<String> = row.try_get(column.name.as_str())?;
        let value = raw.map_or(Value::Null, |raw| value_from_string(&raw, column));
        result.insert(column.name.clone(), value);
    }
    Ok(result)
}

fn value_from_string(raw: &str, column: &ColumnInfo) -> Value {
    match column.data_type.as_str() {
        "tinyint"
            if column
                .column_type
                .to_ascii_lowercase()
                .starts_with("tinyint(1)") =>
        {
            json!(raw != "0")
        }
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint"
            if column.column_type.to_ascii_lowercase().contains("unsigned") =>
        {
            raw.parse::<u64>().map_or_else(|_| json!(raw), |v| json!(v))
        }
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" | "year" => {
            raw.parse::<i64>().map_or_else(|_| json!(raw), |v| json!(v))
        }
        "float" | "double" | "real" => raw
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map_or_else(|| json!(raw), |v| json!(v)),
        "decimal" | "numeric" => raw
            .parse::<Decimal>()
            .map_or_else(|_| json!(raw), |v| json!(v.to_string())),
        "json" => serde_json::from_str(raw).unwrap_or_else(|_| json!(raw)),
        _ => json!(raw),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            data_type: "varchar".into(),
            column_type: "varchar(255)".into(),
            nullable: true,
            key: None,
            default: None,
            extra: String::new(),
        }
    }

    #[test]
    fn pagination_is_one_based_and_bounded() {
        assert_eq!(pagination(None, None), (1, 50, 0));
        assert_eq!(pagination(Some(0), Some(0)), (1, 1, 0));
        assert_eq!(pagination(Some(3), Some(999)), (3, 200, 400));
    }

    #[test]
    fn identifiers_are_validated_and_quoted() {
        assert_eq!(quote_identifier("odd`name").unwrap(), "`odd``name`");
        assert!(quote_identifier("").is_err());
        assert!(quote_identifier("bad\nname").is_err());
        assert!(quote_identifier(&"a".repeat(65)).is_err());
    }

    #[test]
    fn filter_builder_uses_placeholders_and_escapes_wildcards() {
        let a = column("name");
        let b = column("description");
        let (sql, pattern) = build_filter(&[&a, &b], Some("50%_off"));
        assert_eq!(sql.matches('?').count(), 2);
        assert!(!sql.contains("50%_off"));
        assert_eq!(pattern.as_deref(), Some("%50\\%\\_off%"));
    }

    #[test]
    fn filter_without_text_columns_is_ignored() {
        assert_eq!(build_filter(&[], Some("text")), (String::new(), None));
    }
}
