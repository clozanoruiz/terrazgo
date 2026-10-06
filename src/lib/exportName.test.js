// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The name decides whether an export can be handed back to the app later, so
// it is logic rather than presentation: a second export on one day that reuses
// a name gets renamed by the Android picker to something the import filter
// will not show. See exportName.js for the two measured renamings.

import { describe, expect, it } from "vitest";

import { exportFileName, exportStamp } from "./exportName.js";

const AT = (iso) => new Date(iso);

describe("exportStamp", () => {
  it("is compact UTC to the second", () => {
    expect(exportStamp(AT("2026-09-05T17:04:09Z"))).toBe("20260905T170409");
  });

  it("differs for two exports on the same day", () => {
    const morning = exportStamp(AT("2026-09-05T08:00:00Z"));
    const evening = exportStamp(AT("2026-09-05T20:00:00Z"));
    expect(morning).not.toBe(evening);
  });
});

describe("exportFileName", () => {
  it("joins the fields with underscores and keeps the extension last", () => {
    expect(exportFileName(["cuaderno", "2025-2026", "es"], "pdf", AT("2026-09-05T17:04:09Z"))).toBe(
      "cuaderno_2025-2026_es_20260905T170409.pdf",
    );
  });

  it("drops empty parts, so an absent language needs no conditional", () => {
    expect(
      exportFileName(["cuaderno", "2025-2026", null], "xlsx", AT("2026-09-05T17:04:09Z")),
    ).toBe("cuaderno_2025-2026_20260905T170409.xlsx");
  });

  it("sanitises a season label, which arrives with a slash", () => {
    expect(exportFileName(["cuaderno", "2025/2026"], "pdf", AT("2026-09-05T17:04:09Z"))).toBe(
      "cuaderno_2025-2026_20260905T170409.pdf",
    );
  });

  it("never proposes the same name twice in a day", () => {
    // The failure this guards: two same-day exports collide, and the SAF
    // picker renames the second to "….db (1)" — which no longer ends in .db,
    // so the import dialog's filter hides it and the backup cannot be restored
    // through the app.
    const first = exportFileName(["terrazgo-backup"], "db", AT("2026-09-05T09:15:00Z"));
    const second = exportFileName(["terrazgo-backup"], "db", AT("2026-09-05T18:42:11Z"));
    expect(first).not.toBe(second);
    expect(first.endsWith(".db")).toBe(true);
    expect(second.endsWith(".db")).toBe(true);
  });
});
