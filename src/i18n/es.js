// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.
//
// This file is the locale's entry point (i18n.js imports it by name) and
// holds no entries of its own — every key lives in the area file that owns
// it, so a new module adds one file per locale instead of editing three
// thousand-line dictionaries. A key defined twice would be silently
// overwritten here, which the i18n contract test refuses.

import common from "./es/common.js";
import errors from "./es/errors.js";
import farm from "./es/farm.js";
import book from "./es/book.js";
import fertilisation from "./es/fertilisation.js";
import map from "./es/map.js";
import settings from "./es/settings.js";

export default {
  ...common,
  ...errors,
  ...farm,
  ...book,
  ...fertilisation,
  ...map,
  ...settings,
};
