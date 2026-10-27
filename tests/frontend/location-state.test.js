// Unit tests for static/location-state.js. Run with: node --test "tests/frontend/*.test.js"
const { describe, it, mock } = require("node:test");
const assert = require("node:assert/strict");

const {
  normalizeCoordinates,
  createLocationState,
  createLatestLoader,
  debounce,
} = require("../../static/location-state.js");

const GPS = { latitude: 4.711, longitude: -74.0721 };
const TOKYO = { latitude: 35.6762, longitude: 139.6503 };

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("normalizeCoordinates", () => {
  it("accepts valid coordinates unchanged", () => {
    assert.deepEqual(normalizeCoordinates(GPS.latitude, GPS.longitude), GPS);
    assert.deepEqual(normalizeCoordinates(90, 180), { latitude: 90, longitude: 180 });
    assert.deepEqual(normalizeCoordinates(-90, -180), { latitude: -90, longitude: -180 });
  });

  it("wraps longitudes from neighbouring world copies", () => {
    assert.deepEqual(normalizeCoordinates(10, 200), { latitude: 10, longitude: -160 });
    assert.deepEqual(normalizeCoordinates(10, -190), { latitude: 10, longitude: 170 });
    assert.deepEqual(normalizeCoordinates(10, 540), { latitude: 10, longitude: 180 });
  });

  it("rejects invalid coordinates", () => {
    for (const [latitude, longitude] of [
      [91, 0],
      [-90.01, 0],
      [Number.NaN, 0],
      [0, Number.POSITIVE_INFINITY],
      [null, 0],
      [undefined, 0],
      ["10", "20"],
    ]) {
      assert.equal(normalizeCoordinates(latitude, longitude), null, `${latitude}, ${longitude}`);
    }
  });
});

describe("createLocationState", () => {
  it("starts empty", () => {
    const state = createLocationState();
    assert.equal(state.currentLocation, null);
    assert.equal(state.selectedLocation, null);
    assert.equal(state.isManualSelection, false);
  });

  it("initializes the selected location from GPS", () => {
    const state = createLocationState();
    assert.equal(state.updateCurrentLocation(GPS.latitude, GPS.longitude), true);
    assert.deepEqual(state.currentLocation, GPS);
    assert.deepEqual(state.selectedLocation, GPS);
    assert.equal(state.isManualSelection, false);
  });

  it("follows GPS updates until the user picks a point", () => {
    const state = createLocationState();
    state.updateCurrentLocation(GPS.latitude, GPS.longitude);
    assert.equal(state.updateCurrentLocation(GPS.latitude, GPS.longitude), false, "same fix is not a change");
    assert.equal(state.updateCurrentLocation(5, -75), true);
    assert.deepEqual(state.selectedLocation, { latitude: 5, longitude: -75 });
  });

  it("moving the marker updates only the selected location", () => {
    const state = createLocationState();
    state.updateCurrentLocation(GPS.latitude, GPS.longitude);

    assert.equal(state.selectLocation(TOKYO.latitude, TOKYO.longitude), true);
    assert.deepEqual(state.selectedLocation, TOKYO);
    assert.deepEqual(state.currentLocation, GPS);
    assert.equal(state.isManualSelection, true);
    assert.equal(state.selectLocation(TOKYO.latitude, TOKYO.longitude), false, "same point is not a change");
  });

  it("does not let later GPS updates overwrite a manual selection", () => {
    const state = createLocationState();
    state.updateCurrentLocation(GPS.latitude, GPS.longitude);
    state.selectLocation(TOKYO.latitude, TOKYO.longitude);

    assert.equal(state.updateCurrentLocation(6, -76), false);
    assert.deepEqual(state.selectedLocation, TOKYO);
    assert.deepEqual(state.currentLocation, { latitude: 6, longitude: -76 });
  });

  it("keeps a selection made before the first GPS fix arrives", () => {
    const state = createLocationState();
    state.selectLocation(TOKYO.latitude, TOKYO.longitude);
    assert.equal(state.updateCurrentLocation(GPS.latitude, GPS.longitude), false);
    assert.deepEqual(state.selectedLocation, TOKYO);
  });

  it("resets the selection to the latest GPS location", () => {
    const state = createLocationState();
    state.updateCurrentLocation(GPS.latitude, GPS.longitude);
    state.selectLocation(TOKYO.latitude, TOKYO.longitude);
    state.updateCurrentLocation(6, -76);

    assert.equal(state.resetToCurrentLocation(), true);
    assert.deepEqual(state.selectedLocation, { latitude: 6, longitude: -76 });
    assert.equal(state.isManualSelection, false);
    assert.equal(state.updateCurrentLocation(7, -77), true, "GPS drives the selection again");
    assert.deepEqual(state.selectedLocation, { latitude: 7, longitude: -77 });
  });

  it("supports manual selection when GPS is unavailable", () => {
    const state = createLocationState();
    assert.equal(state.resetToCurrentLocation(), false);
    assert.equal(state.selectedLocation, null);

    assert.equal(state.selectLocation(TOKYO.latitude, TOKYO.longitude), true);
    assert.deepEqual(state.selectedLocation, TOKYO);
    assert.equal(state.resetToCurrentLocation(), false, "nothing to return to");
    assert.deepEqual(state.selectedLocation, TOKYO);
  });

  it("ignores invalid coordinates", () => {
    const state = createLocationState();
    state.updateCurrentLocation(GPS.latitude, GPS.longitude);

    assert.equal(state.selectLocation(120, 0), false);
    assert.equal(state.updateCurrentLocation(Number.NaN, 0), false);
    assert.deepEqual(state.selectedLocation, GPS);
    assert.deepEqual(state.currentLocation, GPS);
    assert.equal(state.isManualSelection, false);
  });

  it("stores the wrapped longitude for selections on a world copy", () => {
    const state = createLocationState();
    state.selectLocation(10, 200);
    assert.deepEqual(state.selectedLocation, { latitude: 10, longitude: -160 });
  });
});

describe("createLatestLoader", () => {
  it("returns fresh data for a single load", async () => {
    const loader = createLatestLoader(async (location) => ({ for: location }));
    assert.deepEqual(await loader.load(GPS), { status: "fresh", data: { for: GPS } });
  });

  it("drops a slower response for an older location", async () => {
    const pending = new Map();
    const loader = createLatestLoader((location) => {
      const request = deferred();
      pending.set(location, request);
      return request.promise;
    });

    const first = loader.load(GPS);
    const second = loader.load(TOKYO);
    pending.get(TOKYO).resolve({ nearest: "tokyo" });
    pending.get(GPS).resolve({ nearest: "bogota" });

    assert.deepEqual(await second, { status: "fresh", data: { nearest: "tokyo" } });
    assert.deepEqual(await first, { status: "stale" });
  });

  it("aborts the previous request when a new one starts", async () => {
    const signals = [];
    const loader = createLatestLoader((_location, signal) => {
      signals.push(signal);
      return new Promise((resolve, reject) => {
        signal.addEventListener("abort", () => reject(new Error("aborted")));
        setTimeout(() => resolve({ ok: true }), 5);
      });
    });

    const first = loader.load(GPS);
    const second = loader.load(TOKYO);

    assert.equal(signals[0].aborted, true);
    assert.equal(signals[1].aborted, false);
    assert.deepEqual(await first, { status: "stale" }, "abort errors are not reported");
    assert.deepEqual(await second, { status: "fresh", data: { ok: true } });
  });

  it("reports errors from the latest request", async () => {
    const loader = createLatestLoader(async () => {
      throw new Error("The earthquake data provider returned an error.");
    });
    await assert.rejects(loader.load(GPS), /provider returned an error/);
  });

  it("drops errors from superseded requests", async () => {
    const pending = [];
    const loader = createLatestLoader(() => {
      const request = deferred();
      pending.push(request);
      return request.promise;
    });

    const first = loader.load(GPS);
    const second = loader.load(TOKYO);
    pending[0].reject(new Error("old failure"));
    pending[1].resolve({ ok: true });

    assert.deepEqual(await first, { status: "stale" });
    assert.deepEqual(await second, { status: "fresh", data: { ok: true } });
  });
});

describe("debounce", () => {
  it("runs once with the latest arguments after the wait", (context) => {
    context.mock.timers.enable({ apis: ["setTimeout"] });
    const calls = [];
    const debounced = debounce((value) => calls.push(value), 250);

    debounced(1);
    context.mock.timers.tick(200);
    debounced(2);
    context.mock.timers.tick(200);
    assert.deepEqual(calls, []);
    context.mock.timers.tick(50);
    assert.deepEqual(calls, [2]);
  });

  it("can be cancelled", (context) => {
    context.mock.timers.enable({ apis: ["setTimeout"] });
    const fn = mock.fn();
    const debounced = debounce(fn, 250);

    debounced();
    debounced.cancel();
    context.mock.timers.tick(500);
    assert.equal(fn.mock.callCount(), 0);
  });
});
