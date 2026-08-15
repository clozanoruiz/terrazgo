// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionari català, per àrees. Les claus són idèntiques en tots els idiomes
// i cap pot repetir-se entre fitxers: i18n.js els fusiona.
//
// This file is the locale's entry point (i18n.js imports it by name) and
// holds no entries of its own — every key lives in the area file that owns
// it, so a new module adds one file per locale instead of editing three
// thousand-line dictionaries. A key defined twice would be silently
// overwritten here, which the i18n contract test refuses.

import common from "./ca/common.js";
import errors from "./ca/errors.js";
import farm from "./ca/farm.js";
import book from "./ca/book.js";
import fertilisation from "./ca/fertilisation.js";
import map from "./ca/map.js";
import settings from "./ca/settings.js";

export default {
  ...common,
  ...errors,
  ...farm,
  ...book,
  ...fertilisation,
  ...map,
  ...settings,
};
