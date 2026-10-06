// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The holding's form, as data: what the fields hold, how a stored farm fills
 * them, and what a submit sends.
 *
 * It exists because two screens ask the same questions — the farms list creates
 * one, the farm's own page corrects it — and until 2026-09-15 they asked
 * different ones: four fields against twenty-six, because `insert_farm` wrote
 * four columns whatever the payload carried. With the backend widened, the only
 * difference left is that `country_code` may be stated at creation and never
 * again, so the two forms are one form and this is the shape it edits.
 *
 * Framework-agnostic on purpose (docs/frontend-conventions.md → the two-tier
 * rule): every rule about what a blank field means lives here, where it can be
 * unit-tested, rather than in the component that draws it.
 *
 * THE RULE THOSE TESTS ARE ABOUT: a blank field is `null`, never `""`. A blank
 * cell in the printed cuaderno means "not recorded" and leaves the model's
 * ruled line for a hand to fill; an empty string is a statement that there is
 * nothing to record, which is a different claim and the wrong one.
 */

/// Every field the form holds, all of them strings — a form field's value is
/// text, and the conversions happen on the way out in `farmPayload`.
export function emptyFarmDraft(countryCode = "") {
  return {
    name: "",
    ownerName: "",
    ownerTaxId: "",
    countryCode,
    locationText: "",
    address: "",
    postalCode: "",
    phoneFixed: "",
    phoneMobile: "",
    email: "",
    openedOn: "",
    latitude: "",
    longitude: "",
    regaCode: "",
    reaCode: "",
    siexCode: "",
    provinceCode: "",
    repName: "",
    repTaxId: "",
    repKind: "",
    repAddress: "",
    repLocality: "",
    repProvince: "",
    repPostalCode: "",
    repPhone: "",
    repEmail: "",
  };
}

/// Fill the form from a stored `FarmDetail` (`{ farm, es, representative }`).
export function farmDraftFrom(detail) {
  const { farm, es, representative } = detail;
  return {
    name: farm.name,
    ownerName: farm.owner_name ?? "",
    ownerTaxId: farm.owner_tax_id ?? "",
    countryCode: farm.country_code,
    locationText: farm.location_text ?? "",
    address: farm.address ?? "",
    postalCode: farm.postal_code ?? "",
    phoneFixed: farm.phone_fixed ?? "",
    phoneMobile: farm.phone_mobile ?? "",
    email: farm.email ?? "",
    openedOn: farm.opened_on ?? "",
    latitude: farm.latitude ?? "",
    longitude: farm.longitude ?? "",
    regaCode: es?.rega_code ?? "",
    reaCode: es?.rea_code ?? "",
    siexCode: es?.siex_code ?? "",
    provinceCode: es?.province_code ?? "",
    repName: representative?.full_name ?? "",
    repTaxId: representative?.tax_id ?? "",
    repKind: representative?.representation_kind ?? "",
    repAddress: representative?.address ?? "",
    repLocality: representative?.locality ?? "",
    repProvince: representative?.province ?? "",
    repPostalCode: representative?.postal_code ?? "",
    repPhone: representative?.phone ?? "",
    repEmail: representative?.email ?? "",
  };
}

function text(value) {
  return String(value ?? "").trim() || null;
}

function numberOrNull(value) {
  const trimmed = String(value ?? "").trim();
  if (trimmed === "") return null;
  const parsed = Number(trimmed);
  return Number.isNaN(parsed) ? null : parsed;
}

/// The Spanish extension block, or null when there is nothing in it. A
/// non-Spanish holding never has one; a Spanish one with every field blank does
/// not either, and submitting null is what removes a stored row.
export function farmEsFields(draft) {
  if (draft.countryCode !== "es") return null;
  const es = {
    rega_code: text(draft.regaCode),
    rea_code: text(draft.reaCode),
    siex_code: text(draft.siexCode),
    province_code: text(draft.provinceCode),
  };
  return Object.values(es).some(Boolean) ? es : null;
}

/// The representative block. THE NAME IS WHAT MAKES ONE EXIST: with it blank the
/// whole block is null, which removes any stored row. The common case is that
/// there is none — the holder signs their own book.
export function farmRepresentativeFields(draft) {
  const fullName = String(draft.repName ?? "").trim();
  if (!fullName) return null;
  return {
    full_name: fullName,
    tax_id: text(draft.repTaxId),
    representation_kind: text(draft.repKind),
    address: text(draft.repAddress),
    locality: text(draft.repLocality),
    province: text(draft.repProvince),
    postal_code: text(draft.repPostalCode),
    phone: text(draft.repPhone),
    email: text(draft.repEmail),
  };
}

/**
 * What a submit sends.
 *
 * `create: true` adds `country_code`, the one field `UpdateFarm` does not carry
 * and `NewFarm` does — a farm's country is stated once, because changing it
 * would re-home the meaning of every country-scoped code already on its
 * records. Everything else is identical, which is the point.
 */
export function farmPayload(draft, { create = false } = {}) {
  const payload = {
    name: String(draft.name ?? "").trim(),
    owner_name: text(draft.ownerName),
    owner_tax_id: text(draft.ownerTaxId),
    location_text: text(draft.locationText),
    address: text(draft.address),
    postal_code: text(draft.postalCode),
    phone_fixed: text(draft.phoneFixed),
    phone_mobile: text(draft.phoneMobile),
    email: text(draft.email),
    opened_on: draft.openedOn || null,
    latitude: numberOrNull(draft.latitude),
    longitude: numberOrNull(draft.longitude),
    es: farmEsFields(draft),
    representative: farmRepresentativeFields(draft),
  };
  if (create) payload.country_code = draft.countryCode;
  return payload;
}
