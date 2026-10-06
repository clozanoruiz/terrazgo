// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Which licence a library is shown under. A dual licence is a choice, and the
// About panel carries only the option taken — so the rule that takes it is what
// decides whether the panel's attribution is complete.
import { describe, expect, it } from "vitest";

import { THIRD_PARTY, licenceShown } from "./thirdParty.js";

const row = (...licences) => ({ name: "example", licences });

describe("licenceShown", () => {
  it("shows a single licence as it is", () => {
    expect(licenceShown(row("Apache-2.0"))).toBe("Apache-2.0");
    expect(licenceShown(row("blessing"))).toBe("blessing");
  });

  it("takes MIT from a choice, whichever order the package states it in", () => {
    // Serde says "MIT OR Apache-2.0"; Tauri and uuid say "Apache-2.0 OR MIT".
    expect(licenceShown(row("MIT", "Apache-2.0"))).toBe("MIT");
    expect(licenceShown(row("Apache-2.0", "MIT"))).toBe("MIT");
  });

  it("takes MIT over the Unlicense too", () => {
    // csv and jiff: "Unlicense OR MIT".
    expect(licenceShown(row("Unlicense", "MIT"))).toBe("MIT");
  });

  it("refuses a choice that does not offer MIT, naming the row", () => {
    expect(() => licenceShown(row("Apache-2.0", "BSD-3-Clause"))).toThrow(/example/);
  });

  it("resolves every row the About panel lists", () => {
    // The generator stops at the first row this throws for; failing here too
    // puts a new dual-licensed dependency in front of `npm test`, not only of
    // whoever next regenerates the licence texts.
    for (const lib of THIRD_PARTY) {
      expect(lib.licences).toContain(licenceShown(lib));
    }
  });
});
