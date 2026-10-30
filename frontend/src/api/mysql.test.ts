import { beforeEach, describe, expect, it, vi } from "vitest";

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("./client", () => ({ default: { get } }));

import {
  getMySqlDatabases,
  getMySqlRows,
  getMySqlTableSchema,
  getMySqlTables,
} from "./mysql";

beforeEach(() => {
  get.mockReset();
  get.mockResolvedValue({ data: {} });
});

describe("MySQL API", () => {
  it("uses the MySQL metadata endpoints and encodes identifiers", async () => {
    await getMySqlDatabases();
    await getMySqlTables("sales archive");
    await getMySqlTableSchema("sales archive", "order/items");

    expect(get).toHaveBeenNthCalledWith(1, "/mysql/databases");
    expect(get).toHaveBeenNthCalledWith(2, "/mysql/databases/sales%20archive/tables");
    expect(get).toHaveBeenNthCalledWith(
      3,
      "/mysql/databases/sales%20archive/tables/order%2Fitems/schema"
    );
  });

  it("passes bounded-page inputs and omits an empty filter", async () => {
    await getMySqlRows("app", "users", 3, 50);

    expect(get).toHaveBeenCalledWith(
      "/mysql/databases/app/tables/users/rows",
      { params: { page: 3, limit: 50, filter: undefined } }
    );
  });

  it("passes a text filter as a query parameter", async () => {
    await getMySqlRows("app", "users", 1, 25, "alice");

    expect(get).toHaveBeenCalledWith(
      "/mysql/databases/app/tables/users/rows",
      { params: { page: 1, limit: 25, filter: "alice" } }
    );
  });
});
