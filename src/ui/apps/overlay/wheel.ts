// Turns wheel input into discrete steps: one step per mouse wheel notch, one step per
// touchpad gesture however long its momentum lasts (a gesture ends after a short pause).
// Shared by the launcher (pages) and the window switcher (windows).

const GESTURE_GAP = 180;
const TOUCHPAD_THRESHOLD = 50;
const NOTCH_INTERVAL = 200;

export type Direction = 1 | -1;

export function wheelStepper(step: (direction: Direction) => void) {
  let gestureDelta = 0;
  let gestureDone = false;
  let gestureEnd = 0;
  let nextNotch = 0;

  const handle = (e: WheelEvent, delta: number) => {
    if (e.deltaMode === WheelEvent.DOM_DELTA_LINE) delta *= 40;
    else if (e.deltaMode === WheelEvent.DOM_DELTA_PAGE) delta *= 800;
    if (!delta) return;

    const notch =
      e.deltaMode !== WheelEvent.DOM_DELTA_PIXEL || (Number.isInteger(delta) && Math.abs(delta) >= 100);
    if (notch) {
      const now = performance.now();
      if (now < nextNotch) return;
      nextNotch = now + NOTCH_INTERVAL;
      step(delta > 0 ? 1 : -1);
      return;
    }

    clearTimeout(gestureEnd);
    gestureEnd = window.setTimeout(() => {
      gestureDelta = 0;
      gestureDone = false;
    }, GESTURE_GAP);
    if (gestureDone) return;
    gestureDelta += delta;
    if (Math.abs(gestureDelta) >= TOUCHPAD_THRESHOLD) {
      gestureDone = true;
      step(gestureDelta > 0 ? 1 : -1);
    }
  };

  return {
    /** Feeds one wheel event; `delta` is the axis the caller cares about. */
    handle,
    dispose: () => clearTimeout(gestureEnd),
  };
}
