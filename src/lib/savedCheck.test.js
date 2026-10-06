// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The hub between a register's form and the book page's duplicate check.
import { describe, expect, it } from "vitest";
import { checkSaved, onSaved } from "./savedCheck.js";

describe("savedCheck", () => {
  it("hands a save to the listener with its register and id", async () => {
    const heard = [];
    const stop = onSaved(async (register, id) => heard.push([register, id]));
    await checkSaved("treatment_record", "r1");
    expect(heard).toEqual([["treatment_record", "r1"]]);
    stop();
  });

  it("waits for the listener to finish", async () => {
    let done = false;
    const stop = onSaved(async () => {
      await new Promise((resolve) => setTimeout(resolve, 5));
      done = true;
    });
    await checkSaved("crop", "c1");
    expect(done).toBe(true);
    stop();
  });

  it("does nothing, and does not fail, when nothing listens", async () => {
    await expect(checkSaved("crop", "c1")).resolves.toBeUndefined();
  });

  it("does nothing for a save it could not name", async () => {
    const heard = [];
    const stop = onSaved(async (register, id) => heard.push([register, id]));
    await checkSaved("crop", undefined);
    expect(heard).toEqual([]);
    stop();
  });

  it("stops listening when told to", async () => {
    const heard = [];
    const stop = onSaved(async (register, id) => heard.push([register, id]));
    stop();
    await checkSaved("crop", "c1");
    expect(heard).toEqual([]);
  });

  it("is not silenced by an earlier listener stopping after a later one began", async () => {
    // A book page unmounting after the next one mounted must not stop the
    // new page's check.
    const heard = [];
    const stopOld = onSaved(async () => heard.push("old"));
    const stopNew = onSaved(async () => heard.push("new"));
    stopOld();
    await checkSaved("crop", "c1");
    expect(heard).toEqual(["new"]);
    stopNew();
  });
});
