import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import { useTranslation } from "react-i18next";
import {
  getMySqlRows,
  getMySqlTableSchema,
  type MySqlColumn,
  type MySqlRowPage,
} from "../api/mysql";
import { useThemeMode } from "../context/useThemeMode";

interface Props {
  db: string;
  table: string;
  visible: boolean;
}

const PAGE_SIZE = 50;
const FONT = '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';

function errorMessage(error: unknown): string {
  return (
    (error as { response?: { data?: { error?: string } } }).response?.data?.error ??
    (error as Error).message ??
    "MySQL request failed"
  );
}

function displayValue(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

export default function MySqlTableView({ db, table, visible }: Props) {
  const { t } = useTranslation();
  const { mode } = useThemeMode();
  const dark = mode === "dark";
  const [schema, setSchema] = useState<MySqlColumn[]>([]);
  const [data, setData] = useState<MySqlRowPage>({ rows: [], total: 0, page: 1, limit: PAGE_SIZE });
  const [page, setPage] = useState(1);
  const [filterInput, setFilterInput] = useState("");
  const [filter, setFilter] = useState("");
  const [view, setView] = useState<"rows" | "schema">("rows");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    setPage(1);
    setFilterInput("");
    setFilter("");
    setView("rows");
    setData({ rows: [], total: 0, page: 1, limit: PAGE_SIZE });
    setSchema([]);
  }, [db, table]);

  useEffect(() => {
    if (!visible || !db || !table) return;
    let active = true;
    getMySqlTableSchema(db, table)
      .then((result) => {
        if (active) setSchema(result.columns);
      })
      .catch((requestError) => {
        if (active) setError(errorMessage(requestError));
      });
    return () => {
      active = false;
    };
  }, [db, table, refreshKey, visible]);

  useEffect(() => {
    if (!visible || !db || !table) return;
    let active = true;
    setLoading(true);
    setError("");
    getMySqlRows(db, table, page, PAGE_SIZE, filter)
      .then((result) => {
        if (active) setData(result);
      })
      .catch((requestError) => {
        if (active) setError(errorMessage(requestError));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [db, table, page, filter, refreshKey, visible]);

  const columns = useMemo(() => {
    if (schema.length > 0) return schema.map((column) => column.name);
    return Object.keys(data.rows[0] ?? {});
  }, [schema, data.rows]);
  const totalPages = Math.max(1, Math.ceil(data.total / data.limit));
  const border = dark ? "#334155" : "#dbe3ee";
  const surface = dark ? "#1e293b" : "#ffffff";
  const background = dark ? "#111827" : "#f8fafc";
  const text = dark ? "#e2e8f0" : "#1f2937";
  const muted = dark ? "#94a3b8" : "#64748b";

  const applyFilter = (event: FormEvent) => {
    event.preventDefault();
    setPage(1);
    setFilter(filterInput.trim());
  };

  if (!db || !table) return null;

  return (
    <section
      aria-busy={loading}
      aria-label={t("mysql.tableLabel", { database: db, table })}
      style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0, background, color: text, fontFamily: FONT }}
    >
      <header style={{ padding: "14px 18px", borderBottom: `1px solid ${border}`, background: surface, display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap" }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontSize: 11, color: muted, textTransform: "uppercase", letterSpacing: "0.08em" }}>{t("mysql.readOnly")}</div>
          <h2 style={{ margin: "2px 0 0", fontSize: 18, overflow: "hidden", textOverflow: "ellipsis" }}>{db}.{table}</h2>
        </div>
        <div role="tablist" aria-label={t("mysql.views")} style={{ display: "flex", gap: 4 }}>
          {([ ["rows", t("tabs.documents")], ["schema", t("tabs.schema")] ] as const).map(([id, label]) => (
            <button
              key={id}
              role="tab"
              aria-selected={view === id}
              onClick={() => setView(id)}
              style={{ border: `1px solid ${border}`, borderRadius: 6, padding: "6px 10px", cursor: "pointer", background: view === id ? "#2563eb" : surface, color: view === id ? "#fff" : text }}
            >
              {label}
            </button>
          ))}
        </div>
        <button
          onClick={() => setRefreshKey((value) => value + 1)}
          style={{ marginLeft: "auto", border: `1px solid ${border}`, borderRadius: 6, padding: "6px 10px", cursor: "pointer", background: surface, color: text }}
        >
          {t("buttons.refresh")}
        </button>
      </header>

      {error && <div role="alert" style={{ padding: "10px 18px", background: dark ? "#451a1a" : "#fef2f2", color: dark ? "#fecaca" : "#b91c1c", borderBottom: `1px solid ${border}` }}>{error}</div>}

      {view === "rows" ? (
        <>
          <form onSubmit={applyFilter} style={{ padding: "10px 18px", display: "flex", gap: 8, borderBottom: `1px solid ${border}`, background: surface }}>
            <label htmlFor={`mysql-filter-${db}-${table}`} className="sr-only">{t("mysql.filterRows")}</label>
            <input
              id={`mysql-filter-${db}-${table}`}
              value={filterInput}
              onChange={(event) => setFilterInput(event.target.value)}
              placeholder={t("mysql.searchRows")}
              style={{ flex: 1, minWidth: 100, border: `1px solid ${border}`, borderRadius: 6, padding: "7px 10px", background, color: text }}
            />
            <button type="submit" style={{ border: 0, borderRadius: 6, padding: "7px 14px", background: "#2563eb", color: "#fff", cursor: "pointer" }}>{t("mysql.applyFilter")}</button>
            {filter && <button type="button" onClick={() => { setFilterInput(""); setFilter(""); setPage(1); }} style={{ border: `1px solid ${border}`, borderRadius: 6, padding: "7px 12px", background: surface, color: text, cursor: "pointer" }}>{t("mysql.clearFilter")}</button>}
          </form>
          <div style={{ flex: 1, overflow: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
              <thead style={{ position: "sticky", top: 0, zIndex: 1, background: dark ? "#273449" : "#eef2f7" }}>
                <tr>{columns.map((column) => <th key={column} scope="col" style={{ textAlign: "left", padding: "9px 12px", borderBottom: `1px solid ${border}`, whiteSpace: "nowrap" }}>{column}</th>)}</tr>
              </thead>
              <tbody>
                {data.rows.map((row, index) => (
                  <tr key={index} style={{ background: index % 2 ? (dark ? "#172033" : "#f8fafc") : surface }}>
                    {columns.map((column) => {
                      const value = row[column];
                      return <td key={column} title={displayValue(value)} style={{ padding: "8px 12px", borderBottom: `1px solid ${border}`, maxWidth: 360, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", color: value === null ? muted : text }}>{displayValue(value)}</td>;
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
            {!loading && data.rows.length === 0 && <p role="status" style={{ color: muted, padding: 24, textAlign: "center" }}>{t("mysql.noRows")}</p>}
          </div>
          <footer style={{ padding: "9px 18px", borderTop: `1px solid ${border}`, background: surface, display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
            <span style={{ color: muted, fontSize: 12 }}>{t("mysql.rowCount", { total: data.total.toLocaleString(), page, pages: totalPages })}</span>
            <div style={{ display: "flex", gap: 6 }}>
              <button aria-label={t("pagination.button.prev")} disabled={page <= 1 || loading} onClick={() => setPage((value) => Math.max(1, value - 1))} style={{ border: `1px solid ${border}`, borderRadius: 5, padding: "5px 9px", background: surface, color: text, cursor: page <= 1 ? "not-allowed" : "pointer" }}>{t("pagination.button.prev")}</button>
              <button aria-label={t("pagination.button.next")} disabled={page >= totalPages || loading} onClick={() => setPage((value) => Math.min(totalPages, value + 1))} style={{ border: `1px solid ${border}`, borderRadius: 5, padding: "5px 9px", background: surface, color: text, cursor: page >= totalPages ? "not-allowed" : "pointer" }}>{t("pagination.button.next")}</button>
            </div>
          </footer>
        </>
      ) : (
        <div style={{ flex: 1, overflow: "auto", padding: 18 }}>
          <table style={{ width: "100%", borderCollapse: "collapse", background: surface, fontSize: 13 }}>
            <thead><tr>{["column", "type", "nullable", "key", "default", "extra"].map((heading) => <th key={heading} scope="col" style={{ textAlign: "left", padding: "9px 12px", borderBottom: `1px solid ${border}` }}>{t(`mysql.schema.${heading}`)}</th>)}</tr></thead>
            <tbody>{schema.map((column) => <tr key={column.name}>{[column.name, column.column_type || column.data_type, column.nullable ? "YES" : "NO", column.key ?? "", displayValue(column.default), column.extra].map((value, index) => <td key={index} style={{ padding: "8px 12px", borderBottom: `1px solid ${border}` }}>{value}</td>)}</tr>)}</tbody>
          </table>
        </div>
      )}
    </section>
  );
}
