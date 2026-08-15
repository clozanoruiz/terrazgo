// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// English dictionary, by area. The key set is identical in every locale and
// no key may appear in two files: i18n.js merges them.
//
// This file is the locale's entry point (i18n.js imports it by name) and
// holds no entries of its own — every key lives in the area file that owns
// it, so a new module adds one file per locale instead of editing three
// thousand-line dictionaries. A key defined twice would be silently
// overwritten here, which the i18n contract test refuses.

import common from "./en/common.js";
import errors from "./en/errors.js";
import farm from "./en/farm.js";
import book from "./en/book.js";
import fertilisation from "./en/fertilisation.js";
import map from "./en/map.js";
import settings from "./en/settings.js";

export default {
  ...common,
  ...errors,
  ...farm,
  ...book,
  ...fertilisation,
  ...map,
  ...settings,
};
