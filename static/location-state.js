// Selected-location state for the earthquake map. DOM-free so it can be unit
// tested with Node (tests/frontend) and loaded as a plain script by index.html.
(function (root, factory) {
  if (typeof module === "object" && module.exports) {
    module.exports = factory();
  } else {
    root.LocationState = factory();
  }
})(typeof self !== "undefined" ? self : this, function () {
  "use strict";

  /** Returns `{ latitude, longitude }` with longitude wrapped to [-180, 180], or null if invalid. */
  function normalizeCoordinates(latitude, longitude) {
    if (typeof latitude !== "number" || typeof longitude !== "number") return null;
    if (!Number.isFinite(latitude) || !Number.isFinite(longitude)) return null;
    if (latitude < -90 || latitude > 90) return null;

    if (longitude >= -180 && longitude <= 180) return { latitude, longitude };

    // Only out-of-range values are wrapped, so in-range ones keep their exact value.
    let wrapped = ((((longitude + 180) % 360) + 360) % 360) - 180;
    if (wrapped === -180 && longitude > 0) wrapped = 180;
    return { latitude, longitude: wrapped };
  }

  function sameLocation(a, b) {
    return a !== null && b !== null && a.latitude === b.latitude && a.longitude === b.longitude;
  }

  /**
   * Tracks the GPS position (`currentLocation`) separately from the point used
   * for distance calculations (`selectedLocation`). Once the user picks a point
   * manually, GPS updates no longer move the selection until it is reset.
   * Mutators return true when `selectedLocation` changed.
   */
  function createLocationState() {
    let currentLocation = null;
    let selectedLocation = null;
    let manualSelection = false;

    return {
      get currentLocation() { return currentLocation; },
      get selectedLocation() { return selectedLocation; },
      get isManualSelection() { return manualSelection; },

      updateCurrentLocation(latitude, longitude) {
        const location = normalizeCoordinates(latitude, longitude);
        if (!location) return false;
        currentLocation = location;
        if (manualSelection || sameLocation(selectedLocation, location)) return false;
        selectedLocation = location;
        return true;
      },

      selectLocation(latitude, longitude) {
        const location = normalizeCoordinates(latitude, longitude);
        if (!location) return false;
        manualSelection = true;
        if (sameLocation(selectedLocation, location)) return false;
        selectedLocation = location;
        return true;
      },

      resetToCurrentLocation() {
        if (!currentLocation) return false;
        manualSelection = false;
        if (sameLocation(selectedLocation, currentLocation)) return false;
        selectedLocation = currentLocation;
        return true;
      },
    };
  }

  /**
   * Wraps `fetchNearby(location, signal)` so only the newest load reports data.
   * Starting a load aborts the previous one; anything that settles after a newer
   * load started resolves to `{ status: "stale" }` instead of its data or error.
   */
  function createLatestLoader(fetchNearby) {
    let latestId = 0;
    let controller = null;

    return {
      async load(location) {
        const id = ++latestId;
        if (controller) controller.abort();
        const ownController = new AbortController();
        controller = ownController;

        try {
          const data = await fetchNearby(location, ownController.signal);
          return id === latestId ? { status: "fresh", data } : { status: "stale" };
        } catch (error) {
          if (id !== latestId) return { status: "stale" };
          throw error;
        }
      },
    };
  }

  /** Delays `fn` until `waitMs` passes without another call. */
  function debounce(fn, waitMs) {
    let timer = null;
    const debounced = (...args) => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        fn(...args);
      }, waitMs);
    };
    debounced.cancel = () => {
      clearTimeout(timer);
      timer = null;
    };
    return debounced;
  }

  return { normalizeCoordinates, createLocationState, createLatestLoader, debounce };
});
