import api from "./client";

export interface MySqlDatabaseList {
  databases: string[];
}

export interface MySqlTableList {
  tables: string[];
}

export interface MySqlColumn {
  name: string;
  data_type: string;
  column_type: string;
  nullable: boolean;
  key: string | null;
  default: unknown;
  extra: string;
}

export interface MySqlTableSchema {
  columns: MySqlColumn[];
}

export interface MySqlRowPage {
  rows: Record<string, unknown>[];
  total: number;
  page: number;
  limit: number;
}

export const getMySqlDatabases = () =>
  api.get<MySqlDatabaseList>("/mysql/databases").then((response) => response.data);

export const getMySqlTables = (database: string) =>
  api
    .get<MySqlTableList>(`/mysql/databases/${encodeURIComponent(database)}/tables`)
    .then((response) => response.data);

export const getMySqlTableSchema = (database: string, table: string) =>
  api
    .get<MySqlTableSchema>(
      `/mysql/databases/${encodeURIComponent(database)}/tables/${encodeURIComponent(table)}/schema`
    )
    .then((response) => response.data);

export const getMySqlRows = (
  database: string,
  table: string,
  page = 1,
  limit = 50,
  filter = ""
) =>
  api
    .get<MySqlRowPage>(
      `/mysql/databases/${encodeURIComponent(database)}/tables/${encodeURIComponent(table)}/rows`,
      { params: { page, limit, filter: filter || undefined } }
    )
    .then((response) => response.data);
