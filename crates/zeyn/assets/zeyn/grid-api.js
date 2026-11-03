/**
 * Functions and constants of the always loaded runtime (`zeyn.js`) that the grid runtime uses,
 * handed over by `install` before the first grid is set up.
 */
let BASE, FILTER_DELAY_MS, MAX_COLUMN, MIN_COLUMN, UNREACHABLE, VIEW_SAVE_DELAY_MS, V_CACHE, V_CACHE_CELLS, V_CAP, V_FETCH_DELAY_MS, V_OVERSCAN, address, fill, fmt, grids, make, num, openMenu, paramsQuery, parseEffect, place, post, readJson, relativeTimes, runEffect, runEffects, str, toast;

/** Takes the shared functions and constants from `zeyn.js`; called once, right after loading. */
export function install(z) {
  ({ BASE, FILTER_DELAY_MS, MAX_COLUMN, MIN_COLUMN, UNREACHABLE, VIEW_SAVE_DELAY_MS, V_CACHE, V_CACHE_CELLS, V_CAP, V_FETCH_DELAY_MS, V_OVERSCAN, address, fill, fmt, grids, make, num, openMenu, paramsQuery, parseEffect, place, post, readJson, relativeTimes, runEffect, runEffects, str, toast } = z);
}

