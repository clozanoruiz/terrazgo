// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  emptyFarmDraft,
  farmDraftFrom,
  farmEsFields,
  farmPayload,
  farmRepresentativeFields,
} from "./farmDraft.js";

const stored = {
  farm: {
    name: "Finca Los Llanos",
    owner_name: "Carlos Lozano",
    owner_tax_id: "12345678Z",
    country_code: "es",
    location_text: "Moreruela de Tábara",
    address: "Camino de los Llanos, 14",
    postal_code: "49148",
    phone_fixed: "980 000 000",
    phone_mobile: null,
    email: "finca@ejemplo.es",
    opened_on: "2026-01-02",
    latitude: 41.80312,
    longitude: -5.84571,
  },
  es: { rega_code: "ES490000001", rea_code: null, siex_code: null, province_code: "49" },
  representative: {
    full_name: "Ana Gómez",
    tax_id: "87654321X",
    representation_kind: "Administradora",
    address: null,
    locality: null,
    province: null,
    postal_code: null,
    phone: null,
    email: null,
  },
};

describe("emptyFarmDraft", () => {
  it("holds every field as a string, so a control always has a value to bind", () => {
    const draft = emptyFarmDraft();
    expect(Object.values(draft).every((v) => typeof v === "string")).toBe(true);
  });

  it("takes the country it is created for, the one field that cannot change later", () => {
    expect(emptyFarmDraft("it").countryCode).toBe("it");
  });
});

describe("farmDraftFrom", () => {
  it("fills every field a stored holding has", () => {
    const draft = farmDraftFrom(stored);
    expect(draft.name).toBe("Finca Los Llanos");
    expect(draft.address).toBe("Camino de los Llanos, 14");
    expect(draft.openedOn).toBe("2026-01-02");
    expect(draft.latitude).toBe(41.80312);
    expect(draft.regaCode).toBe("ES490000001");
    expect(draft.repKind).toBe("Administradora");
  });

  it("turns a stored null into a blank field, never the string 'null'", () => {
    const draft = farmDraftFrom(stored);
    expect(draft.phoneMobile).toBe("");
    expect(draft.reaCode).toBe("");
    expect(draft.repLocality).toBe("");
  });

  it("survives a holding with no extension and no representative", () => {
    const draft = farmDraftFrom({ farm: stored.farm, es: null, representative: null });
    expect(draft.siexCode).toBe("");
    expect(draft.repName).toBe("");
  });

  it("round-trips through a payload without inventing or losing a field", () => {
    const payload = farmPayload(farmDraftFrom(stored));
    expect(payload.name).toBe("Finca Los Llanos");
    expect(payload.phone_mobile).toBeNull();
    expect(payload.latitude).toBe(41.80312);
    expect(payload.es.rega_code).toBe("ES490000001");
    expect(payload.representative.full_name).toBe("Ana Gómez");
  });
});

describe("farmPayload", () => {
  it("sends null for a blank field, never an empty string", () => {
    // A blank cell in the printed book means "not recorded" and leaves the
    // model's ruled line; "" would state there is nothing to record.
    const payload = farmPayload(emptyFarmDraft("es"));
    for (const [key, value] of Object.entries(payload)) {
      if (key === "name") continue;
      expect(value, key).not.toBe("");
    }
    expect(payload.address).toBeNull();
    expect(payload.opened_on).toBeNull();
  });

  it("trims what it is given", () => {
    const draft = { ...emptyFarmDraft("es"), name: "  Finca  ", email: "  a@b.es  " };
    const payload = farmPayload(draft);
    expect(payload.name).toBe("Finca");
    expect(payload.email).toBe("a@b.es");
  });

  it("omits country_code when correcting and carries it when creating", () => {
    const draft = emptyFarmDraft("it");
    expect(farmPayload(draft)).not.toHaveProperty("country_code");
    expect(farmPayload(draft, { create: true }).country_code).toBe("it");
  });

  it("reads a coordinate as a number and a blank one as null", () => {
    const draft = { ...emptyFarmDraft("es"), latitude: "41.5", longitude: "" };
    const payload = farmPayload(draft);
    expect(payload.latitude).toBe(41.5);
    expect(payload.longitude).toBeNull();
  });

  it("refuses an unreadable coordinate rather than sending NaN", () => {
    const payload = farmPayload({ ...emptyFarmDraft("es"), latitude: "norte" });
    expect(payload.latitude).toBeNull();
  });
});

describe("farmEsFields", () => {
  it("is null for a holding outside Spain, whatever the fields hold", () => {
    const draft = { ...emptyFarmDraft("it"), regaCode: "ES490000001" };
    expect(farmEsFields(draft)).toBeNull();
  });

  it("is null when every Spanish field is blank, which removes a stored row", () => {
    expect(farmEsFields(emptyFarmDraft("es"))).toBeNull();
  });

  it("is submitted as soon as one field has something in it", () => {
    const draft = { ...emptyFarmDraft("es"), provinceCode: "49" };
    expect(farmEsFields(draft)).toEqual({
      rega_code: null,
      rea_code: null,
      siex_code: null,
      province_code: "49",
    });
  });
});

describe("farmRepresentativeFields", () => {
  it("is null without a name, however much else is filled in", () => {
    // The name is what makes a representative exist; the rest describes one.
    const draft = { ...emptyFarmDraft("es"), repTaxId: "87654321X", repPhone: "600 000 000" };
    expect(farmRepresentativeFields(draft)).toBeNull();
  });

  it("is submitted whole once it has a name", () => {
    const draft = { ...emptyFarmDraft("es"), repName: " Ana Gómez ", repProvince: "Zamora" };
    const fields = farmRepresentativeFields(draft);
    expect(fields.full_name).toBe("Ana Gómez");
    expect(fields.province).toBe("Zamora");
    expect(fields.email).toBeNull();
  });
});
